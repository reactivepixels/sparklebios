//! Local date helpers over libc.

/// Local calendar date for a unix timestamp, via libc::localtime_r.
/// Returns `(year, month, day)` with a four digit year, a 1 to 12 month
/// and a 1 to 31 day.
pub fn local_ymd(unix_secs: i64) -> (i32, u32, u32) {
    let time = unix_secs as libc::time_t;
    // SAFETY: `tm` is a C struct of plain integer and pointer fields, so an
    // all zero bit pattern is a valid value.
    let mut result: libc::tm = unsafe { std::mem::zeroed() };
    // SAFETY: `time` points at a valid, initialised `time_t` and `result`
    // points at a valid, writable `tm` we own for the duration of the call.
    unsafe {
        libc::localtime_r(&time, &mut result);
    }
    let year = result.tm_year + 1900;
    let month = (result.tm_mon + 1) as u32;
    let day = result.tm_mday as u32;
    (year, month, day)
}

/// The current unix time in seconds.
pub fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// "YYYY-MM-DD" in local time.
pub fn day_string(unix_secs: i64) -> String {
    let (year, month, day) = local_ymd(unix_secs);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn day_string_is_iso_shaped() {
        let s = day_string(now_unix() as i64);
        assert_eq!(s.len(), 10);
        assert_eq!(&s[4..5], "-");
        assert_eq!(&s[7..8], "-");
    }

    #[test]
    fn consecutive_days_differ() {
        let now = now_unix() as i64;
        assert_ne!(day_string(now), day_string(now - 86_400));
    }
}
