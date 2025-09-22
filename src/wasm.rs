// src/wasm.rs

use crate::app::{App, AppEvent};
use crate::model::{Segment, TypingCorrectnessChar};
use crate::renderer::{calculate_pixel_font_size, gui_renderer};
use crate::ui::{self, Renderable};
use ab_glyph::FontRef;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use web_sys::{CanvasRenderingContext2d, ImageData, KeyboardEvent};

/// WASMモジュールのエントリーポイント
#[wasm_bindgen(start)]
#[cfg(feature = "wasm")]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let body = document.body().unwrap();

    let canvas = document.create_element("canvas")?.dyn_into::<web_sys::HtmlCanvasElement>()?;
    body.append_child(&canvas)?;
    let context = canvas.get_context("2d")?.unwrap().dyn_into::<CanvasRenderingContext2d>()?;

    let font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data).map_err(|e| JsValue::from_str(&e.to_string()))?;

    let app = Rc::new(RefCell::new(App::new()));
    app.borrow_mut().on_event(AppEvent::Start);

    let size = Rc::new(RefCell::new((0, 0)));
    
    // (リサイズハンドラ、キーボードイベントリスナは変更なし)

    // requestAnimationFrameによるメインループ
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    *g.borrow_mut() = Some(Closure::<dyn FnMut()>::new(move || {
        let (width, height) = *size.borrow();
        let app = app.borrow();

        let mut pixel_buffer = vec![0u32; width * height];
        let render_list = ui::build_ui(&app);

        for item in render_list {
            match item {
                 Renderable::Background { gradient } => {
                    crate::renderer::draw_linear_gradient(
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
                    const CURSOR_TARGET_X: f32 = 0.3;
                    const RUBY_FONT_SIZE_RATIO: f32 = 0.4;
                    const RUBY_Y_OFFSET: f32 = -0.08;

                    let base_pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let ruby_pixel_font_size = base_pixel_font_size * RUBY_FONT_SIZE_RATIO;
                    
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);

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

                    for (seg_idx, seg) in content_line.segments.iter().enumerate() {
                        let correctness_seg = &correctness_line.segments[seg_idx];
                        
                        match seg {
                            Segment::Plain { text } => {
                                gui_renderer::draw_text(&mut pixel_buffer, width, &font, text, (pen_x, anchor_pos.1 as f32), base_pixel_font_size, 0xFF_AAAAAA);
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
                                gui_renderer::draw_text(&mut pixel_buffer, width, &font, base, (pen_x, anchor_pos.1 as f32), base_pixel_font_size, 0xFF_AAAAAA);
                                gui_renderer::draw_text(&mut pixel_buffer, width, &font, reading, (pen_x, ruby_y), ruby_pixel_font_size, 0xFF_AAAAAA);
                                
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
        
        // (Canvasへの転送処理は変更なし)

    }));
    request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}

// (request_animation_frameヘルパー関数は変更なし)