// src/wasm.rs

use crate::app::{App, AppEvent, Fonts};
use crate::renderer::{calculate_pixel_font_size, gui_renderer};
use crate::ui::{self, ActiveLowerElement, LowerTypingSegment, Renderable, UpperSegmentState};
use ab_glyph::FontRef;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlInputElement, ImageData, InputEvent, KeyboardEvent};

thread_local! {
    static APP_INSTANCE: RefCell<Option<Rc<RefCell<App<'static>>>>> = RefCell::new(None);
}

// --- デバッグ用のログ出力ヘルパー関数を追加 ---
#[cfg(debug_assertions)]
fn debug_log(message: &str) {
    crate::wasm_debug_logger::log(message);
}
#[cfg(not(debug_assertions))]
fn debug_log(_message: &str) {
    // リリースビルドでは何もしない
}

#[wasm_bindgen]
pub fn trigger_event(event_type: &str) {
    debug_log(&format!("Triggered event: {}", event_type));
    APP_INSTANCE.with(|instance| {
        if let Some(app_rc) = instance.borrow().as_ref() {
            let mut app = app_rc.borrow_mut();
            match event_type {
                "Up" => app.on_event(AppEvent::Up),
                "Down" => app.on_event(AppEvent::Down),
                "Enter" => app.on_event(AppEvent::Enter),
                "Backspace" => app.on_event(AppEvent::Backspace),
                "Escape" => app.on_event(AppEvent::Escape),
                _ => {},
            }
        }
    });
}


