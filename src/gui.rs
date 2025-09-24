// src/gui.rs

#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use crate::app::{App, AppEvent, Fonts}; // Fontsをインポート
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use crate::renderer::{calculate_pixel_font_size, draw_linear_gradient, gui_renderer};
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use crate::ui::{self, ActiveLowerElement, LowerTypingSegment, Renderable, UpperSegmentState};
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use ab_glyph::FontRef;
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use minifb::{Key, KeyRepeat, Window, WindowOptions};
#[cfg(not(feature = "uefi"))]
use std::time::Instant;

// ... (Windows固有の処理は変更なし)
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
use winapi::ctypes::c_void;
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
use winapi::shared::windef::HWND;
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
use winapi::um::dwmapi::DwmSetWindowAttribute;

#[cfg(all(target_os = "windows", not(feature = "uefi")))]
const DWMWA_CAPTION_COLOR: u32 = 35;
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
const DWMWA_TEXT_COLOR: u32 = 36;
#[cfg(all(target_os = "windows", not(feature = "uefi")))]
const DWMWA_BORDER_COLOR: u32 = 37;

#[cfg(all(target_os = "windows", not(feature = "uefi")))]
fn rgb_to_colorref(r: u8, g: u8, b: u8) -> u32 {
    ((b as u32) << 16) | ((g as u32) << 8) | (r as u32)
}

