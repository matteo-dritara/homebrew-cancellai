//! ISO-8601 UTC formatting for `Timestamp` (`docs/architecture/JSON_CONTRACTS.md`'s common
//! envelope: `generated_at` is an "ISO-8601 UTC timestamp").
//!
//! Hand-rolled rather than pulling in a date/time crate (AGENTS.md: "Do not add a dependency
//! merely to reduce implementation effort") - civil-date conversion from a day count is a
//! well-known, bounded algorithm (Howard Hinnant's `civil_from_days`, public domain,
//! <http://howardhinnant.github.io/date_algorithms.html>), not something that benefits from an
//! external crate's surface area on this one call site.

use cancellai_platform::Timestamp;

/// Days-since-epoch -> proleptic Gregorian (year, month, day). Valid for every `i64` day count
/// this codebase can produce (any `u64` seconds-since-epoch fits comfortably).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

/// Formats `timestamp` (seconds since the Unix epoch) as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn to_iso8601_utc(timestamp: Timestamp) -> String {
    let secs = timestamp.0 as i64;
    let days = secs.div_euclid(86_400);
    let time_of_day = secs.rem_euclid(86_400);
    let (year, month, day) = civil_from_days(days);
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    let second = time_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_unix_epoch_itself_formats_correctly() {
        assert_eq!(to_iso8601_utc(Timestamp(0)), "1970-01-01T00:00:00Z");
    }

    #[test]
    fn a_known_recent_timestamp_formats_correctly() {
        // 2026-08-28T09:00:00Z, matching the golden fixtures' own `generated_at`.
        assert_eq!(
            to_iso8601_utc(Timestamp(1_787_907_600)),
            "2026-08-28T09:00:00Z"
        );
    }

    #[test]
    fn a_leap_day_formats_correctly() {
        // 2024-02-29T00:00:00Z.
        assert_eq!(
            to_iso8601_utc(Timestamp(1_709_164_800)),
            "2024-02-29T00:00:00Z"
        );
    }
}
