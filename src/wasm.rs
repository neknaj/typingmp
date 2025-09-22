// src/wasm.rs

use crate::app::{App, AppEvent};
use crate::renderer::{calculate_pixel_font_size, gui_renderer};
use crate::ui::{self, Renderable};
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
            // ブラウザのデフォルト動作（フォーム送信など）を抑制する
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