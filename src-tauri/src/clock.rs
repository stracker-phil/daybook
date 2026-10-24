//! Wall-clock helpers: `HH:MM` strings, minutes since midnight and weekly time windows. Pure.

use chrono::{Datelike, NaiveDateTime, Timelike};

const MINUTES_PER_DAY: u32 = 24 * 60;

/// `"09:45"` and `"9:45"` → 585; `None` when it isn't a time of day.
pub fn parse_clock(value: &str) -> Option<u32> {
    let (hours, minutes) = value.trim().split_once(':')?;
    let digits = |part: &str, len: std::ops::RangeInclusive<usize>| {
        (len.contains(&part.len()) && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse::<u32>().ok())
            .flatten()
    };
    let (hours, minutes) = (digits(hours, 1..=2)?, digits(minutes, 2..=2)?);
    (hours <= 23 && minutes <= 59).then_some(hours * 60 + minutes)
}

/// 585 → `"09:45"`
pub fn format_clock(minutes: u32) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

pub fn minute_of_day(now: NaiveDateTime) -> u32 {
    now.hour() * 60 + now.minute()
}

/// Minutes from `time` (today) until `now`, wrapping past midnight; `None` without a time.
pub fn minutes_since(time: Option<&str>, now: NaiveDateTime) -> Option<u32> {
    let start = parse_clock(time?)?;
    Some((minute_of_day(now) + MINUTES_PER_DAY - start) % MINUTES_PER_DAY)
}

/// On `days` (0 = Sunday; empty: every day) from `from` until `to`. An empty `to` runs to the end of
/// the day; a `to` before `from` runs past midnight.
pub fn is_within_window(days: &[u8], from: &str, to: &str, now: NaiveDateTime) -> bool {
    let weekday = now.weekday().num_days_from_sunday() as u8;
    if !days.is_empty() && !days.contains(&weekday) {
        return false;
    }
    let Some(from) = parse_clock(from) else {
        return false;
    };
    let minute = minute_of_day(now);
    match parse_clock(to) {
        None => minute >= from,
        Some(to) if from <= to => minute >= from && minute < to,
        Some(to) => minute >= from || minute < to,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    /// 12 Oct 2026 is a Monday.
    fn at(hour: u32, minute: u32, day: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    fn monday(hour: u32, minute: u32) -> NaiveDateTime {
        at(hour, minute, 12)
    }

    #[test]
    fn clock_times_are_read_with_or_without_a_leading_zero_and_surrounding_spaces() {
        for (input, expected) in [
            ("9:45", Some(585)),
            ("09:45", Some(585)),
            (" 09:45 ", Some(585)),
            ("00:00", Some(0)),
            ("23:59", Some(1439)),
        ] {
            assert_eq!(parse_clock(input), expected, "{input:?}");
        }
    }

    #[test]
    fn text_that_is_not_a_time_of_day_is_rejected() {
        for input in [
            "24:00", "12:60", "nine", "", "9", "9:5", "9:456", "123:00", "-1:00", "+9:00", "9:4x",
        ] {
            assert_eq!(parse_clock(input), None, "{input:?}");
        }
    }

    #[test]
    fn minutes_are_written_as_hh_mm() {
        for (minutes, expected) in [(585, "09:45"), (0, "00:00"), (1439, "23:59")] {
            assert_eq!(format_clock(minutes), expected, "{minutes}");
        }
    }

    #[test]
    fn minutes_since_counts_from_a_time_today() {
        assert_eq!(minutes_since(Some("09:00"), monday(10, 30)), Some(90));
        assert_eq!(minutes_since(Some("10:30"), monday(10, 30)), Some(0));
    }

    #[test]
    fn minutes_since_wraps_past_midnight() {
        assert_eq!(minutes_since(Some("23:30"), monday(0, 15)), Some(45));
    }

    #[test]
    fn minutes_since_is_none_without_a_valid_time() {
        assert_eq!(minutes_since(None, monday(10, 0)), None);
        assert_eq!(minutes_since(Some("nine"), monday(10, 0)), None);
    }

    #[test]
    fn a_workday_window_contains_its_hours_on_its_days_only() {
        let days = [1, 2, 3, 4, 5];
        for (now, expected) in [
            (monday(10, 0), true),
            (monday(9, 0), true),
            (monday(8, 59), false),
            (monday(18, 0), false),
            (at(10, 0, 11), false),
            (at(10, 0, 17), false),
        ] {
            assert_eq!(
                is_within_window(&days, "09:00", "18:00", now),
                expected,
                "{now}"
            );
        }
    }

    #[test]
    fn an_empty_end_runs_to_the_end_of_the_day_on_every_day() {
        for (now, expected) in [
            (monday(23, 59), true),
            (monday(17, 30), true),
            (monday(17, 29), false),
            (at(18, 0, 11), true),
        ] {
            assert_eq!(is_within_window(&[], "17:30", "", now), expected, "{now}");
        }
    }

    #[test]
    fn an_end_before_the_start_runs_past_midnight() {
        for (now, expected) in [
            (monday(23, 0), true),
            (monday(1, 0), true),
            (monday(2, 0), false),
            (monday(3, 0), false),
            (monday(21, 59), false),
        ] {
            assert_eq!(
                is_within_window(&[], "22:00", "02:00", now),
                expected,
                "{now}"
            );
        }
    }

    #[test]
    fn a_window_without_a_valid_start_never_matches() {
        assert!(!is_within_window(&[], "", "18:00", monday(10, 0)));
        assert!(!is_within_window(&[], "soon", "", monday(10, 0)));
    }

    #[test]
    fn sunday_is_day_zero() {
        assert!(is_within_window(&[0], "00:00", "", at(12, 0, 11)));
        assert!(!is_within_window(&[0], "00:00", "", monday(12, 0)));
    }
}
