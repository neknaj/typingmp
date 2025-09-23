#![cfg(debug_assertions)]

use std::cell::RefCell;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CloseEvent, Event, WebSocket};

// 通常のログ用
fn console_log(message: &str) {
    web_sys::console::log_1(&message.into());
}

// スタイル付きログ用（エラー、成功など）
fn console_log_styled(message: &str, style: &str) {
    web_sys::console::log_2(&format!("%c{}", message).into(), &style.into());
}

thread_local! {
    static WS_CONNECTION: RefCell<Option<WebSocket>> = RefCell::new(None);
    static CONNECTION_ID: RefCell<String> = RefCell::new(String::new());
}

pub fn init() {
    console_log("[WASM Logger] Initializing...");

    let mut buf = [0u8; 16];
    getrandom::getrandom(&mut buf).expect("Failed to get random bytes");
    let id = buf.iter().map(|b| format!("{:02x}", b)).collect::<String>();
    CONNECTION_ID.with(|cell| *cell.borrow_mut() = id.clone());
    console_log(&format!("[WASM Logger] Generated Connection ID: {}", id));
    
    // !!! ここはあなたのPCのIPアドレスに要変更 !!!
    let server_address = "ws://localhost:8081";
    console_log(&format!("[WASM Logger] Attempting to connect to: {}", server_address));

    match WebSocket::new(server_address) {
        Ok(ws) => {
            console_log("[WASM Logger] WebSocket::new() succeeded. Setting callbacks...");

            // 接続成功時のコールバック
            let onopen_callback = Closure::<dyn FnMut()>::new(|| {
                console_log_styled("[WASM Logger] WebSocket connection opened successfully! (onopen)", "color: green; font-weight: bold;");
                log_internal("Connection established.");
            });
            ws.set_onopen(Some(onopen_callback.as_ref().unchecked_ref()));
            onopen_callback.forget();

            // エラー発生時のコールバック
            let onerror_callback = Closure::<dyn FnMut(_)>::new(|e: Event| {
                // ErrorEventは詳細情報を持たないので、基本的なEventとしてログ出力
                console_log_styled(&format!("[WASM Logger] WebSocket error occurred. See browser devtools for details."), "color: red; font-weight: bold;");
            });
            ws.set_onerror(Some(onerror_callback.as_ref().unchecked_ref()));
            onerror_callback.forget();
            
            // 切断時のコールバック
            let onclose_callback = Closure::<dyn FnMut(_)>::new(|e: CloseEvent| {
                console_log_styled(&format!(
                    "[WASM Logger] WebSocket connection closed. Code: {}, Reason: '{}'",
                    e.code(),
                    e.reason(),
                ), "color: orange;");
            });
            ws.set_onclose(Some(onclose_callback.as_ref().unchecked_ref()));
            onclose_callback.forget();
            
            WS_CONNECTION.with(|cell| *cell.borrow_mut() = Some(ws));
        }
        Err(e) => {
            console_log_styled(&format!("[WASM Logger] WebSocket::new() failed: {:?}", e), "color: red; font-weight: bold;");
        }
    }
}

pub fn log(message: &str) {
    log_internal(message);
}

fn log_internal(message: &str) {
    WS_CONNECTION.with(|ws_cell| {
        if let Some(ws) = ws_cell.borrow().as_ref() {
            if ws.ready_state() == WebSocket::OPEN {
                CONNECTION_ID.with(|id_cell| {
                    let id = id_cell.borrow();
                    let escaped_message = message.replace('\\', "\\\\").replace('"', "\\\"");
                    let payload = format!(r#"{{"id":"{}","message":"{}"}}"#, id, escaped_message);
                    
                    match ws.send_with_str(&payload) {
                        Ok(_) => console_log(&format!("[WASM Logger] Sent log: {}", message)),
                        Err(e) => console_log_styled(&format!("[WASM Logger] Failed to send log: {:?}", e), "color: red;"),
                    }
                });
            } else {
                console_log(&format!(
                    "[WASM Logger] Log not sent. WebSocket state is not OPEN (state: {}).",
                    ws.ready_state()
                ));
            }
        }
    });
}