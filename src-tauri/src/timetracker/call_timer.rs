//! A call pauses the running timer and resumes it when it ends, so the call's suggestion is time no
//! recording covers and TimeTracker shows it (docs/timetracker.md#calls-become-suggestions). Pure.

use super::reply::TimerState;
use crate::store::settings::Status;

/// The timer a call stopped, to start again with the same project, note and tag (TimeTracker then
/// continues today's recording).
#[derive(Clone, Debug, PartialEq)]
pub struct PausedTimer {
    pub project_id: String,
    pub project: String,
    /// Empty without a note.
    pub note: String,
    pub tag: Option<String>,
}

/// What a call's start sees.
pub struct CallStart<'a> {
    /// Settings → Workflows, "Start and stop the timer".
    pub status_timer: bool,
    pub status: Option<Status>,
    pub status_feature: bool,
    pub timer: Option<&'a TimerState>,
}

/// The running timer to stop for the call: only while Working, in a context with the Status feature,
/// with the status driving the timer.
pub fn pause_for_call(start: &CallStart) -> Option<PausedTimer> {
    if !start.status_timer || start.status != Some(Status::Working) || !start.status_feature {
        return None;
    }
    start.timer.map(|timer| PausedTimer {
        project_id: timer.project_id.clone(),
        project: timer.project.clone(),
        note: timer.note.clone().unwrap_or_default(),
        tag: timer.tag.clone(),
    })
}

/// The paused timer to start again at the call's end: only while still Working and no other timer
/// started meanwhile.
pub fn resume_after_call(
    paused: PausedTimer,
    status: Option<Status>,
    timer_running: bool,
) -> Option<PausedTimer> {
    (status == Some(Status::Working) && !timer_running).then_some(paused)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timer(note: Option<&str>) -> TimerState {
        TimerState {
            recording_id: "r1".into(),
            project_id: "p1".into(),
            project: "Apollo".into(),
            client: None,
            tag_id: Some("t1".into()),
            tag: Some("deep".into()),
            note: note.map(String::from),
            started_at: 0,
        }
    }

    fn paused(note: &str) -> PausedTimer {
        PausedTimer {
            project_id: "p1".into(),
            project: "Apollo".into(),
            note: note.into(),
            tag: Some("deep".into()),
        }
    }

    fn start(timer: Option<&TimerState>) -> CallStart<'_> {
        CallStart {
            status_timer: true,
            status: Some(Status::Working),
            status_feature: true,
            timer,
        }
    }

    #[test]
    fn a_running_timer_is_paused_for_a_call_while_working() {
        let running = timer(Some("Review"));
        assert_eq!(
            pause_for_call(&start(Some(&running))),
            Some(paused("Review"))
        );
    }

    #[test]
    fn a_timer_without_a_note_is_paused_with_an_empty_note() {
        let running = timer(None);
        assert_eq!(pause_for_call(&start(Some(&running))), Some(paused("")));
    }

    #[test]
    fn no_timer_is_paused_when_any_condition_fails() {
        let running = timer(Some("Review"));
        let cases = [
            (
                "timer setting off",
                CallStart {
                    status_timer: false,
                    ..start(Some(&running))
                },
            ),
            (
                "on a break",
                CallStart {
                    status: Some(Status::Break),
                    ..start(Some(&running))
                },
            ),
            (
                "off",
                CallStart {
                    status: Some(Status::Off),
                    ..start(Some(&running))
                },
            ),
            (
                "no status",
                CallStart {
                    status: None,
                    ..start(Some(&running))
                },
            ),
            (
                "no status feature",
                CallStart {
                    status_feature: false,
                    ..start(Some(&running))
                },
            ),
            ("no timer running", start(None)),
        ];
        for (name, call_start) in cases {
            assert_eq!(pause_for_call(&call_start), None, "{name}");
        }
    }

    #[test]
    fn the_paused_timer_resumes_only_while_working_and_no_other_timer_runs() {
        for (status, running, resumes) in [
            (Some(Status::Working), false, true),
            (Some(Status::Working), true, false),
            (Some(Status::Break), false, false),
            (Some(Status::Off), false, false),
            (None, false, false),
        ] {
            assert_eq!(
                resume_after_call(paused("x"), status, running),
                resumes.then(|| paused("x")),
                "{status:?} running={running}"
            );
        }
    }
}
