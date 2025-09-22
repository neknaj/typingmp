use crate::app::App;
use crate::renderer::{gui_renderer, BG_COLOR};
use crate::ui::{self, Renderable};
use ab_glyph::{Font, FontRef};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use web_sys::{CanvasRenderingContext2d, ImageData, KeyboardEvent};

const WIDTH: usize = 800;
const HEIGHT: usize = 100;
const BIG_FONT_SIZE: f32 = 48.0;
const NORMAL_FONT_SIZE: f32 = 16.0;

/// WASMモジュールのエントリーポイント
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();
    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();
    let body = document.body().unwrap();

    let canvas = document.create_element("canvas")?.dyn_into::<web_sys::HtmlCanvasElement>()?;
    canvas.set_width(WIDTH as u32);
    canvas.set_height(HEIGHT as u32);
    body.append_child(&canvas)?;
    let context = canvas.get_context("2d")?.unwrap().dyn_into::<CanvasRenderingContext2d>()?;

    let font = FontRef::try_from_slice(include_bytes!("../fonts/NotoSerifJP-Regular.ttf"))
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let app = Rc::new(RefCell::new(App::new()));

    // キーボードイベントのリスナーを設定
    {
        let app_clone = app.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: KeyboardEvent| {
            let mut app = app_clone.borrow_mut();
            match event.key().as_str() {
                "Backspace" => app.on_backspace(),
                "Enter" => app.on_key('\n'),
                key if key.len() == 1 => app.on_key(key.chars().next().unwrap()),
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
        let app = app.borrow(); // 描画中は不変借用

        // 1. 背景色でピクセルバッファをクリア
        let mut pixel_buffer = vec![BG_COLOR; WIDTH * HEIGHT];

        // 2. UI定義から描画リストを取得
        let render_list = ui::build_ui(&app);

        // 3. 描画リストを解釈して描画
        for item in render_list {
            match item {
                 Renderable::BigText { text, anchor, shift, align } => {
                    let (text_width, text_height) = gui_renderer::measure_text(&font, text, BIG_FONT_SIZE);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, WIDTH, HEIGHT);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    gui_renderer::draw_text(
                        &mut pixel_buffer, WIDTH, &font, text,
                        (x as f32, y as f32), BIG_FONT_SIZE,
                    );
                }
                Renderable::Text { text, anchor, shift, align } => {
                    let (text_width, text_height) = gui_renderer::measure_text(&font, text, NORMAL_FONT_SIZE);
                    let anchor_pos = ui::calculate_anchor_position(anchor, shift, WIDTH, HEIGHT);
                    let (x, y) = ui::calculate_aligned_position(anchor_pos, text_width, text_height, align);
                    gui_renderer::draw_text(
                        &mut pixel_buffer, WIDTH, &font, text,
                        (x as f32, y as f32), NORMAL_FONT_SIZE,
                    );
                }
            }
        }
        
        // 4. 完成したバッファをCanvasに転送
        let mut u8_buffer = Vec::with_capacity(WIDTH * HEIGHT * 4);
        for pixel in pixel_buffer.iter() {
            u8_buffer.extend_from_slice(&[((*pixel >> 16) & 0xFF) as u8, ((*pixel >> 8) & 0xFF) as u8, (*pixel & 0xFF) as u8, 255]);
        }
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&u8_buffer), WIDTH as u32, HEIGHT as u32).unwrap();
        context.put_image_data(&image_data, 0.0, 0.0).unwrap();

        // 次のフレームを要求
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));
    request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}

/// requestAnimationFrameを呼び出すヘルパー関数
fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window().unwrap().request_animation_frame(f.as_ref().unchecked_ref()).unwrap();
}