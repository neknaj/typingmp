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

// --- Windows固有のインポートと処理 (変更なし) ---
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
                    // --- Define Colors based on old source ---
                    const CORRECT_COLOR: u32 = 0xFF_9097FF;
                    const INCORRECT_COLOR: u32 = 0xFF_FF9898;
                    const PENDING_COLOR: u32 = 0xFF_999999;
                    const WRONG_KEY_COLOR: u32 = 0xFF_F55252;
                    const CURSOR_COLOR: u32 = 0xFF_FFFFFF;

                    const CURSOR_TARGET_X: f32 = 0.3; // 30% from left
                    const RUBY_FONT_SIZE_RATIO: f32 = 0.4;
                    const RUBY_Y_OFFSET: f32 = -0.08; // Relative to window height

                    let base_pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let ruby_pixel_font_size = base_pixel_font_size * RUBY_FONT_SIZE_RATIO;
                    
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    
                    // --- Calculate cursor alignment offset ---
                    let mut typed_pixel_width = 0.0;
                    let mut target_found = false;

                    for (seg_idx, seg) in content_line.segments.iter().enumerate() {
                        if target_found { break; }
                        let reading = match seg {
                            Segment::Plain { text } => text.clone(),
                            Segment::Annotated { reading, .. } => reading.clone(),
                        };
                        for (char_idx, character) in reading.chars().enumerate() {
                             if seg_idx == status.segment as usize && char_idx == status.char_ as usize {
                                target_found = true;
                                break;
                            }
                            let text_to_measure = if let Segment::Annotated{..} = seg { &reading } else { &text_to_measure_from_seg(seg) };
                            let (char_w, _, _) = gui_renderer::measure_text(&font, &character.to_string(), if let Segment::Annotated{..} = seg { ruby_pixel_font_size } else { base_pixel_font_size });
                            typed_pixel_width += char_w as f32;
                        }
                    }

                    let start_x = anchor_pos.0 as f32 + (width as f32 * CURSOR_TARGET_X) - typed_pixel_width;

                    // --- PASS 1: Draw all text in PENDING_COLOR ---
                    let mut pen_x = start_x;
                    for seg in &content_line.segments {
                        match seg {
                            Segment::Plain { text } => {
                                let (seg_w, ..) = gui_renderer::measure_text(&font, text, base_pixel_font_size);
                                gui_renderer::draw_text(&mut pixel_buffer, width, &font, text, (pen_x, anchor_pos.1 as f32), base_pixel_font_size, PENDING_COLOR);
                                pen_x += seg_w as f32;
                            }
                            Segment::Annotated { base, reading } => {
                                let (base_w, ..) = gui_renderer::measure_text(&font, base, base_pixel_font_size);
                                let ruby_y = anchor_pos.1 as f32 + (height as f32 * RUBY_Y_OFFSET);
                                gui_renderer::draw_text(&mut pixel_buffer, width, &font, base, (pen_x, anchor_pos.1 as f32), base_pixel_font_size, PENDING_COLOR);
                                gui_renderer::draw_text(&mut pixel_buffer, width, &font, reading, (pen_x, ruby_y), ruby_pixel_font_size, PENDING_COLOR);
                                pen_x += base_w as f32;
                            }
                        }
                    }
                    
                    // --- PASS 2: Overlay typed characters with correct/incorrect colors ---
                    pen_x = start_x;
                    for (seg_idx, seg) in content_line.segments.iter().enumerate() {
                        let correctness_seg = &correctness_line.segments[seg_idx];
                        match seg {
                            Segment::Plain { text } => {
                                let mut char_pen_x = pen_x;
                                for (char_idx, character) in text.chars().enumerate() {
                                    let char_str = character.to_string();
                                    let (char_w, ..) = gui_renderer::measure_text(&font, &char_str, base_pixel_font_size);
                                    if seg_idx < status.segment as usize || (seg_idx == status.segment as usize && char_idx < status.char_ as usize) {
                                        let color = match correctness_seg.chars[char_idx] {
                                            TypingCorrectnessChar::Correct => CORRECT_COLOR,
                                            _ => INCORRECT_COLOR,
                                        };
                                        gui_renderer::draw_text(&mut pixel_buffer, width, &font, &char_str, (char_pen_x, anchor_pos.1 as f32), base_pixel_font_size, color);
                                    }
                                    char_pen_x += char_w as f32;
                                }
                                pen_x = char_pen_x;
                            }
                            Segment::Annotated { base, reading } => {
                                let (base_w, ..) = gui_renderer::measure_text(&font, base, base_pixel_font_size);
                                let ruby_y = anchor_pos.1 as f32 + (height as f32 * RUBY_Y_OFFSET);
                                let mut ruby_pen_x = pen_x;
                                for (char_idx, character) in reading.chars().enumerate() {
                                    let char_str = character.to_string();
                                    let (char_w, ..) = gui_renderer::measure_text(&font, &char_str, ruby_pixel_font_size);
                                     if seg_idx < status.segment as usize || (seg_idx == status.segment as usize && char_idx < status.char_ as usize) {
                                         let color = match correctness_seg.chars[char_idx] {
                                            TypingCorrectnessChar::Correct => CORRECT_COLOR,
                                            _ => INCORRECT_COLOR,
                                         };
                                         gui_renderer::draw_text(&mut pixel_buffer, width, &font, &char_str, (ruby_pen_x, ruby_y), ruby_pixel_font_size, color);
                                    }
                                    ruby_pen_x += char_w as f32;
                                }
                                pen_x += base_w as f32;
                            }
                        }
                    }

                    // --- PASS 3: Draw Cursor and Extras ---
                    let cursor_pen_x = start_x + typed_pixel_width;
                    let cursor_rect_y = anchor_pos.1 - (base_pixel_font_size / 2.0) as i32;
                    for y in cursor_rect_y..(cursor_rect_y + base_pixel_font_size as i32) {
                        for x in (cursor_pen_x as i32)..((cursor_pen_x as i32) + 2) {
                             if x >= 0 && x < width as i32 && y >= 0 && y < height as i32 {
                                pixel_buffer[y as usize * width + x as usize] = CURSOR_COLOR;
                             }
                        }
                    }

                    let extras_y = anchor_pos.1 as f32 + base_pixel_font_size * 0.2;
                    let extras_font_size = base_pixel_font_size * 0.7;

                    if !status.unconfirmed.is_empty() {
                         let unconfirmed_text: String = status.unconfirmed.iter().collect();
                         gui_renderer::draw_text(&mut pixel_buffer, width, &font, &unconfirmed_text, (cursor_pen_x + 5.0, extras_y), extras_font_size, PENDING_COLOR);
                    } else if let Some(wrong_char) = status.last_wrong_keydown {
                        let wrong_text = wrong_char.to_string();
                        gui_renderer::draw_text(&mut pixel_buffer, width, &font, &wrong_text, (cursor_pen_x + 5.0, extras_y), extras_font_size, WRONG_KEY_COLOR);
                    }
                }
            }
        }
        window.update_with_buffer(&pixel_buffer, width, height)?;
    }
    Ok(())
}

fn text_to_measure_from_seg(seg: &Segment) -> String {
    match seg {
        Segment::Plain { text } => text.clone(),
        Segment::Annotated { base, .. } => base.clone(),
    }
}

// (handle_input と key_to_char 関数は変更なし)
#[cfg(not(feature = "uefi"))]
fn handle_input(window: &mut Window, app: &mut App) {
    if window.is_key_pressed(Key::Escape, KeyRepeat::No) {
        app.on_event(AppEvent::Escape);
    }
    for key in window.get_keys_pressed(KeyRepeat::Yes) {
        match key {
            Key::Up => app.on_event(AppEvent::Up),
            Key::Down => app.on_event(AppEvent::Down),
            Key::Backspace => app.on_event(AppEvent::Backspace),
            Key::Enter => app.on_event(AppEvent::Enter),
            _ => {
                if let Some(char_key) = key_to_char(key, window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift)) {
                    app.on_event(AppEvent::Char(char_key));
                }
            }
        }
    }
}
#[cfg(not(feature = "uefi"))]
fn key_to_char(key: Key, is_shift: bool) -> Option<char> {
    // ... (implementation is unchanged)
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