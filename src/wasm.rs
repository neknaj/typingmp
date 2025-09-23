// src/wasm.rs

use crate::app::{App, AppEvent};
use crate::renderer::{calculate_pixel_font_size, gui_renderer};
use crate::ui::{self, ActiveLowerElement, LowerTypingSegment, Renderable, UpperSegmentState};
use ab_glyph::FontRef;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use web_sys::{CanvasRenderingContext2d, ImageData, KeyboardEvent};

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

    let font = FontRef::try_from_slice(include_bytes!("../fonts/NotoSerifJP-Regular.ttf"))
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let app = Rc::new(RefCell::new(App::new()));
    app.borrow_mut().on_event(AppEvent::Start);

    let size = Rc::new(RefCell::new((0, 0)));

    // 初期サイズ設定とリサイズハンドラ
    {
        let window_clone = window.clone();
        let canvas_clone = canvas.clone();
        let size_clone = size.clone();
        let resize_handler = Closure::<dyn FnMut()>::new(move || {
            let width = window_clone.inner_width().unwrap().as_f64().unwrap() as u32;
            let height = window_clone.inner_height().unwrap().as_f64().unwrap() as u32;
            canvas_clone.set_width(width);
            canvas_clone.set_height(height);
            *size_clone.borrow_mut() = (width as usize, height as usize);
        });
        window.add_event_listener_with_callback("resize", resize_handler.as_ref().unchecked_ref())?;
        
        let width = window.inner_width().unwrap().as_f64().unwrap() as u32;
        let height = window.inner_height().unwrap().as_f64().unwrap() as u32;
        canvas.set_width(width);
        canvas.set_height(height);
        *size.borrow_mut() = (width as usize, height as usize);
        
        resize_handler.forget();
    }

    // キーボードイベントのリスナーを設定
    {
        let app_clone = app.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: KeyboardEvent| {
            event.prevent_default();

            let mut app = app_clone.borrow_mut();
            match event.key().as_str() {
                "ArrowUp" => app.on_event(AppEvent::Up),
                "ArrowDown" => app.on_event(AppEvent::Down),
                "Backspace" => app.on_event(AppEvent::Backspace),
                "Enter" => app.on_event(AppEvent::Enter),
                "Escape" => app.on_event(AppEvent::Escape),
                key if key.len() == 1 => app.on_event(AppEvent::Char { c: key.chars().next().unwrap(), timestamp: crate::timestamp::now() }),
                _ => {}
            }
        });
        document.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // requestAnimationFrameによるメインループ
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    *g.borrow_mut() = Some(Closure::<dyn FnMut()>::new(move || {
        let (width, height) = *size.borrow();

        app.borrow_mut().update(width, height, &font);

        let app = app.borrow();

        let mut pixel_buffer = vec![0u32; width * height];
        let render_list = ui::build_ui(&app, &font, width, height);

        for item in render_list {
            match item {
                Renderable::Background { gradient } => {
                    crate::renderer::draw_linear_gradient(&mut pixel_buffer, width, height, gradient.start_color, gradient.end_color, (0.0, 0.0), (width as f32, height as f32));
                }
                Renderable::BigText { text, anchor, shift, align, font_size, color } |
                Renderable::Text { text, anchor, shift, align, font_size, color } => {
                    let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let (text_width, text_height, _) = gui_renderer::measure_text(&font, &text, pixel_font_size);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    gui_renderer::draw_text(&mut pixel_buffer, width, &font, &text, (x as f32, y as f32), pixel_font_size, color);
                }
                Renderable::TypingUpper { segments, anchor, shift, align, font_size } => {
                    let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let ruby_pixel_font_size = pixel_font_size * 0.4;
                    
                    let total_width = segments.iter().map(|seg| {
                        gui_renderer::measure_text(&font, &seg.base_text, pixel_font_size).0
                    }).sum::<u32>();
                    let total_height = gui_renderer::measure_text(&font, " ", pixel_font_size).1;

                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (mut pen_x, y) = ui::calculate_aligned_position(anchor_pos, total_width, total_height, align);

                    for seg in segments {
                        let color = match seg.state {
                            UpperSegmentState::Correct => ui::CORRECT_COLOR,
                            UpperSegmentState::Incorrect => ui::INCORRECT_COLOR,
                            UpperSegmentState::Active => ui::ACTIVE_COLOR,
                            UpperSegmentState::Pending => ui::PENDING_COLOR,
                        };
                        gui_renderer::draw_text(&mut pixel_buffer, width, &font, &seg.base_text, (pen_x as f32, y as f32), pixel_font_size, color);
                        
                        if let Some(ruby) = &seg.ruby_text {
                            let (base_w, ..) = gui_renderer::measure_text(&font, &seg.base_text, pixel_font_size);
                            let (ruby_w, ..) = gui_renderer::measure_text(&font, ruby, ruby_pixel_font_size);
                            let ruby_x = pen_x as f32 + (base_w as f32 - ruby_w as f32) / 2.0;
                            let ruby_y = y as f32 - ruby_pixel_font_size;
                            gui_renderer::draw_text(&mut pixel_buffer, width, &font, ruby, (ruby_x, ruby_y), ruby_pixel_font_size, color);
                        }
                        
                        let (seg_width, _, _) = gui_renderer::measure_text(&font, &seg.base_text, pixel_font_size);
                        pen_x += seg_width as i32;
                    }
                }
                Renderable::TypingLower { segments, anchor, shift, align, font_size, target_line_total_width } => {
                    let pixel_font_size = calculate_pixel_font_size(font_size, width, height);
                    let ruby_pixel_font_size = pixel_font_size * 0.3;
                    let total_height = gui_renderer::measure_text(&font, " ", pixel_font_size).1;

                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, width, height);
                    let (mut pen_x, y) = ui::calculate_aligned_position(anchor_pos, target_line_total_width, total_height, align);

                    for seg in segments {
                        match seg {
                            LowerTypingSegment::Completed { base_text, ruby_text, is_correct } => {
                                let color = if is_correct { ui::CORRECT_COLOR } else { ui::INCORRECT_COLOR };
                                gui_renderer::draw_text(&mut pixel_buffer, width, &font, &base_text, (pen_x as f32, y as f32), pixel_font_size, color);

                                if let Some(ruby) = ruby_text {
                                    // FIX: &base_text と &ruby を渡す
                                    let (base_w, ..) = gui_renderer::measure_text(&font, &base_text, pixel_font_size);
                                    let (ruby_w, ..) = gui_renderer::measure_text(&font, &ruby, ruby_pixel_font_size);
                                    let ruby_x = pen_x as f32 + (base_w as f32 - ruby_w as f32) / 2.0;
                                    let ruby_y = y as f32 - ruby_pixel_font_size;
                                    // FIX: &ruby を渡す
                                    gui_renderer::draw_text(&mut pixel_buffer, width, &font, &ruby, (ruby_x, ruby_y), ruby_pixel_font_size, color);
                                }
                                
                                // FIX: &base_text を渡す
                                pen_x += gui_renderer::measure_text(&font, &base_text, pixel_font_size).0 as i32;
                            }
                            LowerTypingSegment::Active { elements } => {
                                for el in elements {
                                    let (text, color) = match el {
                                        ActiveLowerElement::Typed { character, is_correct } => (character.to_string(), if is_correct { ui::CORRECT_COLOR } else { ui::INCORRECT_COLOR }),
                                        ActiveLowerElement::Cursor => ("|".to_string(), ui::CURSOR_COLOR),
                                        ActiveLowerElement::UnconfirmedInput(s) => (s.clone(), ui::UNCONFIRMED_COLOR),
                                        ActiveLowerElement::LastIncorrectInput(c) => (c.to_string(), ui::WRONG_KEY_COLOR),
                                    };
                                    gui_renderer::draw_text(&mut pixel_buffer, width, &font, &text, (pen_x as f32, y as f32), pixel_font_size, color);
                                    pen_x += gui_renderer::measure_text(&font, &text, pixel_font_size).0 as i32;
                                }
                            }
                        }
                    }
                }
             }
        }
        
        let mut u8_buffer = Vec::with_capacity(width * height * 4);
        for pixel in pixel_buffer.iter() {
            u8_buffer.extend_from_slice(&[
                ((*pixel >> 16) & 0xFF) as u8,
                ((*pixel >> 8) & 0xFF) as u8,
                (*pixel & 0xFF) as u8,
                255,
            ]);
        }
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(
            Clamped(&u8_buffer),
            width as u32,
            height as u32,
        )
        .unwrap();
        context.put_image_data(&image_data, 0.0, 0.0).unwrap();

        request_animation_frame(f.borrow().as_ref().unwrap());
    }));
    request_animation_frame(g.borrow().as_ref().unwrap());
    Ok(())
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .unwrap()
        .request_animation_frame(f.as_ref().unchecked_ref())
        .unwrap();
}