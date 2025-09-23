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

#[cfg(feature = "uefi")]
pub fn now() -> f64 {
    use uefi::runtime;
    // uefi::println!("get time");
    let res = runtime::get_time().unwrap();
    let mut timestamp_ms: f64 = 0.0;
    timestamp_ms += res.year() as f64 * 31_536_000_000.0; // 1年
    timestamp_ms += res.month() as f64 * 2_628_000_000.0; // 1ヶ月
    timestamp_ms += res.day() as f64 * 86_400_000.0; // 1日
    timestamp_ms += res.hour() as f64 * 3_600_000.0; // 1時間
    timestamp_ms += res.minute() as f64 * 60_000.0; // 1分
    timestamp_ms += res.second() as f64 * 1_000.0; // 1秒
    timestamp_ms += res.nanosecond() as f64 / 1_000_000.0; // ナノ秒をミリ秒に
    timestamp_ms
}