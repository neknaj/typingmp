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
// ここでは単純なカウンターで代用する。より正確な実装には`uefi-services`のTimerProtocolなどが必要。
#[cfg(feature = "uefi")]
pub fn now() -> f64 {
    // 簡単のため、ここではブートからの経過時間(マイクロ秒)をミリ秒に変換して返す。
    uefi::boot::running_boot_services().timer().get_time().as_micros() as f64 / 1000.0
}