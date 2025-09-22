// src/gui.rs

#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use crate::app::{App, AppEvent};
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use crate::model::{Segment, TypingCorrectnessChar};
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use crate::renderer::{calculate_pixel_font_size, draw_linear_gradient, gui_renderer};
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use crate::ui::{self, Renderable};
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use ab_glyph::FontRef;
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
use minifb::{Key, KeyRepeat, Window, WindowOptions};

// --- Windows固有のインポートと処理 ---
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
// --- End Windows固有のインポートと処理 ---

/// GUIアプリケーションのメイン関数
#[cfg(not(feature = "uefi"))]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data).map_err(|_| "Failed to load font from slice")?;

    let mut width = 800;
    let mut height = 500;

    let mut window = Window::new(
        "Neknaj Typing Multi-Platform",
        width, height,
        WindowOptions { resize: true, ..WindowOptions::default() },
    )?;
    window.set_target_fps(60);
    
    // (Windows固有のコードは変更なし)

    let mut app = App::new();
    app.on_event(AppEvent::Start);

    while window.is_open() && !app.should_quit {
        let (new_width, new_height) = window.get_size();
        if new_width != width || new_height != height {
            width = new_width;
            height = new_height;
        }

        handle_input(&mut window, &mut app);

        let mut pixel_buffer = vec![0u32; width * height];
        let render_list = ui::build_ui(&app);

        for item in render_list {
            match item {
                Renderable::Background { gradient } => {
                    draw_linear_gradient(
                        &mut pixel_buffer, width, height,
                        gradient.start_color, gradient.end_color,
                        (0.0, 0.0), (width as f32, height as f32),
                    );
                }
                Renderable::BigText { text, anchor, shift, align, font_size, color } |
                Renderable::Text { text, anchor, shift, align, font_size, color } => {
                    let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let (text_width, text_height, _) = gui_renderer::measure_text(&font, &text, pixel_font_size);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    gui_renderer::draw_text(&mut pixel_buffer, width, &font, &text, (x as f32, y as f32), pixel_font_size, color);
                }
                Renderable::TypingText { content_line, correctness_line, status, anchor, shift, font_size } => {
                    const CURSOR_TARGET_X: f32 = 0.3; // 30% from left
                    const RUBY_FONT_SIZE_RATIO: f32 = 0.4;
                    const RUBY_Y_OFFSET: f32 = -0.08; // Relative to window height

                    let base_pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let ruby_pixel_font_size = base_pixel_font_size * RUBY_FONT_SIZE_RATIO;
                    
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);

                    // 1. Calculate the pixel width of the text already typed to align the cursor
                    let mut typed_pixel_width = 0.0;
                    let mut target_found = false;
                    for (seg_idx, seg) in content_line.segments.iter().enumerate() {
                        if target_found { break; }
                        let reading = match seg {
                            Segment::Plain { text } => text,
                            Segment::Annotated { reading, .. } => reading,
                        };
                        for char_idx in 0..reading.chars().count() {
                            if seg_idx == status.segment as usize && char_idx == status.char_ as usize {
                                target_found = true;
                                break;
                            }
                            let (char_w, _, _) = gui_renderer::measure_text(&font, &reading.chars().nth(char_idx).unwrap_or(' ').to_string(), base_pixel_font_size);
                            typed_pixel_width += char_w as f32;
                        }
                    }

                    let start_x = anchor_pos.0 as f32 + (width as f32 * CURSOR_TARGET_X) - typed_pixel_width;
                    let mut pen_x = start_x;

                    // 2. Draw the full line (base text + ruby overlays)
                    for (seg_idx, seg) in content_line.segments.iter().enumerate() {
                        let correctness_seg = &correctness_line.segments[seg_idx];
                        
                        match seg {
                            Segment::Plain { text } => {
                                // Draw gray background text
                                gui_renderer::draw_text(&mut pixel_buffer, width, &font, text, (pen_x, anchor_pos.1 as f32), base_pixel_font_size, 0xFF_AAAAAA);
                                
                                // Overlay colored text
                                let mut char_pen_x = pen_x;
                                for (char_idx, character) in text.chars().enumerate() {
                                    let color = match correctness_seg.chars[char_idx] {
                                        TypingCorrectnessChar::Correct => 0xFF_22FF22,
                                        TypingCorrectnessChar::Incorrect => 0xFF_FF2222,
                                        _ => 0,
                                    };
                                    if color != 0 {
                                        let char_str = character.to_string();
                                        gui_renderer::draw_text(&mut pixel_buffer, width, &font, &char_str, (char_pen_x, anchor_pos.1 as f32), base_pixel_font_size, color);
                                    }
                                    let (char_w, _, _) = gui_renderer::measure_text(&font, &character.to_string(), base_pixel_font_size);
                                    char_pen_x += char_w as f32;
                                }
                                let (seg_w, _, _) = gui_renderer::measure_text(&font, text, base_pixel_font_size);
                                pen_x += seg_w as f32;
                            }
                            Segment::Annotated { base, reading } => {
                                let ruby_y = anchor_pos.1 as f32 + (height as f32 * RUBY_Y_OFFSET);
                                // Draw base and ruby background text
                                gui_renderer::draw_text(&mut pixel_buffer, width, &font, base, (pen_x, anchor_pos.1 as f32), base_pixel_font_size, 0xFF_AAAAAA);
                                gui_renderer::draw_text(&mut pixel_buffer, width, &font, reading, (pen_x, ruby_y), ruby_pixel_font_size, 0xFF_AAAAAA);
                                
                                // Overlay colored ruby text
                                let mut ruby_pen_x = pen_x;
                                for (char_idx, character) in reading.chars().enumerate() {
                                    let color = match correctness_seg.chars[char_idx] {
                                        TypingCorrectnessChar::Correct => 0xFF_22FF22,
                                        TypingCorrectnessChar::Incorrect => 0xFF_FF2222,
                                        _ => 0,
                                    };
                                    if color != 0 {
                                        let char_str = character.to_string();
                                        gui_renderer::draw_text(&mut pixel_buffer, width, &font, &char_str, (ruby_pen_x, ruby_y), ruby_pixel_font_size, color);
                                    }
                                    let (char_w, _, _) = gui_renderer::measure_text(&font, &character.to_string(), ruby_pixel_font_size);
                                    ruby_pen_x += char_w as f32;
                                }
                                let (seg_w, _, _) = gui_renderer::measure_text(&font, base, base_pixel_font_size);
                                pen_x += seg_w as f32;
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

// (handle_input と key_to_char 関数は変更なし)
/// キーボード入力を処理する
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
fn handle_input(window: &mut Window, app: &mut App) {
    // Escapeキーは単発で処理する
    if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
        app.on_event(AppEvent::Escape);
    }

    // 他のキーはリピートを許可して処理する
    for key in window.get_keys_pressed(KeyRepeat::Yes) {
        match key {
            Key::Up => app.on_event(AppEvent::Up),
            Key::Down => app.on_event(AppEvent::Down),
            Key::Backspace => app.on_event(AppEvent::Backspace),
            Key::Enter => app.on_event(AppEvent::Enter),
            _ => {
                // Keyをcharに変換してイベントを発行する
                if let Some(char_key) = key_to_char(
                    key,
                    window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift),
                ) {
                    app.on_event(AppEvent::Char(char_key));
                }
            }
        }
    }
}

// キーコードを文字に変換するヘルパー関数
#[cfg(not(feature = "uefi"))] // Only compile if uefi feature is NOT enabled
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
        (Key::Key0, false) => Some('0'), (Key::Key0, true) => Some(')'),
        (Key::Key1, false) => Some('1'), (Key::Key1, true) => Some('!'),
        (Key::Key2, false) => Some('2'), (Key::Key2, true) => Some('@'),
        (Key::Key3, false) => Some('3'), (Key::Key3, true) => Some('#'),
        (Key::Key4, false) => Some('4'), (Key::Key4, true) => Some('$'),
        (Key::Key5, false) => Some('5'), (Key::Key5, true) => Some('%'),
        (Key::Key6, false) => Some('6'), (Key::Key6, true) => Some('^'),
        (Key::Key7, false) => Some('7'), (Key::Key7, true) => Some('&'),
        (Key::Key8, false) => Some('8'), (Key::Key8, true) => Some('*'),
        (Key::Key9, false) => Some('9'), (Key::Key9, true) => Some('('),
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