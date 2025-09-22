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

// (初期化処理、リサイズ、キーイベントリスナは変更なし)

#[wasm_bindgen(start)]
#[cfg(feature = "wasm")]
pub fn start() -> Result<(), JsValue> {
    // ... (既存のセットアップコード) ...

    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    *g.borrow_mut() = Some(Closure::<dyn FnMut()>::new(move || {
        let (width, height) = *size.borrow();
        let app = app.borrow();

        let mut pixel_buffer = vec![0u32; width * height];
        // build_uiにfontとサイズを渡す
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