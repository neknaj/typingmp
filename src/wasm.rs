// src/wasm.rs

use crate::app::{App, AppEvent, CustomProblem, FontBundle, Fonts, UiCommand};
use crate::backend::BackendError;
use crate::io::{
    bundled_font_entries, bundled_font_file_name, PersistentStore, ProviderError, ProviderErrorKind,
};
use crate::renderer::{ArgbSurface, RenderCache};
use crate::screen_keyboard::{
    self, ScreenKeyboardAction, ScreenKeyboardKeyRole, ScreenKeyboardLayoutKind,
};
use crate::ui;
use ab_glyph::FontVec;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::Clamped;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::{spawn_local, JsFuture};
use web_sys::{
    CanvasRenderingContext2d, HtmlInputElement, ImageData, InputEvent, KeyboardEvent, WheelEvent,
};

const LS_KEY: &str = "typingmp_custom_problems";
type AnimationFrameCallback = Rc<RefCell<Option<Closure<dyn FnMut()>>>>;

#[derive(Debug, Clone, Copy, Default)]
struct WebCustomProblemStore;

impl PersistentStore for WebCustomProblemStore {
    fn load_custom_problems(&self) -> Result<Vec<CustomProblem>, ProviderError> {
        let Some(window) = web_sys::window() else {
            return Ok(Vec::new());
        };
        let storage = match window.local_storage() {
            Ok(Some(storage)) => storage,
            Ok(None) => return Ok(Vec::new()),
            Err(_) => {
                return Err(ProviderError::new(
                    ProviderErrorKind::Io,
                    "failed to access localStorage",
                ));
            }
        };
        let json = match storage.get_item(LS_KEY) {
            Ok(Some(json)) => json,
            Ok(None) => return Ok(Vec::new()),
            Err(_) => {
                return Err(ProviderError::new(
                    ProviderErrorKind::Io,
                    "failed to read custom problems from localStorage",
                ));
            }
        };

        let parsed = js_sys::JSON::parse(&json).map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::Decode,
                "failed to parse custom problems JSON",
            )
        })?;
        let arr = parsed.dyn_into::<js_sys::Array>().map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::Decode,
                "custom problems JSON root is not an array",
            )
        })?;

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
        Ok(result)
    }

    fn save_custom_problems(&self, problems: &[CustomProblem]) -> Result<(), ProviderError> {
        let Some(window) = web_sys::window() else {
            return Ok(());
        };
        let storage = match window.local_storage() {
            Ok(Some(storage)) => storage,
            Ok(None) => return Ok(()),
            Err(_) => {
                return Err(ProviderError::new(
                    ProviderErrorKind::Io,
                    "failed to access localStorage",
                ));
            }
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
        let json = js_sys::JSON::stringify(&arr).map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::Decode,
                "failed to encode custom problems JSON",
            )
        })?;
        storage.set_item(LS_KEY, &String::from(json)).map_err(|_| {
            ProviderError::new(
                ProviderErrorKind::Io,
                "failed to save custom problems to localStorage",
            )
        })
    }
}

thread_local! {
    static APP_INSTANCE: RefCell<Option<Rc<RefCell<App>>>> = const { RefCell::new(None) };
    /// フレームごとの再確保を避けるためピクセルバッファを永続化する
    static PIXEL_BUFFER: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    /// ImageData 生成用 u8 バッファを永続化する
    static U8_BUFFER: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    /// Pixel render state shared across animation frames.
    static RENDER_CACHE: RefCell<RenderCache> = const { RefCell::new(RenderCache::new()) };
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
        instance
            .borrow()
            .as_ref()
            .is_some_and(|app_rc| app_rc.borrow().is_typing_active())
    })
}

/// 直前の入力が誤り状態かどうかを返す。
/// JS 側の大⇔小キー（後置修飾）の適用判定に使用する:
/// true の場合は最後に送信した文字が間違いとして記録されており、
/// Backspace で消して修正文字を再送できる。
#[wasm_bindgen]
pub fn has_wrong_input() -> bool {
    APP_INSTANCE.with(|instance| {
        instance
            .borrow()
            .as_ref()
            .is_some_and(|app_rc| app_rc.borrow().has_pending_input_correction())
    })
}

