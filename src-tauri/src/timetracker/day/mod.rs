//! The Activity tab (docs/timetracker.md#the-activity-tab): TimeTracker's day, read-only, read through
//! its `recordings` and `suggestions` commands on request; nothing is kept on disk.

mod projects;
pub mod records;
mod reload;
mod timeline;
pub mod view;

use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::Serialize;
use ts_rs::TS;

pub use reload::{DayChange, reloads};
pub use view::{DayActivity, day_view};

use super::client::{self, TimeTracker, state_of};
use super::reply::TtProject;
use super::watch::TtState;
use crate::error::{AppResult, ErrorKind};

/// The Activity tab's answer: the day, or why there is none.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct TimeTrackerDay {
    pub state: TtState,
    /// `None` unless TimeTracker answered.
    pub activity: Option<DayActivity>,
}

/// Reads `day`'s recordings and suggestions and builds the tab; a TimeTracker that isn't running or
/// isn't installed is a state, a refusal stays an error.
pub fn read_day(
    tt: &dyn TimeTracker,
    projects: &[TtProject],
    day: NaiveDate,
    now: DateTime<FixedOffset>,
) -> AppResult<TimeTrackerDay> {
    match read_both(tt, day) {
        Ok((recordings, suggestions)) => Ok(TimeTrackerDay {
            state: TtState::Available,
            activity: Some(day_view(&recordings, &suggestions, projects, now, day)),
        }),
        Err(e) if e.kind() == ErrorKind::TimetrackerRefused => Err(e),
        Err(e) => Ok(TimeTrackerDay {
            state: state_of(&e),
            activity: None,
        }),
    }
}

fn read_both(
    tt: &dyn TimeTracker,
    day: NaiveDate,
) -> AppResult<(Vec<records::Recording>, Vec<records::Suggestion>)> {
    let recordings = client::recordings(tt, day)?;
    let suggestions = client::suggestions(tt, day)?;
    Ok((recordings, suggestions))
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::timetracker::client::{RawReply, RunFailure, Watch};

    struct Fake(Result<RawReply, RunFailure>);

    impl TimeTracker for Fake {
        fn run(&self, _args: &[&str]) -> Result<RawReply, RunFailure> {
            self.0.clone()
        }

        fn watch(&self) -> Result<Box<dyn Watch>, RunFailure> {
            Err(RunFailure::Missing)
        }
    }

    fn answering(code: i32, stdout: &str, stderr: &str) -> Fake {
        Fake(Ok(RawReply {
            code: Some(code),
            stdout: stdout.into(),
            stderr: stderr.into(),
        }))
    }

    fn day() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 23).unwrap()
    }

    fn now() -> DateTime<FixedOffset> {
        FixedOffset::east_opt(2 * 3600)
            .unwrap()
            .with_ymd_and_hms(2026, 10, 23, 12, 0, 0)
            .unwrap()
    }

    #[test]
    fn an_answering_timetracker_gives_the_available_day() {
        let day_read = read_day(&answering(0, "[]", ""), &[], day(), now()).unwrap();
        assert_eq!(day_read.state, TtState::Available);
        let activity = day_read.activity.unwrap();
        assert_eq!(activity.day, "2026-10-23");
        assert_eq!(activity.empty.as_deref(), Some("Nothing recorded yet"));
    }

    #[test]
    fn a_timetracker_that_is_not_running_is_a_state_without_a_day() {
        let tt = answering(2, "", "TimeTracker is not running");
        assert_eq!(
            read_day(&tt, &[], day(), now()),
            Ok(TimeTrackerDay {
                state: TtState::NotRunning,
                activity: None,
            })
        );
    }

    #[test]
    fn a_timetracker_that_does_not_answer_in_time_is_not_running() {
        assert_eq!(
            read_day(&Fake(Err(RunFailure::TimedOut)), &[], day(), now()),
            Ok(TimeTrackerDay {
                state: TtState::NotRunning,
                activity: None,
            })
        );
    }

    #[test]
    fn a_missing_timetracker_is_a_state_without_a_day() {
        assert_eq!(
            read_day(&Fake(Err(RunFailure::Missing)), &[], day(), now()),
            Ok(TimeTrackerDay {
                state: TtState::Missing,
                activity: None,
            })
        );
    }

    #[test]
    fn a_refusal_stays_an_error() {
        let err = read_day(&answering(1, "", "boom"), &[], day(), now()).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::TimetrackerRefused);
    }

    #[test]
    fn an_unreadable_reply_is_a_refusal() {
        let err = read_day(&answering(0, "{nope", ""), &[], day(), now()).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::TimetrackerRefused);
    }

    #[test]
    fn an_unavailable_day_serializes_with_a_camel_case_state_and_no_activity() {
        let tt = Fake(Err(RunFailure::Missing));
        let value = serde_json::to_value(read_day(&tt, &[], day(), now()).unwrap()).unwrap();
        assert_eq!(
            value,
            serde_json::json!({"state": "missing", "activity": null})
        );
    }
}