/// GUIアプリケーションのメイン関数
#[cfg(not(feature = "uefi"))]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // フォントの読み込み
    let yuji_font_data = include_bytes!("../fonts/YujiSyuku-Regular.ttf");
    let yuji_font = FontRef::try_from_slice(yuji_font_data).map_err(|_| "Failed to load Yuji Syuku font")?;
    
    let noto_font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let noto_font = FontRef::try_from_slice(noto_font_data).map_err(|_| "Failed to load Noto Serif JP font")?;

    let fonts = Fonts {
        yuji_syuku: yuji_font,
        noto_serif: noto_font,
    };

    let mut width = 800;
    let mut height = 500;

    let mut window = Window::new(
        "Neknaj Typing Multi-Platform",
        width, height,
        WindowOptions { resize: true, ..WindowOptions::default() },
    )?;
    window.set_target_fps(60);

    let mut app = App::new(fonts); // Appにフォントを渡す
    app.on_event(AppEvent::Start);

    let mut last_frame_time = Instant::now();

    while window.is_open() && !app.should_quit {
        let (new_width, new_height) = window.get_size();
        if new_width != width || new_height != height {
            width = new_width;
            height = new_height;
        }

        let now_time = Instant::now();
        let delta_time = now_time.duration_since(last_frame_time).as_millis() as f64;
        last_frame_time = now_time;

        handle_input(&mut window, &mut app);

        app.update(width, height, delta_time);

        let mut pixel_buffer = vec![0u32; width * height];
        let current_font = app.get_current_font(); // 現在のフォントを取得
        let render_list = ui::build_ui(&app, current_font, width, height);

        for item in render_list {
            match item {
                Renderable::Background { gradient } => {
                    draw_linear_gradient(&mut pixel_buffer, width, height, gradient.start_color, gradient.end_color, (0.0, 0.0), (width as f32, height as f32));
                }
                Renderable::BigText { text, anchor, shift, align, font_size, color } |
                Renderable::Text { text, anchor, shift, align, font_size, color } => {
                    let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let (text_width, text_height, _) = gui_renderer::measure_text(current_font, &text, pixel_font_size);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    gui_renderer::draw_text(&mut pixel_buffer, width, current_font, &text, (x as f32, y as f32), pixel_font_size, color);
                }
                Renderable::TypingUpper { segments, anchor, shift, align, font_size } => {
                    let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let ruby_pixel_font_size = pixel_font_size * 0.4;
                    
                    let total_width = segments.iter().map(|seg| {
                        gui_renderer::measure_text(current_font, &seg.base_text, pixel_font_size).0
                    }).sum::<u32>();
                    let total_height = gui_renderer::measure_text(current_font, " ", pixel_font_size).1;

                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (mut pen_x, y) = ui::calculate_aligned_position(anchor_pos, total_width, total_height, align);

                    for seg in segments {
                        let color = match seg.state {
                            UpperSegmentState::Correct => ui::CORRECT_COLOR,
                            UpperSegmentState::Incorrect => ui::INCORRECT_COLOR,
                            UpperSegmentState::Active => ui::ACTIVE_COLOR,
                            UpperSegmentState::Pending => ui::PENDING_COLOR,
                        };
                        gui_renderer::draw_text(&mut pixel_buffer, width, current_font, &seg.base_text, (pen_x as f32, y as f32), pixel_font_size, color);
                        
                        if let Some(ruby) = &seg.ruby_text {
                            let (base_w, ..) = gui_renderer::measure_text(current_font, &seg.base_text, pixel_font_size);
                            let (ruby_w, ..) = gui_renderer::measure_text(current_font, ruby, ruby_pixel_font_size);
                            let ruby_x = pen_x as f32 + (base_w as f32 - ruby_w as f32) / 2.0;
                            let ruby_y = y as f32 - ruby_pixel_font_size*0.5;
                            gui_renderer::draw_text(&mut pixel_buffer, width, current_font, ruby, (ruby_x, ruby_y), ruby_pixel_font_size, color);
                        }
                        
                        let (seg_width, _, _) = gui_renderer::measure_text(current_font, &seg.base_text, pixel_font_size);
                        pen_x += seg_width as i32;
                    }
                }
                Renderable::ProgressBar { anchor, shift, width_ratio, height_ratio, progress, bg_color, fg_color } => {
                    let bar_width = (width as f32 * width_ratio) as u32;
                    let bar_height = (height as f32 * height_ratio) as u32;

                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    // anchor_posが左下を指すので、描画開始Y座標を調整
                    let start_x = anchor_pos.0 as usize;
                    let start_y = (anchor_pos.1 - bar_height as i32).max(0) as usize;

                    // 背景を描画
                    gui_renderer::draw_rect(&mut pixel_buffer, width, start_x, start_y, bar_width as usize, bar_height as usize, bg_color);

                    // 前景（進捗）を描画
                    let fg_width = (bar_width as f32 * progress) as usize;
                    if fg_width > 0 {
                        gui_renderer::draw_rect(&mut pixel_buffer, width, start_x, start_y, fg_width, bar_height as usize, fg_color);
                    }
                }
                Renderable::TypingLower { segments, anchor, shift, align, font_size, target_line_total_width } => {
                    let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let ruby_pixel_font_size = pixel_font_size * 0.3;
                    let total_height = gui_renderer::measure_text(current_font, " ", pixel_font_size).1;

                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (mut pen_x, y) = ui::calculate_aligned_position(anchor_pos, target_line_total_width, total_height, align);

                    for seg in segments {
                        match seg {
                            LowerTypingSegment::Completed { base_text, ruby_text, is_correct } => {
                                let color = if is_correct { ui::CORRECT_COLOR } else { ui::INCORRECT_COLOR };
                                gui_renderer::draw_text(&mut pixel_buffer, width, current_font, &base_text, (pen_x as f32, y as f32), pixel_font_size, color);
                                
                                if let Some(ruby) = ruby_text {
                                    let (base_w, ..) = gui_renderer::measure_text(current_font, &base_text, pixel_font_size);
                                    let (ruby_w, ..) = gui_renderer::measure_text(current_font, &ruby, ruby_pixel_font_size);
                                    let ruby_x = pen_x as f32 + (base_w as f32 - ruby_w as f32) / 2.0;
                                    let ruby_y = y as f32 - ruby_pixel_font_size*0.5;
                                    gui_renderer::draw_text(&mut pixel_buffer, width, current_font, &ruby, (ruby_x, ruby_y), ruby_pixel_font_size, color);
                                }

                                pen_x += gui_renderer::measure_text(current_font, &base_text, pixel_font_size).0 as i32;
                            }
                            LowerTypingSegment::Active { elements } => {
                                for el in elements {
                                    let (text, color) = match el {
                                        ActiveLowerElement::Typed { character, is_correct } => (character.to_string(), if is_correct { ui::CORRECT_COLOR } else { ui::INCORRECT_COLOR }),
                                        ActiveLowerElement::Cursor => ("|".to_string(), ui::CURSOR_COLOR),
                                        ActiveLowerElement::UnconfirmedInput(s) => (s.clone(), ui::UNCONFIRMED_COLOR),
                                        ActiveLowerElement::LastIncorrectInput(c) => (c.to_string(), ui::WRONG_KEY_COLOR),
                                    };
                                    gui_renderer::draw_text(&mut pixel_buffer, width, current_font, &text, (pen_x as f32, y as f32), pixel_font_size, color);
                                    pen_x += gui_renderer::measure_text(current_font, &text, pixel_font_size).0 as i32;
                                }
                            }
                        }
                    }
                }
            }
        }
        window.update_with_buffer(&pixel_buffer, width, height)?;
    }
    Ok(())
}

