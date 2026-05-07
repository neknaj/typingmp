// src/timestamp.rs

#[cfg(all(not(feature = "wasm"), not(feature = "uefi")))]
pub fn now() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as f64,
        Err(_) => 0.0,
    }
}

#[cfg(feature = "wasm")]
pub fn now() -> f64 {
    js_sys::Date::now()
}

#[cfg(feature = "uefi")]
pub fn now() -> f64 {
    use uefi::runtime;

    match runtime::get_time() {
        Ok(time) => unix_timestamp_ms(
            time.year(),
            time.month(),
            time.day(),
            time.hour(),
            time.minute(),
            time.second(),
            time.nanosecond(),
        ),
        Err(_) => 0.0,
    }
}

#[cfg(any(test, feature = "uefi"))]
fn unix_timestamp_ms(
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
    nanosecond: u32,
) -> f64 {
    const MILLIS_PER_SECOND: u64 = 1_000;
    const MILLIS_PER_MINUTE: u64 = 60 * MILLIS_PER_SECOND;
    const MILLIS_PER_HOUR: u64 = 60 * MILLIS_PER_MINUTE;
    const MILLIS_PER_DAY: u64 = 24 * MILLIS_PER_HOUR;
    const MONTH_DAYS: [u16; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

    if year < 1970 || !(1..=12).contains(&month) || day == 0 {
        return 0.0;
    }

    let mut days = 0u64;
    for current_year in 1970..year {
        days += if is_leap_year(current_year) { 366 } else { 365 };
    }

    for current_month in 1..month {
        let mut month_days = MONTH_DAYS[(current_month - 1) as usize];
        if current_month == 2 && is_leap_year(year) {
            month_days += 1;
        }
        days += u64::from(month_days);
    }

    let max_day = {
        let mut month_days = MONTH_DAYS[(month - 1) as usize];
        if month == 2 && is_leap_year(year) {
            month_days += 1;
        }
        month_days
    };
    if u16::from(day) > max_day || hour > 23 || minute > 59 || second > 60 {
        return 0.0;
    }

    days += u64::from(day - 1);
    let millis = days * MILLIS_PER_DAY
        + u64::from(hour) * MILLIS_PER_HOUR
        + u64::from(minute) * MILLIS_PER_MINUTE
        + u64::from(second) * MILLIS_PER_SECOND;
    millis as f64 + nanosecond as f64 / 1_000_000.0
}

#[cfg(any(test, feature = "uefi"))]
fn is_leap_year(year: u16) -> bool {
    year.is_multiple_of(4) && !year.is_multiple_of(100) || year.is_multiple_of(400)
}

#[cfg(test)]
mod tests {
    use super::unix_timestamp_ms;

    #[test]
    fn unix_timestamp_handles_leap_days() {
        assert_eq!(unix_timestamp_ms(1970, 1, 1, 0, 0, 0, 0), 0.0);
        assert_eq!(unix_timestamp_ms(1970, 1, 2, 0, 0, 0, 0), 86_400_000.0);
        assert_eq!(
            unix_timestamp_ms(1972, 3, 1, 0, 0, 0, 0),
            790.0 * 86_400_000.0
        );
    }

    #[test]
    fn unix_timestamp_rejects_invalid_dates() {
        assert_eq!(unix_timestamp_ms(1969, 12, 31, 23, 59, 59, 0), 0.0);
        assert_eq!(unix_timestamp_ms(2026, 2, 29, 0, 0, 0, 0), 0.0);
        assert_eq!(
            unix_timestamp_ms(2024, 2, 29, 0, 0, 0, 0),
            1_709_164_800_000.0
        );
    }
}
