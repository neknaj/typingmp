// src/timestamp.rs

#[cfg(all(not(target_arch = "wasm32"), not(feature = "uefi")))]
pub fn now() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as f64
}

#[cfg(target_arch = "wasm32")]
pub fn now() -> f64 {
    js_sys::Date::now()
}

// UEFI環境では高解像度の単調タイマーを簡単に取得できないため、
// uefi.rs のメインループで時間を管理する。
// この関数は理論上呼ばれないが、万が一のために0.0を返す。
#[cfg(feature = "uefi")]
pub fn now() -> f64 {
    // この実装は使われない。
    // uefi.rsで直接タイムスタンプを生成してTypingInputに渡す。
    0.0
}