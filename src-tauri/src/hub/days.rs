//! The days next to the shown one that ‹ and › step to. Pure.

use chrono::NaiveDate;

/// The closest days before and after `shown` among the days with a daily note. Today always counts, with or
/// without a note, so the way back never skips it.
pub fn adjacent_days(
    note_days: &[NaiveDate],
    shown: NaiveDate,
    today: NaiveDate,
) -> (Option<NaiveDate>, Option<NaiveDate>) {
    let days = note_days.iter().copied().chain([today]);
    let previous = days.clone().filter(|day| *day < shown).max();
    let next = days.filter(|day| *day > shown).min();
    (previous, next)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, month, day).unwrap()
    }

    fn notes() -> Vec<NaiveDate> {
        vec![day(10, 12), day(10, 7), day(10, 9)]
    }

    #[test]
    fn steps_between_the_neighbouring_notes() {
        for (shown, expected) in [
            (day(10, 12), (Some(day(10, 9)), None)),
            (day(10, 9), (Some(day(10, 7)), Some(day(10, 12)))),
            (day(10, 7), (None, Some(day(10, 9)))),
        ] {
            assert_eq!(
                adjacent_days(&notes(), shown, day(10, 12)),
                expected,
                "{shown}"
            );
        }
    }

    #[test]
    fn today_is_a_stop_while_it_has_no_note() {
        assert_eq!(
            adjacent_days(&notes(), day(10, 12), day(10, 13)),
            (Some(day(10, 9)), Some(day(10, 13)))
        );
        assert_eq!(
            adjacent_days(&notes(), day(10, 13), day(10, 13)),
            (Some(day(10, 12)), None)
        );
    }

    #[test]
    fn a_note_dated_after_today_is_reached() {
        assert_eq!(
            adjacent_days(&notes(), day(10, 9), day(10, 8)),
            (Some(day(10, 8)), Some(day(10, 12)))
        );
        assert_eq!(
            adjacent_days(&notes(), day(10, 8), day(10, 8)),
            (Some(day(10, 7)), Some(day(10, 9)))
        );
    }

    #[test]
    fn without_notes_only_today_is_a_neighbour() {
        assert_eq!(
            adjacent_days(&[], day(10, 5), day(10, 8)),
            (None, Some(day(10, 8)))
        );
    }
}
