// src/timestamp.rs

#[cfg(not(target_arch = "wasm32"))]
pub fn now() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as f64
}

#[cfg(target_arch = "wasm32")]
pub fn now() -> f64 {
    js_sys::Date::now()
}