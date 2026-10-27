//! What ↵ in the activity overlay does: nothing for a check-in that keeps the running timer, else
//! `timer start` on the picked TimeTracker project with the typed note. Pure.

use serde::Deserialize;
use ts_rs::TS;

use super::session::ActivityMode;
use crate::error::{AppError, AppResult};
use crate::timetracker::reply::TimerState;

pub const NO_PROJECT: &str = "No project to time: link one in Settings → Projects.";

/// What the overlay's ↵ submits.
#[derive(Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ActivityPick {
    pub tt_project_id: String,
    /// As typed; sent verbatim.
    pub note: String,
    /// The picked recent timer's tag, or the session's while its project and note stay.
    pub tag: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    /// "Still on it": the running timer stays as it is.
    Unchanged,
    Start {
        project_id: String,
        note: String,
        tag: Option<String>,
    },
}

/// A check-in confirmed as it opened: the running timer's project and note.
fn keeps_timer(mode: ActivityMode, pick: &ActivityPick, timer: Option<&TimerState>) -> bool {
    mode == ActivityMode::Checkin
        && timer.is_some_and(|timer| {
            timer.project_id == pick.tt_project_id
                && timer.note.as_deref().unwrap_or_default().trim() == pick.note.trim()
        })
}

/// The pick's tag, else the running timer's when the project stays: TimeTracker stops a tagged
/// timer for a `timer start` without one.
fn tag(pick: &ActivityPick, timer: Option<&TimerState>) -> Option<String> {
    pick.tag.clone().or_else(|| {
        timer
            .filter(|timer| timer.project_id == pick.tt_project_id)
            .and_then(|timer| timer.tag.clone())
    })
}

pub fn plan(
    mode: ActivityMode,
    pick: &ActivityPick,
    timer: Option<&TimerState>,
) -> AppResult<Plan> {
    if keeps_timer(mode, pick, timer) {
        return Ok(Plan::Unchanged);
    }
    if pick.tt_project_id.trim().is_empty() {
        return Err(AppError::InvalidInput(NO_PROJECT.into()));
    }
    Ok(Plan::Start {
        project_id: pick.tt_project_id.clone(),
        note: pick.note.clone(),
        tag: tag(pick, timer),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pick(project: &str, note: &str, tag: Option<&str>) -> ActivityPick {
        ActivityPick {
            tt_project_id: project.into(),
            note: note.into(),
            tag: tag.map(str::to_string),
        }
    }

    fn timer(project: &str, note: Option<&str>, tag: Option<&str>) -> TimerState {
        TimerState {
            recording_id: "r1".into(),
            project_id: project.into(),
            project: "TT".into(),
            client: None,
            tag_id: None,
            tag: tag.map(str::to_string),
            note: note.map(str::to_string),
            started_at: 0,
        }
    }

    fn start(project: &str, note: &str, tag: Option<&str>) -> Plan {
        Plan::Start {
            project_id: project.into(),
            note: note.into(),
            tag: tag.map(str::to_string),
        }
    }

    #[test]
    fn a_check_in_confirmed_as_it_opened_sends_nothing() {
        let timer = timer("sdk", Some("QRS-1"), Some("Dev"));
        let plan = plan(
            ActivityMode::Checkin,
            &pick("sdk", "QRS-1", None),
            Some(&timer),
        );
        assert_eq!(plan.unwrap(), Plan::Unchanged);
    }

    #[test]
    fn surrounding_spaces_and_a_missing_note_do_not_count_as_a_change() {
        for (timer_note, typed) in [(Some("QRS-1"), "  QRS-1 "), (None, "   "), (None, "")] {
            let timer = timer("sdk", timer_note, None);
            let plan = plan(
                ActivityMode::Checkin,
                &pick("sdk", typed, None),
                Some(&timer),
            );
            assert_eq!(plan.unwrap(), Plan::Unchanged, "{timer_note:?} {typed:?}");
        }
    }

    #[test]
    fn a_check_in_with_another_note_or_project_starts_a_timer() {
        let timer = timer("sdk", Some("QRS-1"), None);
        for (project, note) in [("sdk", "QRS-2"), ("apollo", "QRS-1")] {
            let plan = plan(
                ActivityMode::Checkin,
                &pick(project, note, None),
                Some(&timer),
            );
            assert_eq!(
                plan.unwrap(),
                start(project, note, None),
                "{project} {note}"
            );
        }
    }

    #[test]
    fn a_check_in_without_a_running_timer_starts_one() {
        let plan = plan(ActivityMode::Checkin, &pick("sdk", "QRS-1", None), None);
        assert_eq!(plan.unwrap(), start("sdk", "QRS-1", None));
    }

    #[test]
    fn a_switch_always_starts_even_when_nothing_changed() {
        let timer = timer("sdk", Some("QRS-1"), None);
        let plan = plan(
            ActivityMode::Switch,
            &pick("sdk", "QRS-1", None),
            Some(&timer),
        );
        assert_eq!(plan.unwrap(), start("sdk", "QRS-1", None));
    }

    #[test]
    fn the_note_is_sent_as_typed() {
        let plan = plan(ActivityMode::Switch, &pick("sdk", "  -x  ", None), None);
        assert_eq!(plan.unwrap(), start("sdk", "  -x  ", None));
    }

    #[test]
    fn the_tag_is_the_picks_else_the_running_timers_on_the_same_project() {
        let timer = timer("sdk", Some("old"), Some("Dev"));
        for (project, tag, expected) in [
            ("sdk", Some("Admin"), Some("Admin")),
            ("sdk", None, Some("Dev")),
            ("apollo", None, None),
            ("apollo", Some("Admin"), Some("Admin")),
        ] {
            let plan = plan(
                ActivityMode::Switch,
                &pick(project, "new", tag),
                Some(&timer),
            );
            assert_eq!(
                plan.unwrap(),
                start(project, "new", expected),
                "{project} {tag:?}"
            );
        }
    }

    #[test]
    fn a_blank_project_is_invalid_input() {
        for project in ["", "  "] {
            let err = plan(ActivityMode::Switch, &pick(project, "x", None), None).unwrap_err();
            assert!(
                matches!(&err, AppError::InvalidInput(message) if message == NO_PROJECT),
                "{project:?}"
            );
        }
    }

    #[test]
    fn a_pick_is_read_from_camel_case_fields() {
        let parsed: ActivityPick =
            serde_json::from_value(json!({ "ttProjectId": "sdk", "note": "x", "tag": "Dev" }))
                .unwrap();
        assert_eq!(parsed, pick("sdk", "x", Some("Dev")));
    }
}
