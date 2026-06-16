// src/mobile.rs
// Slint バックエンド — Android / デスクトップ Mobile UI

use slint::{Image, ModelRc, Rgb8Pixel, SharedPixelBuffer, SharedString, VecModel};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use crate::app::{App, AppEvent, UiCommand};
use crate::backend::BackendError;
use crate::font_loading::load_desktop_fonts;
use crate::io::{AssetProvider, DesktopAssetProvider};
use crate::renderer::{ArgbSurface, RenderCache};
use crate::screen_keyboard::{
    self, ScreenKeyboardAction, ScreenKeyboardInputState, ScreenKeyboardKey,
    ScreenKeyboardKeyClass, ScreenKeyboardKeyRole, ScreenKeyboardKeyWidth,
    ScreenKeyboardLayoutKind,
};
use crate::ui;

slint::include_modules!();

fn shared(value: &str) -> SharedString {
    SharedString::from(value)
}

fn optional_shared(value: Option<&str>) -> SharedString {
    SharedString::from(value.unwrap_or(""))
}

fn mobile_visual_kind(class: ScreenKeyboardKeyClass) -> KeyboardVisualKind {
    match class {
        ScreenKeyboardKeyClass::Text => KeyboardVisualKind::Text,
        ScreenKeyboardKeyClass::Special => KeyboardVisualKind::Special,
        ScreenKeyboardKeyClass::Backspace => KeyboardVisualKind::Backspace,
        ScreenKeyboardKeyClass::Enter => KeyboardVisualKind::Enter,
        ScreenKeyboardKeyClass::Modifier => KeyboardVisualKind::Modifier,
        ScreenKeyboardKeyClass::Spacer => KeyboardVisualKind::Spacer,
    }
}

fn mobile_width_kind(width: ScreenKeyboardKeyWidth) -> KeyboardWidthKind {
    match width {
        ScreenKeyboardKeyWidth::Normal => KeyboardWidthKind::Normal,
        ScreenKeyboardKeyWidth::Wide => KeyboardWidthKind::Wide,
        ScreenKeyboardKeyWidth::Space => KeyboardWidthKind::Space,
    }
}

fn mobile_key_view(key: &ScreenKeyboardKey) -> KeyboardKeyView {
    let (up, right, down, left) = match key.role {
        ScreenKeyboardKeyRole::Flick(map) => (map.up, map.right, map.down, map.left),
        ScreenKeyboardKeyRole::Action(_) | ScreenKeyboardKeyRole::Spacer => {
            (None, None, None, None)
        }
    };

    KeyboardKeyView {
        label: shared(key.label),
        up: optional_shared(up),
        right: optional_shared(right),
        down: optional_shared(down),
        left: optional_shared(left),
        visual_kind: mobile_visual_kind(key.class),
        width_kind: mobile_width_kind(key.width),
        interactive: !matches!(key.role, ScreenKeyboardKeyRole::Spacer),
    }
}

fn mobile_row_model(keys: &[ScreenKeyboardKey]) -> ModelRc<KeyboardKeyView> {
    ModelRc::new(VecModel::from(
        keys.iter().map(mobile_key_view).collect::<Vec<_>>(),
    ))
}

fn apply_screen_keyboard_layout(window: &AppWindow, kind: ScreenKeyboardLayoutKind) {
    let layout = screen_keyboard::layout(kind);
    window.set_keyboard_row_0(mobile_row_model(layout.rows[0].keys));
    window.set_keyboard_row_1(mobile_row_model(layout.rows[1].keys));
    window.set_keyboard_row_2(mobile_row_model(layout.rows[2].keys));
    window.set_keyboard_row_3(mobile_row_model(layout.rows[3].keys));
}

fn send_screen_keyboard_text(app: &mut App, state: &mut ScreenKeyboardInputState, text: &str) {
    let timestamp = crate::timestamp::now();
    for ch in text.chars() {
        app.on_event(AppEvent::Char { c: ch, timestamp });
    }
    state.record_text(text, !app.has_pending_input_correction());
}

