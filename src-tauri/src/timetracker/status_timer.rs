//! What a status change does to TimeTracker's timer: Working continues the last timer, Break and Off
//! stop it (docs/timetracker.md#status-and-calls). The timer never changes the status. Pure.

use super::reply::RecentTimer;
use super::watch::{Tracking, TtState};
use crate::status::StatusChanged;
use crate::store::settings::{Project, Status};

/// What Daybook knows of the timer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimerKnown {
    Running,
    Idle,
    /// TimeTracker isn't reachable, or hasn't answered yet.
    Unknown,
}

impl TimerKnown {
    pub fn of(tracking: &Tracking) -> Self {
        match (tracking.tt, &tracking.timer) {
            (TtState::Available, Some(_)) => Self::Running,
            (TtState::Available, None) => Self::Idle,
            _ => Self::Unknown,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TimerStep {
    /// `timer start`; TimeTracker continues today's recording with the same project, note and tag.
    Start {
        project_id: String,
        note: String,
        tag: Option<String>,
    },
    Stop,
    /// Opens the activity overlay.
    Ask,
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Intent {
    Nothing,
    Stop,
    Resume,
}

fn intent(change: &StatusChanged, timer: TimerKnown) -> Intent {
    // The activity pick started its own timer.
    if change.by_activity {
        return Intent::Nothing;
    }
    match change.next {
        Status::Working if change.previous == Some(Status::Working) => Intent::Nothing,
        // A `timer start` would replace what TimeTracker times.
        Status::Working if timer == TimerKnown::Running => Intent::Nothing,
        Status::Working => Intent::Resume,
        Status::Break | Status::Off if timer == TimerKnown::Idle => Intent::Nothing,
        Status::Break | Status::Off => Intent::Stop,
    }
}

/// Whether `timer_step` needs the last timer (`timer recent --limit 1`).
pub fn needs_last_timer(change: &StatusChanged, timer: TimerKnown) -> bool {
    intent(change, timer) == Intent::Resume
}

fn is_offered(offered: &[&Project], tt_project_id: &str) -> bool {
    offered.iter().any(|project| {
        project
            .timetracker_projects
            .iter()
            .any(|linked| linked.id == tt_project_id)
    })
}

/// `last`: the newest of `timer recent`; `offered`: the current context's projects. Without a last
/// timer on an offered project the overlay asks, only in a context with the Status feature.
pub fn timer_step(
    change: &StatusChanged,
    timer: TimerKnown,
    last: Option<&RecentTimer>,
    offered: &[&Project],
    status_feature: bool,
) -> TimerStep {
    match intent(change, timer) {
        Intent::Nothing => TimerStep::Nothing,
        Intent::Stop => TimerStep::Stop,
        Intent::Resume => match last.filter(|last| is_offered(offered, &last.project_id)) {
            Some(last) => TimerStep::Start {
                project_id: last.project_id.clone(),
                note: last.note.clone(),
                tag: last.tag.clone(),
            },
            None if status_feature => TimerStep::Ask,
            None => TimerStep::Nothing,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::super::reply::TimerState;
    use super::*;
    use crate::store::settings::LinkedProject;

    fn change(previous: Option<Status>, next: Status) -> StatusChanged {
        StatusChanged {
            previous,
            next,
            first: false,
            since: "09:00".into(),
            message: String::new(),
            end_of_day: None,
            message_sent: false,
            by_activity: false,
        }
    }

    fn project(name: &str, tt_ids: &[&str]) -> Project {
        let mut project = Project::new(name);
        project.timetracker_projects = tt_ids
            .iter()
            .map(|id| LinkedProject {
                id: id.to_string(),
                name: format!("TT {id}"),
            })
            .collect();
        project
    }

    fn last(project_id: &str) -> RecentTimer {
        RecentTimer {
            project_id: project_id.into(),
            project: "SDK v2".into(),
            tag: Some("Development".into()),
            note: "QRS-7064: Contact Form".into(),
        }
    }

    fn start_of(project_id: &str) -> TimerStep {
        TimerStep::Start {
            project_id: project_id.into(),
            note: "QRS-7064: Contact Form".into(),
            tag: Some("Development".into()),
        }
    }

    fn timer_state() -> TimerState {
        TimerState {
            recording_id: "r1".into(),
            project_id: "tt1".into(),
            project: "SDK v2".into(),
            client: None,
            tag_id: None,
            tag: None,
            note: None,
            started_at: 0,
        }
    }

    #[test]
    fn working_after_none_break_or_off_resumes_the_last_timer_of_an_offered_project() {
        let offered = project("Oscorp", &["tt1"]);
        for previous in [None, Some(Status::Break), Some(Status::Off)] {
            for known in [TimerKnown::Idle, TimerKnown::Unknown] {
                assert_eq!(
                    timer_step(
                        &change(previous, Status::Working),
                        known,
                        Some(&last("tt1")),
                        &[&offered],
                        true
                    ),
                    start_of("tt1"),
                    "{previous:?} {known:?}"
                );
            }
        }
    }

    #[test]
    fn a_project_linked_to_several_timetracker_projects_is_offered_by_any_of_them() {
        let offered = project("Oscorp", &["tt0", "tt2"]);
        assert_eq!(
            timer_step(
                &change(Some(Status::Break), Status::Working),
                TimerKnown::Idle,
                Some(&last("tt2")),
                &[&offered],
                true
            ),
            start_of("tt2")
        );
    }

    #[test]
    fn a_resumed_timer_without_a_tag_starts_without_one() {
        let offered = project("Oscorp", &["tt1"]);
        let mut recent = last("tt1");
        recent.tag = None;
        recent.note = String::new();
        assert_eq!(
            timer_step(
                &change(None, Status::Working),
                TimerKnown::Idle,
                Some(&recent),
                &[&offered],
                true
            ),
            TimerStep::Start {
                project_id: "tt1".into(),
                note: String::new(),
                tag: None,
            }
        );
    }

    #[test]
    fn working_asks_when_there_is_no_usable_last_timer_and_the_context_has_status() {
        let offered = project("Oscorp", &["tt1"]);
        let other = project("Other", &["tt9"]);
        let unlinked = project("Solo", &[]);
        let cases: [(Option<RecentTimer>, Vec<&Project>); 4] = [
            (None, vec![&offered]),
            (Some(last("tt9")), vec![&offered]),
            (Some(last("tt9")), vec![&unlinked]),
            (Some(last("tt1")), vec![&other, &unlinked]),
        ];
        for (recent, projects) in cases {
            assert_eq!(
                timer_step(
                    &change(Some(Status::Break), Status::Working),
                    TimerKnown::Idle,
                    recent.as_ref(),
                    &projects,
                    true
                ),
                TimerStep::Ask,
                "{recent:?}"
            );
        }
    }

    #[test]
    fn working_does_nothing_without_a_usable_last_timer_in_a_context_without_status() {
        let offered = project("Oscorp", &["tt1"]);
        for recent in [None, Some(last("tt9"))] {
            assert_eq!(
                timer_step(
                    &change(Some(Status::Off), Status::Working),
                    TimerKnown::Unknown,
                    recent.as_ref(),
                    &[&offered],
                    false
                ),
                TimerStep::Nothing,
                "{recent:?}"
            );
        }
    }

    #[test]
    fn a_context_without_status_still_resumes_a_usable_last_timer() {
        let offered = project("Oscorp", &["tt1"]);
        assert_eq!(
            timer_step(
                &change(None, Status::Working),
                TimerKnown::Idle,
                Some(&last("tt1")),
                &[&offered],
                false
            ),
            start_of("tt1")
        );
    }

    #[test]
    fn working_keeps_a_timer_that_is_already_running() {
        let offered = project("Oscorp", &["tt1"]);
        for previous in [None, Some(Status::Break), Some(Status::Off)] {
            assert_eq!(
                timer_step(
                    &change(previous, Status::Working),
                    TimerKnown::Running,
                    Some(&last("tt1")),
                    &[&offered],
                    true
                ),
                TimerStep::Nothing,
                "{previous:?}"
            );
        }
    }

    #[test]
    fn staying_on_working_does_nothing() {
        let offered = project("Oscorp", &["tt1"]);
        for known in [TimerKnown::Running, TimerKnown::Idle, TimerKnown::Unknown] {
            assert_eq!(
                timer_step(
                    &change(Some(Status::Working), Status::Working),
                    known,
                    Some(&last("tt1")),
                    &[&offered],
                    true
                ),
                TimerStep::Nothing,
                "{known:?}"
            );
        }
    }

    #[test]
    fn a_status_set_by_an_activity_pick_does_nothing() {
        let offered = project("Oscorp", &["tt1"]);
        for (previous, next) in [
            (Some(Status::Break), Status::Working),
            (None, Status::Working),
            (Some(Status::Working), Status::Break),
            (Some(Status::Working), Status::Off),
        ] {
            for known in [TimerKnown::Running, TimerKnown::Idle, TimerKnown::Unknown] {
                let mut picked = change(previous, next);
                picked.by_activity = true;
                assert_eq!(
                    timer_step(&picked, known, Some(&last("tt1")), &[&offered], true),
                    TimerStep::Nothing,
                    "{previous:?} {next:?} {known:?}"
                );
                assert!(!needs_last_timer(&picked, known));
            }
        }
    }

    #[test]
    fn break_and_off_stop_a_running_or_unknown_timer() {
        for next in [Status::Break, Status::Off] {
            for previous in [None, Some(Status::Working), Some(Status::Break)] {
                if previous == Some(next) {
                    continue;
                }
                for known in [TimerKnown::Running, TimerKnown::Unknown] {
                    assert_eq!(
                        timer_step(&change(previous, next), known, None, &[], true),
                        TimerStep::Stop,
                        "{previous:?} {next:?} {known:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn break_and_off_leave_an_idle_timer_alone() {
        for next in [Status::Break, Status::Off] {
            assert_eq!(
                timer_step(
                    &change(Some(Status::Working), next),
                    TimerKnown::Idle,
                    None,
                    &[],
                    true
                ),
                TimerStep::Nothing,
                "{next:?}"
            );
        }
    }

    #[test]
    fn only_a_resume_needs_the_last_timer() {
        for (previous, next, known, expected) in [
            (Some(Status::Break), Status::Working, TimerKnown::Idle, true),
            (None, Status::Working, TimerKnown::Unknown, true),
            (
                Some(Status::Break),
                Status::Working,
                TimerKnown::Running,
                false,
            ),
            (
                Some(Status::Working),
                Status::Working,
                TimerKnown::Idle,
                false,
            ),
            (
                Some(Status::Working),
                Status::Break,
                TimerKnown::Running,
                false,
            ),
            (Some(Status::Working), Status::Off, TimerKnown::Idle, false),
        ] {
            assert_eq!(
                needs_last_timer(&change(previous, next), known),
                expected,
                "{previous:?} {next:?} {known:?}"
            );
        }
    }

    #[test]
    fn the_timer_is_known_from_timetrackers_state_and_its_timer() {
        for (tracking, expected) in [
            (
                Tracking {
                    tt: TtState::Available,
                    timer: Some(timer_state()),
                },
                TimerKnown::Running,
            ),
            (
                Tracking {
                    tt: TtState::Available,
                    timer: None,
                },
                TimerKnown::Idle,
            ),
            (Tracking::gone(TtState::NotRunning), TimerKnown::Unknown),
            (Tracking::gone(TtState::Missing), TimerKnown::Unknown),
            (
                Tracking {
                    tt: TtState::NotRunning,
                    timer: Some(timer_state()),
                },
                TimerKnown::Unknown,
            ),
        ] {
            assert_eq!(TimerKnown::of(&tracking), expected, "{tracking:?}");
        }
    }
}
