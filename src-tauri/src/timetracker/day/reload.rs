//! When the Activity tab reads the shown day again. Pure.

use chrono::NaiveDate;

use crate::timetracker::reply::{DataChange, DataKind};

/// News from TimeTracker that may change the shown day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DayChange {
    Data(DataChange),
    Tracking {
        tt_changed: bool,
        timer_changed: bool,
    },
}

/// Data that shapes the shown day (or every day) reloads it (`recordings` covers stored suggestions too);
/// a timer change reloads today; TimeTracker starting or quitting reloads any day.
pub fn reloads(change: DayChange, shown: NaiveDate, today: NaiveDate) -> bool {
    match change {
        DayChange::Data(DataChange { kind, day }) => {
            matches!(
                kind,
                DataKind::Recordings | DataKind::Rules | DataKind::Tags | DataKind::Projects
            ) && day.is_none_or(|day| day == shown)
        }
        DayChange::Tracking {
            tt_changed,
            timer_changed,
        } => tt_changed || (timer_changed && shown == today),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    fn data(kind: DataKind, day: Option<NaiveDate>) -> DayChange {
        DayChange::Data(DataChange { kind, day })
    }

    #[test]
    fn data_that_shapes_the_day_reloads_for_the_shown_day_or_any_day() {
        for kind in [
            DataKind::Recordings,
            DataKind::Rules,
            DataKind::Tags,
            DataKind::Projects,
        ] {
            assert!(
                reloads(data(kind, None), date(22), date(23)),
                "{kind:?} any day"
            );
            assert!(
                reloads(data(kind, Some(date(22))), date(22), date(23)),
                "{kind:?} shown day"
            );
        }
    }

    #[test]
    fn data_of_another_day_does_not_reload() {
        for kind in [
            DataKind::Recordings,
            DataKind::Rules,
            DataKind::Tags,
            DataKind::Projects,
        ] {
            assert!(
                !reloads(data(kind, Some(date(21))), date(22), date(23)),
                "{kind:?}"
            );
        }
    }

    #[test]
    fn other_data_never_reloads() {
        assert!(!reloads(data(DataKind::Other, None), date(22), date(23)));
        assert!(!reloads(
            data(DataKind::Other, Some(date(22))),
            date(22),
            date(23)
        ));
    }

    #[test]
    fn tracking_reloads_when_timetracker_started_or_quit_on_any_day() {
        let change = DayChange::Tracking {
            tt_changed: true,
            timer_changed: false,
        };
        assert!(reloads(change, date(22), date(23)));
        assert!(reloads(change, date(23), date(23)));
    }

    #[test]
    fn a_timer_change_reloads_only_today() {
        let change = DayChange::Tracking {
            tt_changed: false,
            timer_changed: true,
        };
        assert!(reloads(change, date(23), date(23)));
        assert!(!reloads(change, date(22), date(23)));
    }

    #[test]
    fn tracking_without_a_change_does_not_reload() {
        let change = DayChange::Tracking {
            tt_changed: false,
            timer_changed: false,
        };
        assert!(!reloads(change, date(23), date(23)));
    }
}