fn dispatch_screen_keyboard_action(
    app: &mut App,
    state: &mut ScreenKeyboardInputState,
    layout_kind: &mut ScreenKeyboardLayoutKind,
    window: &AppWindow,
    action: ScreenKeyboardAction,
) {
    match action {
        ScreenKeyboardAction::Text(text) => {
            send_screen_keyboard_text(app, state, text);
        }
        ScreenKeyboardAction::UiCommand(command) => {
            state.clear();
            let command = UiCommand::from(command);
            app.on_event(command.app_event());
        }
        ScreenKeyboardAction::TransformLastText => {
            if let Some(next_char) = state.pending_modified_char() {
                app.on_event(AppEvent::Backspace);
                let mut buf = [0_u8; 4];
                send_screen_keyboard_text(app, state, next_char.encode_utf8(&mut buf));
            }
        }
        ScreenKeyboardAction::SwitchLayout => {
            state.clear();
            *layout_kind = (*layout_kind).next();
            apply_screen_keyboard_layout(window, *layout_kind);
        }
        ScreenKeyboardAction::SwitchInputSource => {
            state.clear();
        }
        ScreenKeyboardAction::None => {}
    }
}

fn argb_to_rgb8(src: &[u32], dst: &mut [Rgb8Pixel]) {
    for (s, d) in src.iter().zip(dst.iter_mut()) {
        d.r = ((*s >> 16) & 0xFF) as u8;
        d.g = ((*s >> 8) & 0xFF) as u8;
        d.b = (*s & 0xFF) as u8;
    }
}

fn render_frame(
    app: &App,
    width: usize,
    height: usize,
    pixel_buf: &mut Vec<u32>,
    render_cache: &mut RenderCache,
) -> bool {
    pixel_buf.resize(width * height, 0u32);
    let display_settings = app.display_settings();
    let viewport = display_settings.viewport(width, height);
    let fonts = app.fonts();
    let render_list = ui::build_ui(app, fonts, viewport.width, viewport.height);
    if let Some(mut surface) = ArgbSurface::new(width, height, pixel_buf) {
        surface
            .render(fonts, display_settings, &render_list, render_cache)
            .changed()
    } else {
        false
    }
}