#[cfg(not(feature = "uefi"))]
fn handle_input(window: &mut Window, app: &mut App) {
    if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
        app.on_event(AppEvent::Escape);
    }
    if window.is_key_pressed(Key::Tab, KeyRepeat::No) {
        app.on_event(AppEvent::CycleTuiMode);
    }
    for key in window.get_keys_pressed(KeyRepeat::Yes) {
        match key {
            Key::Up => app.on_event(AppEvent::Up),
            Key::Down => app.on_event(AppEvent::Down),
            Key::Backspace => app.on_event(AppEvent::Backspace),
            Key::Enter => app.on_event(AppEvent::Enter),
            _ => {
                if let Some(char_key) = key_to_char(key, window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift)) {
                    app.on_event(AppEvent::Char { c: char_key, timestamp: crate::timestamp::now() });
                }
            }
        }
    }
}

#[cfg(not(feature = "uefi"))]
fn key_to_char(key: Key, is_shift: bool) -> Option<char> {
    match (key, is_shift) {
        (Key::A, false) => Some('a'), (Key::A, true) => Some('A'),
        (Key::B, false) => Some('b'), (Key::B, true) => Some('B'),
        (Key::C, false) => Some('c'), (Key::C, true) => Some('C'),
        (Key::D, false) => Some('d'), (Key::D, true) => Some('D'),
        (Key::E, false) => Some('e'), (Key::E, true) => Some('E'),
        (Key::F, false) => Some('f'), (Key::F, true) => Some('F'),
        (Key::G, false) => Some('g'), (Key::G, true) => Some('G'),
        (Key::H, false) => Some('h'), (Key::H, true) => Some('H'),
        (Key::I, false) => Some('i'), (Key::I, true) => Some('I'),
        (Key::J, false) => Some('j'), (Key::J, true) => Some('J'),
        (Key::K, false) => Some('k'), (Key::K, true) => Some('K'),
        (Key::L, false) => Some('l'), (Key::L, true) => Some('L'),
        (Key::M, false) => Some('m'), (Key::M, true) => Some('M'),
        (Key::N, false) => Some('n'), (Key::N, true) => Some('N'),
        (Key::O, false) => Some('o'), (Key::O, true) => Some('O'),
        (Key::P, false) => Some('p'), (Key::P, true) => Some('P'),
        (Key::Q, false) => Some('q'), (Key::Q, true) => Some('Q'),
        (Key::R, false) => Some('r'), (Key::R, true) => Some('R'),
        (Key::S, false) => Some('s'), (Key::S, true) => Some('S'),
        (Key::T, false) => Some('t'), (Key::T, true) => Some('T'),
        (Key::U, false) => Some('u'), (Key::U, true) => Some('U'),
        (Key::V, false) => Some('v'), (Key::V, true) => Some('V'),
        (Key::W, false) => Some('w'), (Key::W, true) => Some('W'),
        (Key::X, false) => Some('x'), (Key::X, true) => Some('X'),
        (Key::Y, false) => Some('y'), (Key::Y, true) => Some('Y'),
        (Key::Z, false) => Some('z'), (Key::Z, true) => Some('Z'),
        (Key::Key0, false) => Some('0'), (Key::Key0, true) => None,
        (Key::Key1, false) => Some('1'), (Key::Key1, true) => Some('!'),
        (Key::Key2, false) => Some('2'), (Key::Key2, true) => Some('"'),
        (Key::Key3, false) => Some('3'), (Key::Key3, true) => Some('#'),
        (Key::Key4, false) => Some('4'), (Key::Key4, true) => Some('$'),
        (Key::Key5, false) => Some('5'), (Key::Key5, true) => Some('%'),
        (Key::Key6, false) => Some('6'), (Key::Key6, true) => Some('&'),
        (Key::Key7, false) => Some('7'), (Key::Key7, true) => Some('\''),
        (Key::Key8, false) => Some('8'), (Key::Key8, true) => Some('('),
        (Key::Key9, false) => Some('9'), (Key::Key9, true) => Some(')'),
        (Key::Space, _) => Some(' '),
        (Key::Comma, false) => Some(','), (Key::Comma, true) => Some('<'),
        (Key::Period, false) => Some('.'), (Key::Period, true) => Some('>'),
        (Key::Slash, false) => Some('/'), (Key::Slash, true) => Some('?'),
        (Key::Semicolon, false) => Some(';'), (Key::Semicolon, true) => Some(':'),
        (Key::Equal, false) => Some('='), (Key::Equal, true) => Some('+'),
        (Key::Minus, false) => Some('-'), (Key::Minus, true) => Some('_'),
        _ => None,
    }
}

#[cfg(feature = "uefi")]
pub fn run() -> Result<(), Box<dyn core::error::Error>> {
    Err("GUI is not supported in UEFI environment.".into())
}