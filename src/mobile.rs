// src/mobile.rs
// Slint バックエンド — Android / デスクトップ Mobile UI

use ab_glyph::FontVec;
use slint::{Image, Rgb8Pixel, SharedPixelBuffer};
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use crate::app::{App, AppEvent, Fonts, UiCommand};
use crate::backend::BackendError;
use crate::io::{AssetProvider, BundledFont, DesktopAssetProvider};
use crate::renderer::{ArgbSurface, RenderCache};
use crate::ui;

slint::include_modules!();

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
) {
    pixel_buf.resize(width * height, 0u32);
    let font = app.get_current_font();
    let render_list = ui::build_ui(app, font, width, height);
    if let Some(mut surface) = ArgbSurface::new(width, height, pixel_buf) {
        surface.render(font, &render_list, render_cache);
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
    let japanese_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::YujiSyukuRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Yuji Syuku font"))?;
    let traditional_chinese_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::NotoSerifJpRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Noto Serif JP font"))?;
    let simplified_chinese_font =
        FontVec::try_from_vec(asset_provider.load_bundled_font(BundledFont::NotoSerifJpRegular)?)
            .map_err(|_| BackendError::asset("failed to parse Noto Serif JP font"))?;

    let fonts = Fonts {
        japanese: japanese_font,
        traditional_chinese: Some(traditional_chinese_font),
        simplified_chinese: Some(simplified_chinese_font),
    };
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

    // --- コールバック: フリック文字 ---
    {
        let app = Rc::clone(&app_state);
        let win = window.as_weak();
        window.on_char_input(move |c| {
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
        let win = window.as_weak();
        window.on_special_input(move |action| {
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
                a.update(w, h, delta_ms);
                if let Some(request) = a.take_font_load_request() {
                    match asset_provider.load_font(request.font_id) {
                        Ok(bytes) => {
                            if let Err(err) = a.apply_font_bytes(request.script, bytes) {
                                a.report_visible_error(format!("failed to apply font: {err:?}"));
                            }
                        }
                        Err(err) => a.report_visible_error(err.to_string()),
                    }
                }
                render_frame(&a, w, h, &mut pixel_buf, &mut render_cache);
                win.set_keyboard_visible(a.is_typing_active());
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
