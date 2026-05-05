// src/wasm.rs

use crate::app::{App, AppEvent, CustomProblem, Fonts};
use crate::renderer::{calculate_pixel_font_size, gui_renderer};
use crate::ui::{self, ActiveLowerElement, LowerTypingSegment, Renderable, UpperSegmentState};
use ab_glyph::FontVec;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    CanvasRenderingContext2d, HtmlInputElement, ImageData, InputEvent, KeyboardEvent, WheelEvent,
};

const LS_KEY: &str = "typingmp_custom_problems";

/// localStorage からカスタム問題リストを読み込む
fn load_custom_problems() -> Vec<CustomProblem> {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return Vec::new(),
    };
    let storage = match window.local_storage() {
        Ok(Some(s)) => s,
        _ => return Vec::new(),
    };
    let json = match storage.get_item(LS_KEY) {
        Ok(Some(s)) => s,
        _ => return Vec::new(),
    };
    // JSON パース: [{name, content, timestamp}, ...]
    let parsed = match js_sys::JSON::parse(&json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let arr = match parsed.dyn_into::<js_sys::Array>() {
        Ok(a) => a,
        Err(_) => return Vec::new(),
    };
    let mut result = Vec::new();
    for i in 0..arr.length() {
        let item = arr.get(i);
        let name = js_sys::Reflect::get(&item, &JsValue::from_str("name"))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let content = js_sys::Reflect::get(&item, &JsValue::from_str("content"))
            .ok()
            .and_then(|v| v.as_string())
            .unwrap_or_default();
        let timestamp_ms = js_sys::Reflect::get(&item, &JsValue::from_str("timestamp"))
            .ok()
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0) as u64;
        if !name.is_empty() && !content.is_empty() {
            result.push(CustomProblem {
                name,
                content,
                timestamp_ms,
            });
        }
    }
    result
}

/// カスタム問題リストを localStorage に保存する
fn save_custom_problems(problems: &[CustomProblem]) {
    let window = match web_sys::window() {
        Some(w) => w,
        None => return,
    };
    let storage = match window.local_storage() {
        Ok(Some(s)) => s,
        _ => return,
    };
    let arr = js_sys::Array::new();
    for p in problems {
        let obj = js_sys::Object::new();
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("name"),
            &JsValue::from_str(&p.name),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("content"),
            &JsValue::from_str(&p.content),
        );
        let _ = js_sys::Reflect::set(
            &obj,
            &JsValue::from_str("timestamp"),
            &JsValue::from_f64(p.timestamp_ms as f64),
        );
        arr.push(&obj);
    }
    if let Ok(json) = js_sys::JSON::stringify(&arr) {
        let _ = storage.set_item(LS_KEY, &String::from(json));
    }
}

