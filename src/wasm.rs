// -----------------------------------------------------------------------------
// WASMバックエンドの実装 (feature = "wasm" の時のみコンパイル)
// -----------------------------------------------------------------------------
use crate::app::App;
use crate::renderer::gui_renderer;
use ab_glyph::{Font, FontRef};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use web_sys::{CanvasRenderingContext2d, ImageData, KeyboardEvent};

const WIDTH: usize = 800;
const HEIGHT: usize = 100;

// JavaScript側から呼び出すエントリーポイント
#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    // パニック時にコンソールにエラーを出力する
    #[cfg(debug_assertions)]
    console_error_panic_hook::set_once();

    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let body = document.body().expect("document should have a body");

    // キャンバスを作成してbodyに追加
    let canvas = document
        .create_element("canvas")?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;
    canvas.set_width(WIDTH as u32);
    canvas.set_height(HEIGHT as u32);
    body.append_child(&canvas)?;

    let context = canvas
        .get_context("2d")?
        .unwrap()
        .dyn_into::<web_sys::CanvasRenderingContext2d>()?;

    // フォントデータの読み込み
    let font_data = include_bytes!("../fonts/NotoSerifJP-Regular.ttf");
    let font = FontRef::try_from_slice(font_data)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    // アプリケーションの状態をRc<RefCell<T>>で包む
    let app = Rc::new(RefCell::new(App::new()));

    // キーボードイベントの処理
    {
        let app_for_keyboard = app.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: KeyboardEvent| {
            let mut app = app_for_keyboard.borrow_mut();
            match event.key().as_str() {
                "Backspace" => app.on_backspace(),
                "Escape" => app.should_quit = true,
                "Enter" => app.on_key('\n'),
                key if key.len() == 1 => app.on_key(key.chars().next().unwrap()),
                _ => {}
            }
        });

        document.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }


    // メインループ (requestAnimationFrameを使用)
    let f = Rc::new(RefCell::new(None));
    let g = f.clone();
    let app_for_render = app.clone();

    *g.borrow_mut() = Some(Closure::<dyn FnMut()>::new(move || {
        let mut app = app_for_render.borrow_mut();
        if app.should_quit { return; }

        // 描画処理
        let pixel_buffer_u32 = gui_renderer::render(&font, &app.input_text, WIDTH, HEIGHT);
        // u32 (ABGR in memory) から u8 (RGBA for canvas) に変換
        let mut pixel_buffer_u8 = Vec::with_capacity(WIDTH * HEIGHT * 4);
        for pixel in pixel_buffer_u32.iter() {
            let r = ((*pixel >> 16) & 0xFF) as u8;
            let g = ((*pixel >> 8) & 0xFF) as u8;
            let b = (*pixel & 0xFF) as u8;
            pixel_buffer_u8.extend_from_slice(&[r, g, b, 255]); // Alphaを255(不透明)に
        }

        // キャンバスに描画
        let image_data = ImageData::new_with_u8_clamped_array_and_sh(Clamped(&pixel_buffer_u8), WIDTH as u32, HEIGHT as u32).unwrap();
        context.put_image_data(&image_data, 0.0, 0.0).unwrap();

        // 次のフレームを要求
        request_animation_frame(f.borrow().as_ref().unwrap());
    }));

    request_animation_frame(g.borrow().as_ref().unwrap());

    Ok(())
}

// requestAnimationFrameを呼ぶためのヘルパー関数
fn request_animation_frame(f: &Closure<dyn FnMut()>) {
    web_sys::window()
        .expect("no global `window` exists")
        .request_animation_frame(f.as_ref().unchecked_ref())
        .expect("should register `requestAnimationFrame` OK");
}