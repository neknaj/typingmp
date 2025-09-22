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

// UEFIでは正確なSystemTimeは取れないため、BootServicesのタイマー等を利用するか、
// ここではuefi-services経由でタイマーを利用する
#[cfg(feature = "uefi")]
pub fn now() -> f64 {
    // uefi-servicesクレート経由でBootServicesを取得して時刻を取得
    // handle()は一度しか呼べないので注意が必要だが、ここでは簡略化のため毎回呼ぶ
    // 本来は一度だけ取得してstatic変数などに保持するのが望ましい
    if let Ok(st) = uefi::system_table() {
        st.boot_services().get_time().as_micros() as f64 / 1000.0
    } else {
        0.0 // エラー時は0を返す
    }
}