thread_local! {
    static APP_INSTANCE: RefCell<Option<Rc<RefCell<App>>>> = RefCell::new(None);
    /// フレームごとの再確保を避けるためピクセルバッファを永続化する
    static PIXEL_BUFFER: RefCell<Vec<u32>> = RefCell::new(Vec::new());
    /// ImageData 生成用 u8 バッファを永続化する
    static U8_BUFFER: RefCell<Vec<u8>> = RefCell::new(Vec::new());
    /// グラデーション描画結果のキャッシュ。パラメータが変わった場合のみ再計算する。
    /// キー: (start_color, end_color, width, height)
    static GRADIENT_CACHE: RefCell<Option<(u32, u32, usize, usize, Vec<u32>)>> = RefCell::new(None);
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

/// フリックキーボードなどから直接文字を送信する。
/// JS 側で確定した文字（例: "あ"）を Rust の入力ロジックに渡す。
#[wasm_bindgen]
pub fn send_char(c: String) {
    let timestamp = crate::timestamp::now();
    APP_INSTANCE.with(|instance| {
        if let Some(app_rc) = instance.borrow().as_ref() {
            let mut app = app_rc.borrow_mut();
            for ch in c.chars() {
                app.on_event(AppEvent::Char { c: ch, timestamp });
            }
        }
    });
}

/// タイピング中かどうかを返す。JS 側でキーボード表示制御に使用する。
#[wasm_bindgen]
pub fn is_typing_active() -> bool {
    APP_INSTANCE.with(|instance| {
        instance.borrow().as_ref().map_or(false, |app_rc| {
            app_rc.borrow().state == crate::app::AppState::Typing
        })
    })
}

/// 直前の入力が誤り状態かどうかを返す。
/// JS 側の大⇔小キー（後置修飾）の適用判定に使用する:
/// true の場合は最後に送信した文字が間違いとして記録されており、
/// Backspace で消して修正文字を再送できる。
#[wasm_bindgen]
pub fn has_wrong_input() -> bool {
    APP_INSTANCE.with(|instance| {
        instance.borrow().as_ref().map_or(false, |app_rc| {
            let app = app_rc.borrow();
            if let Some(model) = &app.typing_model {
                model.status.last_wrong_keydown.is_some() || !model.status.unconfirmed.is_empty()
            } else {
                false
            }
        })
    })
}

/// ファイルダイアログ要求フラグを取り出す。
/// JS 側がユーザージェスチャのコールスタック内でこれを呼び、
/// true なら file input を click() する。
#[wasm_bindgen]
pub fn take_file_open_request() -> bool {
    APP_INSTANCE.with(|instance| {
        if let Some(app_rc) = instance.borrow().as_ref() {
            let mut app = app_rc.borrow_mut();
            if app.should_open_file_dialog {
                app.should_open_file_dialog = false;
                return true;
            }
        }
        false
    })
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
                _ => {}
            }
        }
    });
}

/// フォントを `fonts/` 相対パスから非同期に fetch して Vec<u8> で返す
async fn fetch_font_bytes(url: &str) -> Result<Vec<u8>, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let resp_value = JsFuture::from(window.fetch_with_str(url)).await?;
    let resp: web_sys::Response = resp_value.dyn_into()?;
    if !resp.ok() {
        return Err(JsValue::from_str(&format!(
            "Failed to fetch font (status {}): {}",
            resp.status(),
            url
        )));
    }
    let buffer = JsFuture::from(resp.array_buffer()?).await?;
    let bytes = js_sys::Uint8Array::new(&buffer).to_vec();
    Ok(bytes)
}

#[wasm_bindgen(start)]
#[cfg(feature = "wasm")]
pub fn start() {
    #[cfg(debug_assertions)]
    {
        crate::wasm_debug_logger::init();
    }
    console_error_panic_hook::set_once();

    // フォント取得とアプリ初期化を非同期で実行する
    wasm_bindgen_futures::spawn_local(async {
        if let Err(e) = start_async().await {
            web_sys::console::error_1(&e);
        }
    });
}

