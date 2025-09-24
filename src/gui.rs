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
use minifb::{Key, KeyRepeat, Window, WindowOptions, InputCallback};
#[cfg(not(feature = "uefi"))]
use std::time::Instant;
#[cfg(not(feature = "uefi"))]
use std::cell::RefCell;
#[cfg(not(feature = "uefi"))]
use std::rc::Rc;


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


#[cfg(not(feature = "uefi"))]
struct AppInputHandler {
    app: Rc<RefCell<App<'static>>>,
}

#[cfg(not(feature = "uefi"))]
impl InputCallback for AppInputHandler {
    fn add_char(&mut self, c: u32) {
        if let Some(character) = std::char::from_u32(c) {
             self.app.borrow_mut().on_event(AppEvent::Char {
                c: character,
                timestamp: crate::timestamp::now(),
            });
        }
    }
}

/// GUIアプリケーションのメイン関数
#[cfg(not(feature = "uefi"))]
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    // フォントの読み込み
    let yuji_font_data: &'static [u8] = include_bytes!("../fonts/YujiSyuku-Regular.ttf");
    let yuji_font = FontRef::try_from_slice(yuji_font_data).map_err(|_| "Failed to load Yuji Syuku font")?;
    
    let noto_font_data: &'static [u8] = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
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

    let app = Rc::new(RefCell::new(App::new(fonts)));
    app.borrow_mut().on_event(AppEvent::Start);

    let input_handler = AppInputHandler { app: app.clone() };
    window.set_input_callback(Box::new(input_handler));

    let mut last_frame_time = Instant::now();

    while window.is_open() && !app.borrow().should_quit {
        let (new_width, new_height) = window.get_size();
        if new_width != width || new_height != height {
            width = new_width;
            height = new_height;
        }

        let now_time = Instant::now();
        let delta_time = now_time.duration_since(last_frame_time).as_millis() as f64;
        last_frame_time = now_time;

        handle_input(&mut window, &mut app.borrow_mut());

        app.borrow_mut().update(width, height, delta_time);

        let mut pixel_buffer = vec![0u32; width * height];
        
        // 描画処理は不変借用で行う
        let app_borrow = app.borrow();
        let current_font = app_borrow.get_current_font();
        let render_list = ui::build_ui(&app_borrow, current_font, width, height);


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
    // 繰り返しを伴うキーイベント
    for key in window.get_keys_pressed(KeyRepeat::Yes) {
        match key {
            Key::Backspace => app.on_event(AppEvent::Backspace),
            _ => {},
        }
    }
    // 繰り返しを伴わないキーイベント
    for key in window.get_keys_pressed(KeyRepeat::No) {
        match key {
            Key::Escape => app.on_event(AppEvent::Escape),
            Key::Tab => app.on_event(AppEvent::CycleTuiMode),
            Key::Up => app.on_event(AppEvent::Up),
            Key::Down => app.on_event(AppEvent::Down),
            Key::Enter => app.on_event(AppEvent::Enter),
            _ => {}
        }
    }
}


#[cfg(feature = "uefi")]
pub fn run() -> Result<(), Box<dyn core::error::Error>> {
    Err("GUI is not supported in UEFI environment.".into())
}