#[wasm_bindgen(start)]
#[cfg(feature = "wasm")]
pub fn start() -> Result<(), JsValue> {
    #[cfg(debug_assertions)]
    {
        crate::wasm_debug_logger::init();
    }
    
    debug_log("Application starting.");

    console_error_panic_hook::set_once();
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let body = document.body().unwrap();

    let input_element = document
        .create_element("input")?
        .dyn_into::<HtmlInputElement>()?;
    input_element.set_type("text");
    {
        input_element.set_attribute("inputmode", "text")?;
        input_element.set_attribute("autocapitalize", "off")?;
        input_element.set_attribute("autocorrect", "off")?;
        input_element.set_attribute("autocomplete", "off")?;
        input_element.set_attribute("spellcheck", "false")?;
    }
    body.append_child(&input_element)?;

    let wrapper = document
        .get_element_by_id("canvas-wrapper")
        .ok_or_else(|| JsValue::from_str("Missing #canvas-wrapper element"))?;

    let canvas = document.create_element("canvas")?.dyn_into::<web_sys::HtmlCanvasElement>()?;
    
    wrapper.append_child(&canvas)?;
    
    let context = canvas.get_context("2d")?.unwrap().dyn_into::<CanvasRenderingContext2d>()?;
    
    let yuji_font_data: &'static [u8] = include_bytes!("../fonts/YujiSyuku-Regular.ttf");
    let yuji_font = FontRef::try_from_slice(yuji_font_data)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let noto_font_data: &'static [u8] = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let noto_font = FontRef::try_from_slice(noto_font_data)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let fonts = Fonts {
        yuji_syuku: yuji_font,
        noto_serif: noto_font,
    };

    let app = Rc::new(RefCell::new(App::new(fonts)));
    app.borrow_mut().on_event(AppEvent::Start);

    APP_INSTANCE.with(|instance| {
        *instance.borrow_mut() = Some(app.clone());
    });

    let size = Rc::new(RefCell::new((0, 0)));
    let last_time = Rc::new(RefCell::new(0.0));

    // canvasクリックでinput要素にフォーカスを当てるリスナー
    {
        let input_clone = input_element.clone();
        let closure = Closure::<dyn FnMut()>::new(move || {
            let _ = input_clone.focus();
        });
        canvas.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // ウィンドウリサイズ時の処理
    {
        let canvas_clone = canvas.clone();
        let size_clone = size.clone();
        let resize_closure = Closure::<dyn FnMut()>::new(move || {
            let width = canvas_clone.client_width() as u32;
            let height = canvas_clone.client_height() as u32;
            canvas_clone.set_width(width);
            canvas_clone.set_height(height);
            *size_clone.borrow_mut() = (width as usize, height as usize);
        });
        window.add_event_listener_with_callback("resize", resize_closure.as_ref().unchecked_ref())?;
        resize_closure.as_ref().unchecked_ref::<js_sys::Function>().call0(&JsValue::NULL).unwrap();
        resize_closure.forget();
    }
    
    // キー入力イベント（特殊キー用）
    {
        let app_clone = app.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: KeyboardEvent| {
            // 生のKeyboardEventの内容をログに出力
            debug_log(&format!(
                "[KeyDown wasm.rs] key: '{}', code: '{}', composing: {}",
                event.key(),
                event.code(),
                event.is_composing()
            ));

            let mut app = app_clone.borrow_mut();
            
            match event.key().as_str() {
                "ArrowUp" => {
                    event.prevent_default();
                    app.on_event(AppEvent::Up)
                },
                "ArrowDown" => {
                    event.prevent_default();
                    app.on_event(AppEvent::Down)
                },
                "Backspace" => {
                    event.prevent_default();
                    app.on_event(AppEvent::Backspace)
                },
                "Enter" => {
                    event.prevent_default();
                    app.on_event(AppEvent::Enter)
                },
                "Escape" => {
                    event.prevent_default();
                    app.on_event(AppEvent::Escape)
                },
                _ => {}
            }
        });
        document.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // input要素の入力イベント（文字入力用）
    {
        let app_clone = app.clone();
        let input_clone = input_element.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: InputEvent| {
            // 生のInputEventの内容をログに出力
            debug_log(&format!(
                "[InputEvent wasm.rs] type: '{}', data: '{:?}', composing: {}, value: '{}'",
                event.input_type(),
                event.data(),
                event.is_composing(),
                input_clone.value()
            ));

            event.prevent_default();

            // input要素の全内容(value)ではなく、イベントで追加された文字(data)のみを処理する
            if let Some(data) = event.data() {
                if !data.is_empty() {
                    let mut app = app_clone.borrow_mut();
                    for c in data.chars() {
                        app.on_event(AppEvent::Char { c, timestamp: crate::timestamp::now() });
                    }
                }
            }
        });
        input_element.add_event_listener_with_callback("input", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    let ime_input_element = input_element.clone();

    // メインループ（アニメーションフレーム）
    *g.borrow_mut() = Some(Closure::<dyn FnMut()>::new(move || {
        let (width, height) = *size.borrow();
        
        if width == 0 || height == 0 {
            request_animation_frame(f.borrow().as_ref().unwrap());
            return;
        }

        let now = crate::timestamp::now();
        let mut last_time_borrow = last_time.borrow_mut();
        let delta_time = if *last_time_borrow > 0.0 { now - *last_time_borrow } else { 16.6 };
        *last_time_borrow = now;

        app.borrow_mut().update(width, height, delta_time);

        // --- 描画処理（不変借用） ---
        {
            let app_borrow = app.borrow();
            let current_font = app_borrow.get_current_font();

            let mut pixel_buffer = vec![0u32; width * height];
            let render_list = ui::build_ui(&app_borrow, current_font, width, height);

            for item in render_list {
                match item {
                    Renderable::Background { gradient } => {
                        crate::renderer::draw_linear_gradient(&mut pixel_buffer, width, height, gradient.start_color, gradient.end_color, (0.0, 0.0), (width as f32, height as f32));
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
        }

        // --- IMEリセット処理（可変借用） ---
        let mut app_borrow_mut = app.borrow_mut();
        if app_borrow_mut.should_reset_ime {
            let _ = ime_input_element.blur();
            let _ = ime_input_element.focus();
            app_borrow_mut.should_reset_ime = false;
        }

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