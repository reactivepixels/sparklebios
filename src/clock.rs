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

/// A sequential day number in local time: the same fixed epoch `days_from_civil` counts from,
/// applied to the local calendar date for `unix_secs`. Two moments on the same local day always
/// give the same number, and consecutive local days are always one apart.
pub fn day_number(unix_secs: i64) -> i64 {
    let (year, month, day) = local_ymd(unix_secs);
    days_from_civil(year, month, day)
}

/// The day the calendar chip is on the run against: 2038-01-19, the last day the classic signed
/// 32-bit unix clock can name before it wraps.
const Y2038_YEAR: i32 = 2038;
const Y2038_MONTH: u32 = 1;
const Y2038_DAY: u32 = 19;

/// Days since an arbitrary but fixed epoch for `(year, month, day)`, proleptic Gregorian, valid
/// for any year with `month` 1 to 12 and `day` a valid day of that month. Hand written (no date
/// library) after Howard Hinnant's public domain `days_from_civil` algorithm: only the difference
/// between two calls means anything, never the value itself.
fn days_from_civil(year: i32, month: u32, day: u32) -> i64 {
    let y = if month <= 2 {
        year as i64 - 1
    } else {
        year as i64
    };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let year_of_era = y - era * 400; // [0, 399]
    let month_index = (month as i64 + 9) % 12; // [0, 11], March is 0
    let day_of_year = (153 * month_index + 2) / 5 + day as i64 - 1; // [0, 365]
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year; // [0, 146096]
    era * 146097 + day_of_era - 719468
}

/// Whole days from `(year, month, day)` to 2038-01-19. Negative once that day is behind us.
pub fn days_until_y2038(year: i32, month: u32, day: u32) -> i64 {
    days_from_civil(Y2038_YEAR, Y2038_MONTH, Y2038_DAY) - days_from_civil(year, month, day)
}

/// True when `year` is a leap year in the proleptic Gregorian calendar.
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

const DAYS_IN_MONTH: [u32; 12] = [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];

/// The 1 based day of the year for `(year, month, day)`: 1 January is day 1, and December 31 is
/// day 365, or 366 in a leap year.
pub fn day_of_year(year: i32, month: u32, day: u32) -> u32 {
    let mut total = day;
    for m in 1..month {
        total += DAYS_IN_MONTH[(m - 1) as usize];
        if m == 2 && is_leap_year(year) {
            total += 1;
        }
    }
    total
}

const WEEKDAY_NAMES: [&str; 7] = [
    "sunday",
    "monday",
    "tuesday",
    "wednesday",
    "thursday",
    "friday",
    "saturday",
];

/// The lowercase weekday name for `(year, month, day)`: `days_from_civil` counts from a fixed
/// epoch, and 1970-01-01 (day 0 on that count) is a known Thursday, so every other day's weekday
/// is that offset, taken modulo 7.
pub fn weekday_name(year: i32, month: u32, day: u32) -> &'static str {
    let days = days_from_civil(year, month, day);
    let index = ((days + 4).rem_euclid(7)) as usize;
    WEEKDAY_NAMES[index]
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

    // --- days_until_y2038 ---------------------------------------------------------------------

    #[test]
    fn days_until_y2038_is_zero_on_the_day_itself_by_hand() {
        // 2038-01-19 is the target day itself: zero days to go.
        assert_eq!(days_until_y2038(2038, 1, 19), 0);
    }

    #[test]
    fn days_until_y2038_counts_the_day_before_and_after_the_boundary() {
        assert_eq!(days_until_y2038(2038, 1, 18), 1);
        assert_eq!(days_until_y2038(2038, 1, 20), -1);
    }

    #[test]
    fn days_until_y2038_matches_a_known_date_by_hand() {
        // Checked independently: 2026-09-19 to 2038-01-19 is 4140 days.
        assert_eq!(days_until_y2038(2026, 9, 19), 4140);
    }

    // --- day_of_year ----------------------------------------------------------------------------

    #[test]
    fn day_of_year_matches_a_known_date_by_hand() {
        // 1 January is always day 1.
        assert_eq!(day_of_year(2026, 1, 1), 1);
    }

    #[test]
    fn day_of_year_256_lands_on_13_september_in_a_common_year_and_12_in_a_leap_year() {
        // 2025 is not a leap year: day 256 is 13 September.
        assert_eq!(day_of_year(2025, 9, 13), 256);
        assert_ne!(day_of_year(2025, 9, 12), 256);
        // 2024 is a leap year: the extra day in February shifts day 256 one day earlier.
        assert_eq!(day_of_year(2024, 9, 12), 256);
        assert_ne!(day_of_year(2024, 9, 13), 256);
    }

    #[test]
    fn day_of_year_counts_the_day_before_and_after_the_leap_day_boundary() {
        // 2024 is a leap year: 29 February exists, and everything after it is one day later in
        // the year than the same date would be in a common year.
        assert_eq!(day_of_year(2024, 2, 28), 59);
        assert_eq!(day_of_year(2024, 2, 29), 60);
        assert_eq!(day_of_year(2024, 3, 1), 61);
        // 2023 is a common year: no 29 February, so 1 March follows straight on from 28 February.
        assert_eq!(day_of_year(2023, 2, 28), 59);
        assert_eq!(day_of_year(2023, 3, 1), 60);
    }

    // --- weekday_name ---------------------------------------------------------------------------

    #[test]
    fn weekday_name_matches_a_known_thursday_by_hand() {
        // The unix epoch, 1970-01-01, is a Thursday: a widely known reference point.
        assert_eq!(weekday_name(1970, 1, 1), "thursday");
    }

    #[test]
    fn weekday_name_finds_friday_the_13th_and_a_13th_that_is_not_friday() {
        // 13 September 2024 is a real Friday the 13th.
        assert_eq!(weekday_name(2024, 9, 13), "friday");
        // 13 September 2026 is a Sunday: the same day of the month, a different day of the week.
        assert_eq!(weekday_name(2026, 9, 13), "sunday");
    }

    #[test]
    fn weekday_name_counts_the_day_before_and_after_a_week_boundary() {
        // 2026-09-19 (a Saturday) rolls over into a new week the next day.
        assert_eq!(weekday_name(2026, 9, 19), "saturday");
        assert_eq!(weekday_name(2026, 9, 20), "sunday");
    }
}