/// Slint の Window から実際のキャンバス物理ピクセルサイズを計算する。
///
/// キーボードエリアの高さは Slint 側の `keyboard-logical-height` (out property) から
/// 動的に取得する。これにより制御ストリップの折畳み状態も正確に反映される。
/// (制御ストリップ 32px 常時 + キー行 288px = 最大 320 論理 px)
///
/// ウィンドウがまだ表示されていない (size = 0) 場合は None を返す。
fn canvas_physical_size(win: &AppWindow) -> Option<(usize, usize)> {
    let phys = win.window().size();
    if phys.width == 0 || phys.height == 0 {
        return None;
    }
    let scale = win.window().scale_factor();
    // Slint から論理 px 高さを取得し、scale を掛けて物理 px に変換
    let kb_logical_h = win.get_keyboard_logical_height();
    let kb_phys_h = (kb_logical_h * scale).round() as u32;
    let h = phys.height.saturating_sub(kb_phys_h);
    if h == 0 {
        return None;
    }
    Some((phys.width as usize, h as usize))
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let asset_provider = DesktopAssetProvider::discover();
    let fonts = load_desktop_fonts(&asset_provider)?;
    let mut app = App::new(fonts);
    app.set_available_fonts(asset_provider.list_fonts());
    let app_state = Rc::new(RefCell::new(app));
    {
        let mut a = app_state
            .try_borrow_mut()
            .map_err(|_| BackendError::state("mobile app state is already borrowed"))?;
        a.on_event(AppEvent::ChangeScene);
    }

    let window = AppWindow::new()?;
    let screen_keyboard_layout = Rc::new(RefCell::new(ScreenKeyboardLayoutKind::default()));
    let screen_keyboard_input_state = Rc::new(RefCell::new(ScreenKeyboardInputState::new()));
    apply_screen_keyboard_layout(&window, *screen_keyboard_layout.borrow());

    // --- コールバック: フリック文字 ---
    {
        let app = Rc::clone(&app_state);
        let screen_keyboard_input_state = Rc::clone(&screen_keyboard_input_state);
        let win = window.as_weak();
        window.on_char_input(move |c| {
            screen_keyboard_input_state.borrow_mut().clear();
            let timestamp = crate::timestamp::now();
            let Ok(mut a) = app.try_borrow_mut() else {
                return;
            };
            for ch in c.chars() {
                a.on_event(AppEvent::Char { c: ch, timestamp });
            }
            if let Some(w) = win.upgrade() {
                w.set_keyboard_visible(a.is_typing_active());
            }
        });
    }

    // --- コールバック: 特殊キー ---
    {
        let app = Rc::clone(&app_state);
        let screen_keyboard_input_state = Rc::clone(&screen_keyboard_input_state);
        let win = window.as_weak();
        window.on_special_input(move |action| {
            screen_keyboard_input_state.borrow_mut().clear();
            let Ok(mut a) = app.try_borrow_mut() else {
                return;
            };
            if let Some(command) = UiCommand::from_bridge_label(action.as_str()) {
                a.on_event(command.app_event());
            }
            if let Some(w) = win.upgrade() {
                w.set_keyboard_visible(a.is_typing_active());
            }
        });
    }

    // --- コールバック: 共通スクリーンキーボード ---
    {
        let app = Rc::clone(&app_state);
        let screen_keyboard_input_state = Rc::clone(&screen_keyboard_input_state);
        let screen_keyboard_layout = Rc::clone(&screen_keyboard_layout);
        let win = window.as_weak();
        window.on_screen_keyboard_gesture(move |row, key, dx, dy| {
            let Some(window) = win.upgrade() else {
                return;
            };
            let Ok(mut a) = app.try_borrow_mut() else {
                return;
            };
            let mut layout_kind = screen_keyboard_layout.borrow_mut();
            let mut input_state = screen_keyboard_input_state.borrow_mut();
            let action =
                screen_keyboard::resolve_key(*layout_kind, row as usize, key as usize, dx, dy);
            dispatch_screen_keyboard_action(
                &mut a,
                &mut input_state,
                &mut layout_kind,
                &window,
                action,
            );
            window.set_keyboard_visible(a.is_typing_active());
        });
    }

    // --- 描画タイマー (~60fps) ---
    // canvas_physical_size() でキャンバスの物理ピクセルサイズを取得し描画する。
    // Slint 側は ImageFit.fill を使用しており、描画サイズと論理キャンバスの
    // アスペクト比が一致するため歪みは発生しない。
    let mut pixel_buf: Vec<u32> = Vec::new();
    let mut render_cache = RenderCache::new();
    let win_weak = window.as_weak();
    let app = Rc::clone(&app_state);
    let mut last_frame = Instant::now();

    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(16),
        move || {
            let win = match win_weak.upgrade() {
                Some(w) => w,
                None => return,
            };

            // 実際の経過時間を計測（FPS計算に使用）
            let now = Instant::now();
            let delta_ms = now.duration_since(last_frame).as_secs_f64() * 1000.0;
            last_frame = now;

            // ウィンドウが未表示の場合はスキップ
            let (w, h) = match canvas_physical_size(&win) {
                Some(s) => s,
                None => return,
            };

            {
                let Ok(mut a) = app.try_borrow_mut() else {
                    return;
                };
                // 実測 delta_ms を渡すことで app.fps が正確な値になる
                let viewport = a.display_settings().viewport(w, h);
                a.update(viewport.width, viewport.height, delta_ms);
                if let Some(request) = a.take_font_load_request() {
                    match asset_provider.load_font(request.font_id) {
                        Ok(bytes) => {
                            if let Err(err) =
                                a.apply_font_bytes(request.target, request.font_name, bytes)
                            {
                                a.report_visible_error(format!("failed to apply font: {err:?}"));
                            }
                        }
                        Err(err) => a.report_visible_error(err.to_string()),
                    }
                }
                let frame_changed = render_frame(&a, w, h, &mut pixel_buf, &mut render_cache);
                win.set_keyboard_visible(a.is_typing_active());
                if !frame_changed {
                    return;
                }
            }

            // ピクセルバッファ → Slint Image（物理ピクセル等倍）
            let mut slint_buf = SharedPixelBuffer::<Rgb8Pixel>::new(w as u32, h as u32);
            argb_to_rgb8(&pixel_buf, slint_buf.make_mut_slice());
            win.set_frame(Image::from_rgb8(slint_buf));
        },
    );

    window.run()?;
    drop(timer);

    Ok(())
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(android_app: slint::android::AndroidApp) {
    if let Err(err) = slint::android::init(android_app) {
        eprintln!("failed to initialize Android Slint backend: {err}");
        return;
    }
    if let Err(err) = run() {
        eprintln!("mobile backend failed: {err}");
    }
}