/// Returns whether the app should accept OS IME input events.
#[wasm_bindgen]
pub fn accepts_ime_input() -> bool {
    APP_INSTANCE.with(|instance| {
        instance
            .borrow()
            .as_ref()
            .is_some_and(|app_rc| app_rc.borrow().accepts_ime_input())
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
            return app.take_file_open_request();
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
            if let Some(command) = UiCommand::from_bridge_label(event_type) {
                app.on_event(command.app_event());
            }
        }
    });
}

#[wasm_bindgen]
pub fn screen_keyboard_layout(kind: &str) -> JsValue {
    let kind = ScreenKeyboardLayoutKind::from_bridge_label(kind).unwrap_or_default();
    let layout = screen_keyboard::layout(kind);
    let obj = js_sys::Object::new();
    set_js_prop(
        &obj,
        "kind",
        JsValue::from_str(layout.kind.bridge_label()).as_ref(),
    );
    set_js_prop(&obj, "label", JsValue::from_str(layout.label).as_ref());

    let rows = js_sys::Array::new();
    for row in layout.rows {
        let row_array = js_sys::Array::new();
        for key in row.keys {
            row_array.push(&screen_keyboard_key_to_js(key));
        }
        rows.push(&row_array);
    }
    set_js_prop(&obj, "rows", rows.as_ref());
    obj.into()
}

#[wasm_bindgen]
pub fn screen_keyboard_resolve(
    kind: &str,
    row_index: u32,
    key_index: u32,
    dx: f32,
    dy: f32,
) -> JsValue {
    let kind = ScreenKeyboardLayoutKind::from_bridge_label(kind).unwrap_or_default();
    screen_keyboard_action_to_js(screen_keyboard::resolve_key(
        kind,
        row_index as usize,
        key_index as usize,
        dx,
        dy,
    ))
}

#[wasm_bindgen]
pub fn screen_keyboard_next_layout(kind: &str) -> String {
    ScreenKeyboardLayoutKind::from_bridge_label(kind)
        .unwrap_or_default()
        .next()
        .bridge_label()
        .to_string()
}

#[wasm_bindgen]
pub fn screen_keyboard_modified_char(value: String) -> String {
    let mut chars = value.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => screen_keyboard::modified_kana(c)
            .map(|c| c.to_string())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn set_js_prop(obj: &js_sys::Object, name: &str, value: &JsValue) {
    let _ = js_sys::Reflect::set(obj, &JsValue::from_str(name), value);
}

fn screen_keyboard_key_to_js(key: &screen_keyboard::ScreenKeyboardKey) -> JsValue {
    let obj = js_sys::Object::new();
    set_js_prop(&obj, "label", JsValue::from_str(key.label).as_ref());
    set_js_prop(
        &obj,
        "class",
        JsValue::from_str(key.class.bridge_label()).as_ref(),
    );
    set_js_prop(
        &obj,
        "width",
        JsValue::from_str(key.width.bridge_label()).as_ref(),
    );
    let interactive = !matches!(key.role, ScreenKeyboardKeyRole::Spacer);
    set_js_prop(
        &obj,
        "interactive",
        JsValue::from_bool(interactive).as_ref(),
    );

    if let ScreenKeyboardKeyRole::Flick(map) = key.role {
        set_js_prop(&obj, "center", JsValue::from_str(map.center).as_ref());
        if let Some(value) = map.up {
            set_js_prop(&obj, "up", JsValue::from_str(value).as_ref());
        }
        if let Some(value) = map.right {
            set_js_prop(&obj, "right", JsValue::from_str(value).as_ref());
        }
        if let Some(value) = map.down {
            set_js_prop(&obj, "down", JsValue::from_str(value).as_ref());
        }
        if let Some(value) = map.left {
            set_js_prop(&obj, "left", JsValue::from_str(value).as_ref());
        }
    }

    obj.into()
}

fn screen_keyboard_action_to_js(action: ScreenKeyboardAction) -> JsValue {
    let obj = js_sys::Object::new();
    match action {
        ScreenKeyboardAction::Text(value) => {
            set_js_prop(&obj, "kind", JsValue::from_str("text").as_ref());
            set_js_prop(&obj, "value", JsValue::from_str(value).as_ref());
        }
        ScreenKeyboardAction::UiCommand(command) => {
            set_js_prop(&obj, "kind", JsValue::from_str("command").as_ref());
            set_js_prop(
                &obj,
                "value",
                JsValue::from_str(command.bridge_label()).as_ref(),
            );
        }
        ScreenKeyboardAction::TransformLastText => {
            set_js_prop(
                &obj,
                "kind",
                JsValue::from_str("transform-last-text").as_ref(),
            );
        }
        ScreenKeyboardAction::SwitchLayout => {
            set_js_prop(&obj, "kind", JsValue::from_str("switch-layout").as_ref());
        }
        ScreenKeyboardAction::SwitchInputSource => {
            set_js_prop(
                &obj,
                "kind",
                JsValue::from_str("switch-input-source").as_ref(),
            );
        }
        ScreenKeyboardAction::None => {
            set_js_prop(&obj, "kind", JsValue::from_str("none").as_ref());
        }
    }
    obj.into()
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
            show_startup_error(&e);
        }
    });
}