#[cfg(feature = "wasm")]
async fn start_async() -> Result<(), JsValue> {
    debug_log("Application starting (async).");
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

    // .ntq ファイルアップロード用の非表示ファイル入力要素
    let file_input = document
        .create_element("input")?
        .dyn_into::<HtmlInputElement>()?;
    file_input.set_type("file");
    file_input.set_id("problem-file-input");
    file_input.set_attribute("accept", ".ntq")?;
    file_input.style().set_property("display", "none")?;
    body.append_child(&file_input)?;

    let wrapper = document
        .get_element_by_id("canvas-wrapper")
        .ok_or_else(|| JsValue::from_str("Missing #canvas-wrapper element"))?;

    let canvas = document
        .create_element("canvas")?
        .dyn_into::<web_sys::HtmlCanvasElement>()?;

    wrapper.append_child(&canvas)?;

    let context = canvas
        .get_context("2d")?
        .unwrap()
        .dyn_into::<CanvasRenderingContext2d>()?;

    // フォントをサーバーから非同期 fetch する（WASM バイナリへの埋め込みを回避）
    let japanese_font =
        FontVec::try_from_vec(fetch_font_bytes("./fonts/YujiSyuku-Regular.ttf").await?)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let traditional_chinese_font =
        FontVec::try_from_vec(fetch_font_bytes("./fonts/NotoSerifJP-Regular.ttf").await?)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let simplified_chinese_font =
        FontVec::try_from_vec(fetch_font_bytes("./fonts/NotoSerifJP-Regular.ttf").await?)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let fonts = Fonts {
        japanese: japanese_font,
        traditional_chinese: Some(traditional_chinese_font),
        simplified_chinese: Some(simplified_chinese_font),
    };

    let app = Rc::new(RefCell::new(App::new(fonts)));
    // localStorage からカスタム問題を復元
    {
        let mut app_mut = app.borrow_mut();
        for p in load_custom_problems() {
            app_mut.custom_problems.push(p);
        }
    }
    app.borrow_mut().on_event(AppEvent::Start);

    APP_INSTANCE.with(|instance| {
        *instance.borrow_mut() = Some(app.clone());
    });

    let size = Rc::new(RefCell::new((0, 0)));
    let last_time = Rc::new(RefCell::new(0.0));

    // ファイル選択時の処理: FileReader で読み込み → App に追加 → localStorage 保存
    {
        let app_clone = app.clone();
        let file_input_clone = file_input.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
            let files = match file_input_clone.files() {
                Some(f) => f,
                None => return,
            };
            let file = match files.get(0) {
                Some(f) => f,
                None => return,
            };
            let file_name = file.name();
            let reader = match web_sys::FileReader::new() {
                Ok(r) => r,
                Err(_) => return,
            };
            // FileReader の onload コールバック
            {
                let app_inner = app_clone.clone();
                let file_input_inner = file_input_clone.clone();
                let reader_clone = reader.clone();
                let name_clone = file_name.clone();
                let onload = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::ProgressEvent| {
                    let result = match reader_clone.result() {
                        Ok(v) => v,
                        Err(_) => return,
                    };
                    let content = match result.as_string() {
                        Some(s) => s,
                        None => return,
                    };
                    let timestamp = crate::timestamp::now() as u64;
                    {
                        let mut app_mut = app_inner.borrow_mut();
                        app_mut.add_custom_problem(name_clone.clone(), content, timestamp);
                        // localStorage に保存
                        save_custom_problems(&app_mut.custom_problems);
                    }
                    // 同一ファイルを再度選択できるようにリセット
                    file_input_inner.set_value("");
                });
                reader.set_onload(Some(onload.as_ref().unchecked_ref()));
                onload.forget();
            }
            let _ = reader.read_as_text(&file);
        });
        file_input.add_event_listener_with_callback("change", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // canvasクリックでinput要素にフォーカスを当てるリスナー
    {
        let input_clone = input_element.clone();
        let closure = Closure::<dyn FnMut()>::new(move || {
            let _ = input_clone.focus();
        });
        canvas.add_event_listener_with_callback("click", closure.as_ref().unchecked_ref())?;
        closure.forget();
    }

    // タッチ/ポインターイベント: タップ→Enter、スワイプ上下→Up/Down
    {
        use web_sys::PointerEvent;
        let touch_start: Rc<RefCell<Option<(f64, f64)>>> = Rc::new(RefCell::new(None));

        // pointerdown: 起点座標を保存し input にフォーカス
        {
            let touch_start_clone = touch_start.clone();
            let input_clone = input_element.clone();
            let closure = Closure::<dyn FnMut(_)>::new(move |e: PointerEvent| {
                *touch_start_clone.borrow_mut() = Some((e.client_x() as f64, e.client_y() as f64));
                let _ = input_clone.focus();
            });
            canvas.add_event_listener_with_callback(
                "pointerdown",
                closure.as_ref().unchecked_ref(),
            )?;
            closure.forget();
        }

        // pointerup: ジェスチャー判定
        {
            let touch_start_clone = touch_start.clone();
            let app_clone = app.clone();
            let file_input_clone = file_input.clone();
            const TAP_MAX_DIST: f64 = 12.0; // タップ判定の最大移動量 (px)
            const SWIPE_MIN_DIST: f64 = 40.0; // スワイプ判定の最小移動量 (px)
            const SWIPE_STEP_PX: f64 = 60.0; // スワイプ距離 60px ごとに 1 ステップ
            let closure = Closure::<dyn FnMut(_)>::new(move |e: PointerEvent| {
                let start = match touch_start_clone.borrow_mut().take() {
                    Some(s) => s,
                    None => return,
                };
                let dx = e.client_x() as f64 - start.0;
                let dy = e.client_y() as f64 - start.1;
                let dist = (dx * dx + dy * dy).sqrt();

                if dist < TAP_MAX_DIST {
                    // タップ → Enter
                    app_clone.borrow_mut().on_event(AppEvent::Enter);
                    if app_clone.borrow().should_open_file_dialog {
                        app_clone.borrow_mut().should_open_file_dialog = false;
                        let _ = file_input_clone.click();
                    }
                } else if dist >= SWIPE_MIN_DIST && dy.abs() > dx.abs() {
                    // 縦スワイプ: 距離に応じてステップ数を調整 (最小 1、最大 5)
                    let steps = ((dy.abs() / SWIPE_STEP_PX).ceil() as u32).max(1).min(5);
                    let mut a = app_clone.borrow_mut();
                    for _ in 0..steps {
                        if dy < 0.0 {
                            a.on_event(AppEvent::Up);
                        } else {
                            a.on_event(AppEvent::Down);
                        }
                    }
                }
                // 横スワイプは無視（タイピング中の横スクロールは将来対応）
            });
            canvas
                .add_event_listener_with_callback("pointerup", closure.as_ref().unchecked_ref())?;
            closure.forget();
        }
    }

    // マウスホイール / タッチパッドスクロール → Up/Down イベント
    // deltaMode: 0=ピクセル, 1=行, 2=ページ
    {
        let app_clone = app.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |e: WheelEvent| {
            e.prevent_default();
            let delta = e.delta_y();
            let steps = match e.delta_mode() {
                0 => ((delta.abs() / 50.0).ceil() as u32).max(1).min(5), // ピクセル単位
                1 => (delta.abs() as u32).max(1).min(5),                 // 行単位
                _ => 1,                                                  // ページ単位など
            };
            let mut a = app_clone.borrow_mut();
            for _ in 0..steps {
                if delta < 0.0 {
                    a.on_event(AppEvent::Up);
                } else {
                    a.on_event(AppEvent::Down);
                }
            }
        });
        canvas.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref())?;
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
        window
            .add_event_listener_with_callback("resize", resize_closure.as_ref().unchecked_ref())?;
        resize_closure
            .as_ref()
            .unchecked_ref::<js_sys::Function>()
            .call0(&JsValue::NULL)
            .unwrap();
        resize_closure.forget();
    }

    // キー入力イベント（特殊キー用）
    {
        let app_clone = app.clone();
        let file_input_clone = file_input.clone();
        let closure = Closure::<dyn FnMut(_)>::new(move |event: KeyboardEvent| {
            // 生のKeyboardEventの内容をログに出力
            debug_log(&format!(
                "[KeyDown wasm.rs] key: '{}', code: '{}', composing: {}",
                event.key(),
                event.code(),
                event.is_composing()
            ));

            match event.key().as_str() {
                "ArrowUp" => {
                    event.prevent_default();
                    app_clone.borrow_mut().on_event(AppEvent::Up)
                }
                "ArrowDown" => {
                    event.prevent_default();
                    app_clone.borrow_mut().on_event(AppEvent::Down)
                }
                "Backspace" => {
                    event.prevent_default();
                    app_clone.borrow_mut().on_event(AppEvent::Backspace)
                }
                "Enter" => {
                    event.prevent_default();
                    app_clone.borrow_mut().on_event(AppEvent::Enter);
                    // ファイルダイアログ要求フラグを確認し、ユーザージェスチャのコールスタック内で click() を呼ぶ
                    if app_clone.borrow().should_open_file_dialog {
                        app_clone.borrow_mut().should_open_file_dialog = false;
                        let _ = file_input_clone.click();
                    }
                }
                "Escape" => {
                    event.prevent_default();
                    app_clone.borrow_mut().on_event(AppEvent::Escape)
                }
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
                        app.on_event(AppEvent::Char {
                            c,
                            timestamp: crate::timestamp::now(),
                        });
                    }
                    // ProblemSelection での削除・並び替えが発生した場合 localStorage に保存
                    if app.should_save_custom_problems {
                        save_custom_problems(&app.custom_problems);
                        app.should_save_custom_problems = false;
                    }
                }
            }
        });
        input_element
            .add_event_listener_with_callback("input", closure.as_ref().unchecked_ref())?;
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
        let delta_time = if *last_time_borrow > 0.0 {
            now - *last_time_borrow
        } else {
            16.6
        };
        *last_time_borrow = now;

        app.borrow_mut().update(width, height, delta_time);

        // --- 描画処理（不変借用） ---
        {
            let app_borrow = app.borrow();
            let current_font = app_borrow.get_current_font();

            // ピクセルバッファを再利用（毎フレームの大量アロケーションを回避）
            PIXEL_BUFFER.with(|pb| {
                let mut pb = pb.borrow_mut();
                let needed = width * height;
                if pb.len() != needed {
                    pb.resize(needed, 0);
                }
                // pb.fill(0) は不要: Background グラデーションが毎フレーム全画面を上書きする

                let render_list = ui::build_ui(&app_borrow, current_font, width, height);

                for item in render_list {
                    match item {
                        Renderable::Background { gradient } => {
                            // グラデーションはパラメータ（色・サイズ）が変わった場合のみ再計算し、
                            // それ以外はキャッシュ済みバッファを memcpy するだけにする。
                            // 毎フレーム ~2000万 float 演算していたものが memcpy に置き換わる。
                            GRADIENT_CACHE.with(|gc| {
                                let mut gc = gc.borrow_mut();
                                let matches = gc.as_ref().map_or(false, |(sc, ec, gw, gh, _)| {
                                    *sc == gradient.start_color
                                        && *ec == gradient.end_color
                                        && *gw == width
                                        && *gh == height
                                });
                                if matches {
                                    let cached = &gc.as_ref().unwrap().4;
                                    pb.copy_from_slice(cached);
                                } else {
                                    crate::renderer::draw_linear_gradient(
                                        &mut pb,
                                        width,
                                        height,
                                        gradient.start_color,
                                        gradient.end_color,
                                        (0.0, 0.0),
                                        (width as f32, height as f32),
                                    );
                                    *gc = Some((
                                        gradient.start_color,
                                        gradient.end_color,
                                        width,
                                        height,
                                        pb.to_vec(),
                                    ));
                                }
                            });
                        }
                        Renderable::BigText {
                            text,
                            anchor,
                            shift,
                            align,
                            font_size,
                            color,
                        }
                        | Renderable::Text {
                            text,
                            anchor,
                            shift,
                            align,
                            font_size,
                            color,
                        } => {
                            let pixel_font_size =
                                calculate_pixel_font_size(font_size, width, height);
                            let (text_width, text_height, _) =
                                gui_renderer::measure_text(current_font, &text, pixel_font_size);
                            let anchor_pos =
                                ui::calculate_anchor_position(anchor, shift, width, height);
                            let (x, y) = ui::calculate_aligned_position(
                                anchor_pos,
                                text_width,
                                text_height,
                                align,
                            );
                            gui_renderer::draw_text(
                                &mut pb,
                                width,
                                current_font,
                                &text,
                                (x as f32, y as f32),
                                pixel_font_size,
                                color,
                            );
                        }
                        Renderable::TypingUpper {
                            segments,
                            anchor,
                            shift,
                            align,
                            font_size,
                        } => {
                            let pixel_font_size =
                                calculate_pixel_font_size(font_size, width, height);
                            let ruby_pixel_font_size = pixel_font_size * 0.4;

                            // 各セグメントの幅を一度だけ計測してキャッシュする
                            let seg_widths: Vec<u32> = segments
                                .iter()
                                .map(|seg| {
                                    gui_renderer::measure_text(
                                        current_font,
                                        &seg.base_text,
                                        pixel_font_size,
                                    )
                                    .0
                                })
                                .collect();
                            let total_width: u32 = seg_widths.iter().sum();
                            let total_height =
                                gui_renderer::measure_text(current_font, " ", pixel_font_size).1;

                            let anchor_pos =
                                ui::calculate_anchor_position(anchor, shift, width, height);
                            let (mut pen_x, y) = ui::calculate_aligned_position(
                                anchor_pos,
                                total_width,
                                total_height,
                                align,
                            );

                            for (seg, &seg_w) in segments.iter().zip(seg_widths.iter()) {
                                let color = match seg.state {
                                    UpperSegmentState::Correct => ui::CORRECT_COLOR,
                                    UpperSegmentState::Incorrect => ui::INCORRECT_COLOR,
                                    UpperSegmentState::Active => ui::ACTIVE_COLOR,
                                    UpperSegmentState::Pending => ui::PENDING_COLOR,
                                };
                                gui_renderer::draw_text(
                                    &mut pb,
                                    width,
                                    current_font,
                                    &seg.base_text,
                                    (pen_x as f32, y as f32),
                                    pixel_font_size,
                                    color,
                                );

                                if let Some(ruby) = &seg.ruby_text {
                                    let (ruby_w, ..) = gui_renderer::measure_text(
                                        current_font,
                                        ruby,
                                        ruby_pixel_font_size,
                                    );
                                    let ruby_x =
                                        pen_x as f32 + (seg_w as f32 - ruby_w as f32) / 2.0;
                                    let ruby_y = y as f32 - ruby_pixel_font_size * 0.5;
                                    gui_renderer::draw_text(
                                        &mut pb,
                                        width,
                                        current_font,
                                        ruby,
                                        (ruby_x, ruby_y),
                                        ruby_pixel_font_size,
                                        color,
                                    );
                                }

                                pen_x += seg_w as i32;
                            }
                        }
                        Renderable::TypingLower {
                            segments,
                            anchor,
                            shift,
                            align,
                            font_size,
                            target_line_total_width,
                        } => {
                            let pixel_font_size =
                                calculate_pixel_font_size(font_size, width, height);
                            let ruby_pixel_font_size = pixel_font_size * 0.3;
                            let total_height =
                                gui_renderer::measure_text(current_font, " ", pixel_font_size).1;

                            let anchor_pos =
                                ui::calculate_anchor_position(anchor, shift, width, height);
                            let (mut pen_x, y) = ui::calculate_aligned_position(
                                anchor_pos,
                                target_line_total_width,
                                total_height,
                                align,
                            );

                            for seg in segments {
                                match seg {
                                    LowerTypingSegment::Completed {
                                        base_text,
                                        ruby_text,
                                        is_correct,
                                        width: seg_width,
                                        ..
                                    } => {
                                        let color = if is_correct {
                                            ui::CORRECT_COLOR
                                        } else {
                                            ui::INCORRECT_COLOR
                                        };
                                        // base_w を一度だけ計測してルビ位置と pen 進行の両方に使う
                                        let base_w = seg_width;
                                        gui_renderer::draw_text(
                                            &mut pb,
                                            width,
                                            current_font,
                                            &base_text,
                                            (pen_x as f32, y as f32),
                                            pixel_font_size,
                                            color,
                                        );

                                        if let Some(ruby) = ruby_text {
                                            let (ruby_w, ..) = gui_renderer::measure_text(
                                                current_font,
                                                &ruby,
                                                ruby_pixel_font_size,
                                            );
                                            let ruby_x = pen_x as f32
                                                + (base_w as f32 - ruby_w as f32) / 2.0;
                                            let ruby_y = y as f32 - ruby_pixel_font_size * 0.5;
                                            gui_renderer::draw_text(
                                                &mut pb,
                                                width,
                                                current_font,
                                                &ruby,
                                                (ruby_x, ruby_y),
                                                ruby_pixel_font_size,
                                                color,
                                            );
                                        }

                                        pen_x += base_w as i32;
                                    }
                                    LowerTypingSegment::Active { elements } => {
                                        for el in elements {
                                            let (text, color) = match el {
                                                ActiveLowerElement::Typed {
                                                    character,
                                                    is_correct,
                                                } => (
                                                    character.to_string(),
                                                    if is_correct {
                                                        ui::CORRECT_COLOR
                                                    } else {
                                                        ui::INCORRECT_COLOR
                                                    },
                                                ),
                                                ActiveLowerElement::Cursor => {
                                                    ("|".to_string(), ui::CURSOR_COLOR)
                                                }
                                                ActiveLowerElement::UnconfirmedInput(s) => {
                                                    (s.clone(), ui::UNCONFIRMED_COLOR)
                                                }
                                                ActiveLowerElement::LastIncorrectInput(c) => {
                                                    (c.to_string(), ui::WRONG_KEY_COLOR)
                                                }
                                            };
                                            let text_w = gui_renderer::measure_text(
                                                current_font,
                                                &text,
                                                pixel_font_size,
                                            )
                                            .0;
                                            gui_renderer::draw_text(
                                                &mut pb,
                                                width,
                                                current_font,
                                                &text,
                                                (pen_x as f32, y as f32),
                                                pixel_font_size,
                                                color,
                                            );
                                            pen_x += text_w as i32;
                                        }
                                    }
                                }
                            }
                        }
                        Renderable::ProgressBar {
                            anchor,
                            shift,
                            width_ratio,
                            height_ratio,
                            progress,
                            bg_color,
                            fg_color,
                        } => {
                            let bar_width = (width as f32 * width_ratio) as u32;
                            let bar_height = (height as f32 * height_ratio) as u32;

                            let anchor_pos =
                                ui::calculate_anchor_position(anchor, shift, width, height);
                            let start_x = anchor_pos.0 as usize;
                            let start_y = (anchor_pos.1 - bar_height as i32).max(0) as usize;

                            gui_renderer::draw_rect(
                                &mut pb,
                                width,
                                start_x,
                                start_y,
                                bar_width as usize,
                                bar_height as usize,
                                bg_color,
                            );

                            let fg_width = (bar_width as f32 * progress) as usize;
                            if fg_width > 0 {
                                gui_renderer::draw_rect(
                                    &mut pb,
                                    width,
                                    start_x,
                                    start_y,
                                    fg_width,
                                    bar_height as usize,
                                    fg_color,
                                );
                            }
                        }
                    }
                }

                // u8 バッファも再利用してコピー（毎フレームの大量アロケーションを回避）
                U8_BUFFER.with(|ub| {
                    let mut ub = ub.borrow_mut();
                    let needed_u8 = width * height * 4;
                    if ub.len() != needed_u8 {
                        ub.resize(needed_u8, 0);
                    }
                    for (i, pixel) in pb.iter().enumerate() {
                        let base = i * 4;
                        ub[base] = ((*pixel >> 16) & 0xFF) as u8;
                        ub[base + 1] = ((*pixel >> 8) & 0xFF) as u8;
                        ub[base + 2] = (*pixel & 0xFF) as u8;
                        ub[base + 3] = 255;
                    }
                    let image_data = ImageData::new_with_u8_clamped_array_and_sh(
                        Clamped(&ub),
                        width as u32,
                        height as u32,
                    )
                    .unwrap();
                    context.put_image_data(&image_data, 0.0, 0.0).unwrap();
                });
            });
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
