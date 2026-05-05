// src/mobile.rs
// Slint バックエンド — Android / デスクトップ Mobile UI

use ab_glyph::FontVec;
use slint::{Image, Rgb8Pixel, SharedPixelBuffer};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::app::{App, AppEvent, Fonts};
use crate::renderer::{calculate_pixel_font_size, gui_renderer};
use crate::ui::{self, ActiveLowerElement, LowerTypingSegment, Renderable, UpperSegmentState};

slint::include_modules!();

fn argb_to_rgb8(src: &[u32], dst: &mut [Rgb8Pixel]) {
    for (s, d) in src.iter().zip(dst.iter_mut()) {
        d.r = ((*s >> 16) & 0xFF) as u8;
        d.g = ((*s >> 8) & 0xFF) as u8;
        d.b = (*s & 0xFF) as u8;
    }
}

fn render_frame(app: &App, width: usize, height: usize, pixel_buf: &mut Vec<u32>) {
    pixel_buf.resize(width * height, 0u32);
    let font = app.get_current_font();
    let render_list = ui::build_ui(app, font, width, height);

    for item in render_list {
        match item {
            Renderable::Background { gradient } => {
                crate::renderer::draw_linear_gradient(
                    pixel_buf,
                    width,
                    height,
                    gradient.start_color,
                    gradient.end_color,
                    (0.0, 0.0),
                    (width as f32, height as f32),
                );
            }
            Renderable::BigText {
                text,
                anchor,
                shift,
                align,
                font_size,
                color,
            }
            | Renderable::Text {
                text,
                anchor,
                shift,
                align,
                font_size,
                color,
            } => {
                let pfs = calculate_pixel_font_size(font_size, width, height);
                let (tw, th, _) = gui_renderer::measure_text(font, &text, pfs);
                let ap = ui::calculate_anchor_position(anchor, shift, width, height);
                let (x, y) = ui::calculate_aligned_position(ap, tw, th, align);
                gui_renderer::draw_text(
                    pixel_buf,
                    width,
                    font,
                    &text,
                    (x as f32, y as f32),
                    pfs,
                    color,
                );
            }
            Renderable::TypingUpper {
                segments,
                anchor,
                shift,
                align,
                font_size,
            } => {
                let pfs = calculate_pixel_font_size(font_size, width, height);
                let ruby_pfs = pfs * 0.4;
                let total_width: u32 = segments
                    .iter()
                    .map(|s| gui_renderer::measure_text(font, &s.base_text, pfs).0)
                    .sum();
                let total_height = gui_renderer::measure_text(font, " ", pfs).1;
                let ap = ui::calculate_anchor_position(anchor, shift, width, height);
                let (mut pen_x, y) =
                    ui::calculate_aligned_position(ap, total_width, total_height, align);
                for seg in segments {
                    let color = match seg.state {
                        UpperSegmentState::Correct => ui::CORRECT_COLOR,
                        UpperSegmentState::Incorrect => ui::INCORRECT_COLOR,
                        UpperSegmentState::Active => ui::ACTIVE_COLOR,
                        UpperSegmentState::Pending => ui::PENDING_COLOR,
                    };
                    gui_renderer::draw_text(
                        pixel_buf,
                        width,
                        font,
                        &seg.base_text,
                        (pen_x as f32, y as f32),
                        pfs,
                        color,
                    );
                    if let Some(ruby) = &seg.ruby_text {
                        let (bw, ..) = gui_renderer::measure_text(font, &seg.base_text, pfs);
                        let (rw, ..) = gui_renderer::measure_text(font, ruby, ruby_pfs);
                        let rx = pen_x as f32 + (bw as f32 - rw as f32) / 2.0;
                        let ry = y as f32 - ruby_pfs * 0.5;
                        gui_renderer::draw_text(
                            pixel_buf,
                            width,
                            font,
                            ruby,
                            (rx, ry),
                            ruby_pfs,
                            color,
                        );
                    }
                    pen_x += gui_renderer::measure_text(font, &seg.base_text, pfs).0 as i32;
                }
            }
            Renderable::TypingLower {
                segments,
                anchor,
                shift,
                align,
                font_size,
                target_line_total_width,
            } => {
                let pfs = calculate_pixel_font_size(font_size, width, height);
                let ruby_pfs = pfs * 0.3;
                let total_height = gui_renderer::measure_text(font, " ", pfs).1;
                let ap = ui::calculate_anchor_position(anchor, shift, width, height);
                let (mut pen_x, y) = ui::calculate_aligned_position(
                    ap,
                    target_line_total_width,
                    total_height,
                    align,
                );
                for seg in segments {
                    match seg {
                        LowerTypingSegment::Completed {
                            base_text,
                            ruby_text,
                            is_correct,
                            width: seg_width,
                            ..
                        } => {
                            let color = if is_correct {
                                ui::CORRECT_COLOR
                            } else {
                                ui::INCORRECT_COLOR
                            };
                            gui_renderer::draw_text(
                                pixel_buf,
                                width,
                                font,
                                &base_text,
                                (pen_x as f32, y as f32),
                                pfs,
                                color,
                            );
                            if let Some(ruby) = ruby_text {
                                let bw = seg_width;
                                let (rw, ..) = gui_renderer::measure_text(font, &ruby, ruby_pfs);
                                let rx = pen_x as f32 + (bw as f32 - rw as f32) / 2.0;
                                let ry = y as f32 - ruby_pfs * 0.5;
                                gui_renderer::draw_text(
                                    pixel_buf,
                                    width,
                                    font,
                                    &ruby,
                                    (rx, ry),
                                    ruby_pfs,
                                    color,
                                );
                            }
                            pen_x += seg_width as i32;
                        }
                        LowerTypingSegment::Active { elements } => {
                            for el in elements {
                                let (text, color) = match el {
                                    ActiveLowerElement::Typed {
                                        character,
                                        is_correct,
                                    } => (
                                        character.to_string(),
                                        if is_correct {
                                            ui::CORRECT_COLOR
                                        } else {
                                            ui::INCORRECT_COLOR
                                        },
                                    ),
                                    ActiveLowerElement::Cursor => {
                                        ("|".to_string(), ui::CURSOR_COLOR)
                                    }
                                    ActiveLowerElement::UnconfirmedInput(s) => {
                                        (s.clone(), ui::UNCONFIRMED_COLOR)
                                    }
                                    ActiveLowerElement::LastIncorrectInput(c) => {
                                        (c.to_string(), ui::WRONG_KEY_COLOR)
                                    }
                                };
                                gui_renderer::draw_text(
                                    pixel_buf,
                                    width,
                                    font,
                                    &text,
                                    (pen_x as f32, y as f32),
                                    pfs,
                                    color,
                                );
                                pen_x += gui_renderer::measure_text(font, &text, pfs).0 as i32;
                            }
                        }
                    }
                }
            }
            Renderable::ProgressBar {
                anchor,
                shift,
                width_ratio,
                height_ratio,
                progress,
                bg_color,
                fg_color,
            } => {
                let bw = (width as f32 * width_ratio) as u32;
                let bh = (height as f32 * height_ratio) as u32;
                let ap = ui::calculate_anchor_position(anchor, shift, width, height);
                let sx = ap.0 as usize;
                let sy = (ap.1 - bh as i32).max(0) as usize;
                gui_renderer::draw_rect(
                    pixel_buf,
                    width,
                    sx,
                    sy,
                    bw as usize,
                    bh as usize,
                    bg_color,
                );
                let fw = (bw as f32 * progress) as usize;
                if fw > 0 {
                    gui_renderer::draw_rect(pixel_buf, width, sx, sy, fw, bh as usize, fg_color);
                }
            }
        }
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

/// 実行バイナリのディレクトリまたはカレントディレクトリの `fonts/` からフォントファイルを読み込む
fn load_font_file(name: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let path = dir.join("fonts").join(name);
            if let Ok(data) = std::fs::read(&path) {
                return Ok(data);
            }
        }
    }
    let path = std::path::PathBuf::from("fonts").join(name);
    Ok(std::fs::read(&path)?)
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let japanese_font = FontVec::try_from_vec(load_font_file("YujiSyuku-Regular.ttf")?)
        .map_err(|e| e.to_string())?;
    let traditional_chinese_font =
        FontVec::try_from_vec(load_font_file("NotoSerifJP-Regular.ttf")?)
            .map_err(|e| e.to_string())?;
    let simplified_chinese_font = FontVec::try_from_vec(load_font_file("NotoSerifJP-Regular.ttf")?)
        .map_err(|e| e.to_string())?;

    let fonts = Fonts {
        japanese: japanese_font,
        traditional_chinese: Some(traditional_chinese_font),
        simplified_chinese: Some(simplified_chinese_font),
    };
    let app_state = Arc::new(Mutex::new(App::new(fonts)));
    {
        let mut a = app_state.lock().unwrap();
        a.on_event(AppEvent::ChangeScene);
    }

    let window = AppWindow::new()?;

    // --- コールバック: フリック文字 ---
    {
        let app = Arc::clone(&app_state);
        let win = window.as_weak();
        window.on_char_input(move |c| {
            let timestamp = crate::timestamp::now();
            let mut a = app.lock().unwrap();
            for ch in c.chars() {
                a.on_event(AppEvent::Char { c: ch, timestamp });
            }
            if let Some(w) = win.upgrade() {
                w.set_keyboard_visible(a.state == crate::app::AppState::Typing);
            }
        });
    }

    // --- コールバック: 特殊キー ---
    {
        let app = Arc::clone(&app_state);
        let win = window.as_weak();
        window.on_special_input(move |action| {
            let mut a = app.lock().unwrap();
            match action.as_str() {
                "Backspace" => a.on_event(AppEvent::Backspace),
                "Enter" => a.on_event(AppEvent::Enter),
                "Escape" => a.on_event(AppEvent::Escape),
                "Up" => a.on_event(AppEvent::Up),
                "Down" => a.on_event(AppEvent::Down),
                _ => {}
            }
            if let Some(w) = win.upgrade() {
                w.set_keyboard_visible(a.state == crate::app::AppState::Typing);
            }
        });
    }

    // --- 描画タイマー (~60fps) ---
    // canvas_physical_size() でキャンバスの物理ピクセルサイズを取得し描画する。
    // Slint 側は ImageFit.fill を使用しており、描画サイズと論理キャンバスの
    // アスペクト比が一致するため歪みは発生しない。
    let mut pixel_buf: Vec<u32> = Vec::new();
    let win_weak = window.as_weak();
    let app = Arc::clone(&app_state);
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
                let mut a = app.lock().unwrap();
                // 実測 delta_ms を渡すことで app.fps が正確な値になる
                a.update(w, h, delta_ms);
                render_frame(&a, w, h, &mut pixel_buf);
                win.set_keyboard_visible(a.state == crate::app::AppState::Typing);
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
    slint::android::init(android_app).unwrap();
    run().unwrap();
}