#[cfg(feature = "wasm")]
async fn start_async() -> Result<(), JsValue> {
    debug_log("Application starting (async).");
    let window = web_sys::window()
        .ok_or_else(|| js_backend_error(BackendError::dom("missing browser window")))?;
    let document = window
        .document()
        .ok_or_else(|| js_backend_error(BackendError::dom("missing browser document")))?;
    let body = document
        .body()
        .ok_or_else(|| js_backend_error(BackendError::dom("missing document body")))?;

    let input_element = document
        .create_element("input")?
        .dyn_into::<HtmlInputElement>()?;
    input_element.set_type("text");
    {
        input_element.set_attribute("inputmode", "none")?;
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
        .ok_or_else(|| js_backend_error(BackendError::dom("2D canvas context is unavailable")))?
        .dyn_into::<CanvasRenderingContext2d>()?;

    // フォントをサーバーから非同期 fetch する（WASM バイナリへの埋め込みを回避）
    let ui_font = FontVec::try_from_vec(fetch_font_bytes("./fonts/NotoSerifJP-Regular.ttf").await?)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let japanese_font =
        FontVec::try_from_vec(fetch_font_bytes("./fonts/YujiSyuku-Regular.ttf").await?)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let japanese_ruby_font =
        FontVec::try_from_vec(fetch_font_bytes("./fonts/YujiSyuku-Regular.ttf").await?)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let japanese_unconfirmed_font =
        FontVec::try_from_vec(fetch_font_bytes("./fonts/YujiSyuku-Regular.ttf").await?)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let simplified_chinese_font =
        FontVec::try_from_vec(fetch_font_bytes("./fonts/LongCang-Regular.ttf").await?)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let simplified_chinese_ruby_font =
        FontVec::try_from_vec(fetch_font_bytes("./fonts/Alegreya-VariableFont_wght.ttf").await?)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let simplified_chinese_unconfirmed_font =
        FontVec::try_from_vec(fetch_font_bytes("./fonts/Alegreya-VariableFont_wght.ttf").await?)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let traditional_chinese_font =
        FontVec::try_from_vec(fetch_font_bytes("./fonts/LongCang-Regular.ttf").await?)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let traditional_chinese_ruby_font =
        FontVec::try_from_vec(fetch_font_bytes("./fonts/Alegreya-VariableFont_wght.ttf").await?)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let traditional_chinese_unconfirmed_font =
        FontVec::try_from_vec(fetch_font_bytes("./fonts/Alegreya-VariableFont_wght.ttf").await?)
            .map_err(|e| JsValue::from_str(&e.to_string()))?;
    let english_font = FontVec::try_from_vec(fetch_font_bytes("./fonts/Kalam-Regular.ttf").await?)
        .map_err(|e| JsValue::from_str(&e.to_string()))?;

    let fonts = Fonts::new(FontBundle {
        ui: ui_font,
        japanese: japanese_font,
        japanese_ruby: japanese_ruby_font,
        japanese_unconfirmed: japanese_unconfirmed_font,
        chinese_simplified: simplified_chinese_font,
        chinese_simplified_ruby: simplified_chinese_ruby_font,
        chinese_simplified_unconfirmed: simplified_chinese_unconfirmed_font,
        traditional_chinese: traditional_chinese_font,
        traditional_chinese_ruby: traditional_chinese_ruby_font,
        traditional_chinese_unconfirmed: traditional_chinese_unconfirmed_font,
        english: english_font,
    });

    let app = Rc::new(RefCell::new(App::new(fonts)));
    // localStorage からカスタム問題を復元
    {
        let mut app_mut = app.borrow_mut();
        app_mut.set_available_fonts(bundled_font_entries());
        match WebCustomProblemStore.load_custom_problems() {
            Ok(problems) => app_mut.set_custom_problems(problems),
            Err(err) => report_provider_error(
                &mut app_mut,
                "failed to load custom problems from localStorage",
                err,
            ),
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
                        if let Err(err) =
                            WebCustomProblemStore.save_custom_problems(app_mut.custom_problems())
                        {
                            report_provider_error(
                                &mut app_mut,
                                "failed to save custom problems to localStorage",
                                err,
                            );
                        }
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
        let app_clone = app.clone();
        let input_clone = input_element.clone();
        let closure = Closure::<dyn FnMut()>::new(move || {
            if app_clone.borrow().accepts_ime_input() {
                let _ = input_clone.focus();
            }
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
            let app_clone = app.clone();
            let input_clone = input_element.clone();
            let closure = Closure::<dyn FnMut(_)>::new(move |e: PointerEvent| {
                *touch_start_clone.borrow_mut() = Some((e.client_x() as f64, e.client_y() as f64));
                if app_clone.borrow().accepts_ime_input() {
                    let _ = input_clone.focus();
                }
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
                    if app_clone.borrow_mut().take_file_open_request() {
                        file_input_clone.click();
                    }
                } else if dist >= SWIPE_MIN_DIST && dy.abs() > dx.abs() {
                    // 縦スワイプ: 距離に応じてステップ数を調整 (最小 1、最大 5)
                    let steps = ((dy.abs() / SWIPE_STEP_PX).ceil() as u32).clamp(1, 5);
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
                0 => ((delta.abs() / 50.0).ceil() as u32).clamp(1, 5), // ピクセル単位
                1 => (delta.abs() as u32).clamp(1, 5),                 // 行単位
                _ => 1,                                                // ページ単位など
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
            .call0(&JsValue::NULL)?;
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

            if let Some(command) = UiCommand::from_web_key(event.key().as_str()) {
                event.prevent_default();
                app_clone.borrow_mut().on_event(command.app_event());
                if command == UiCommand::Enter && app_clone.borrow_mut().take_file_open_request() {
                    file_input_clone.click();
                }
                return;
            }

            if event.is_composing() || event.ctrl_key() || event.alt_key() || event.meta_key() {
                return;
            }

            let key = event.key();
            let mut chars = key.chars();
            let Some(c) = chars.next() else {
                return;
            };
            if chars.next().is_some() {
                return;
            }

            let mut app = app_clone.borrow_mut();
            if !app.accepts_ime_input() {
                event.prevent_default();
                app.on_event(AppEvent::Char {
                    c,
                    timestamp: crate::timestamp::now(),
                });
                if app.take_custom_problem_save_request() {
                    if let Err(err) =
                        WebCustomProblemStore.save_custom_problems(app.custom_problems())
                    {
                        report_provider_error(
                            &mut app,
                            "failed to save custom problems to localStorage",
                            err,
                        );
                    }
                }
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

            if !app_clone.borrow().accepts_ime_input() {
                input_clone.set_value("");
                return;
            }

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
                    if app.take_custom_problem_save_request() {
                        if let Err(err) =
                            WebCustomProblemStore.save_custom_problems(app.custom_problems())
                        {
                            report_provider_error(
                                &mut app,
                                "failed to save custom problems to localStorage",
                                err,
                            );
                        }
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
            schedule_next_frame(&f);
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

        {
            let font_load_request = {
                let mut app_mut = app.borrow_mut();
                let viewport = app_mut.display_settings().viewport(width, height);
                app_mut.update(viewport.width, viewport.height, delta_time);
                app_mut.take_font_load_request()
            };

            if let Some(request) = font_load_request {
                let Some(file_name) = bundled_font_file_name(request.font_id) else {
                    app.borrow_mut()
                        .report_visible_error("selected bundled font was not found");
                    schedule_next_frame(&f);
                    return;
                };
                let app_for_font = app.clone();
                let path = format!("./fonts/{file_name}");
                spawn_local(async move {
                    match fetch_font_bytes(&path).await {
                        Ok(bytes) => {
                            let mut app_mut = app_for_font.borrow_mut();
                            if let Err(err) =
                                app_mut.apply_font_bytes(request.target, request.font_name, bytes)
                            {
                                app_mut
                                    .report_visible_error(format!("failed to apply font: {err:?}"));
                            }
                        }
                        Err(err) => {
                            app_for_font.borrow_mut().report_visible_error(format!(
                                "failed to load font {file_name}: {}",
                                js_value_to_string(&err)
                            ));
                        }
                    }
                });
            }
        }

        // --- 描画処理（不変借用） ---
        {
            let app_borrow = app.borrow();
            let display_settings = app_borrow.display_settings();
            let viewport = display_settings.viewport(width, height);
            let fonts = app_borrow.fonts();

            // ピクセルバッファを再利用（毎フレームの大量アロケーションを回避）
            PIXEL_BUFFER.with(|pb| {
                let mut pb = pb.borrow_mut();
                let needed = width * height;
                if pb.len() != needed {
                    pb.resize(needed, 0);
                }
                // pb.fill(0) は不要: renderer が viewport 外も含めて毎フレーム初期化する

                let render_list = ui::build_ui(&app_borrow, fonts, viewport.width, viewport.height);
                let frame_changed = RENDER_CACHE.with(|cache| {
                    let mut cache = cache.borrow_mut();
                    if let Some(mut surface) = ArgbSurface::new(width, height, &mut pb) {
                        surface
                            .render(fonts, display_settings, &render_list, &mut cache)
                            .changed()
                    } else {
                        false
                    }
                });
                if !frame_changed {
                    return;
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
                    let image_data = match ImageData::new_with_u8_clamped_array_and_sh(
                        Clamped(&ub),
                        width as u32,
                        height as u32,
                    ) {
                        Ok(image_data) => image_data,
                        Err(err) => {
                            web_sys::console::error_1(&err);
                            return;
                        }
                    };
                    if let Err(err) = context.put_image_data(&image_data, 0.0, 0.0) {
                        web_sys::console::error_1(&err);
                    }
                });
            });
        }

        // --- IMEリセット処理（可変借用） ---
        let mut app_borrow_mut = app.borrow_mut();
        if app_borrow_mut.take_ime_reset_request() {
            let _ = ime_input_element.blur();
            if app_borrow_mut.accepts_ime_input() {
                let _ = ime_input_element.focus();
            }
        }

        schedule_next_frame(&f);
    }));
    {
        let borrowed = g.borrow();
        let callback = borrowed.as_ref().ok_or_else(|| {
            js_backend_error(BackendError::state(
                "animation frame callback was not initialized",
            ))
        })?;
        request_animation_frame(callback)?;
    }

    Ok(())
}

fn js_backend_error(error: BackendError) -> JsValue {
    JsValue::from_str(&error.to_string())
}

fn js_value_to_string(value: &JsValue) -> String {
    value
        .as_string()
        .unwrap_or_else(|| "unknown JavaScript error".to_string())
}

fn show_startup_error(error: &JsValue) {
    web_sys::console::error_1(error);
    let message = format!("Failed to start typingmp: {}", js_value_to_string(error));
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    if let Some(wrapper) = document.get_element_by_id("canvas-wrapper") {
        wrapper.set_text_content(Some(&message));
    } else if let Some(body) = document.body() {
        body.set_text_content(Some(&message));
    }
}

fn report_provider_error(app: &mut App, context: &str, error: ProviderError) {
    let message = format!("{context}: {error}");
    web_sys::console::warn_1(&JsValue::from_str(&message));
    app.report_visible_error(message);
}

fn schedule_next_frame(callback: &AnimationFrameCallback) {
    let borrowed = callback.borrow();
    let Some(callback) = borrowed.as_ref() else {
        web_sys::console::error_1(&js_backend_error(BackendError::state(
            "animation frame callback is missing",
        )));
        return;
    };
    if let Err(err) = request_animation_frame(callback) {
        web_sys::console::error_1(&err);
    }
}

fn request_animation_frame(f: &Closure<dyn FnMut()>) -> Result<i32, JsValue> {
    let window = web_sys::window()
        .ok_or_else(|| js_backend_error(BackendError::dom("missing browser window")))?;
    window.request_animation_frame(f.as_ref().unchecked_ref())
}
