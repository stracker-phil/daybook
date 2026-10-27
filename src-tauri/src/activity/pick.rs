//! What ↵ in the activity overlay does with TimeTracker: the picked Daybook project's links decide the
//! timer (none: no timer; one: always that one; several: the picked one or none). Nothing for a
//! check-in that keeps what runs. Pure.

use serde::Deserialize;
use ts_rs::TS;

use super::current::KeptActivity;
use super::session::ActivityMode;
use crate::timetracker::reply::TimerState;

/// What the overlay's ↵ submits.
#[derive(Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ActivityPick {
    /// The Daybook project; empty: none.
    pub project: String,
    /// The timer row's choice as made; `None`: the row is hidden or "None" is picked. `timer_project`
    /// resolves the timer.
    pub tt_project_id: Option<String>,
    /// As typed; sent verbatim.
    pub note: String,
    /// The picked recent timer's tag, or the session's while its project and note stay.
    pub tag: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    /// "Still on it": TimeTracker stays as it is.
    Unchanged,
    Start {
        project_id: String,
        note: String,
        tag: Option<String>,
    },
    /// The activity has no timer: a running one stops.
    NoTimer,
}

/// The timer's TimeTracker project for a pick on a project linking `links` (its active ones).
pub fn timer_project(links: &[String], picked: Option<&str>) -> Option<String> {
    match links {
        [] => None,
        [only] => Some(only.clone()),
        _ => picked
            .filter(|id| links.iter().any(|link| link.as_str() == *id))
            .map(str::to_string),
    }
}

fn same_note(timer: &TimerState, note: &str) -> bool {
    timer.note.as_deref().unwrap_or_default().trim() == note.trim()
}

/// The pick's tag, else the running timer's when the project stays: TimeTracker stops a tagged
/// timer for a `timer start` without one.
fn tag(pick: &ActivityPick, project_id: &str, timer: Option<&TimerState>) -> Option<String> {
    pick.tag.clone().or_else(|| {
        timer
            .filter(|timer| timer.project_id == project_id)
            .and_then(|timer| timer.tag.clone())
    })
}

/// A check-in confirmed as it opened keeps whatever runs, even a timer no Daybook project links.
fn confirms(
    pick: &ActivityPick,
    current: Option<&KeptActivity>,
    links: &[String],
    timer: Option<&TimerState>,
) -> bool {
    let same_activity = current
        .is_some_and(|kept| kept.project == pick.project && kept.note.trim() == pick.note.trim());
    let same_timer = links.len() <= 1
        || pick.tt_project_id.as_deref() == timer.map(|timer| timer.project_id.as_str());
    same_activity && same_timer
}

/// A check-in without a timer to stop changes nothing.
fn without_timer(check_in: bool, timer: Option<&TimerState>) -> Plan {
    if check_in && timer.is_none() {
        Plan::Unchanged
    } else {
        Plan::NoTimer
    }
}

pub fn plan(
    mode: ActivityMode,
    pick: &ActivityPick,
    current: Option<&KeptActivity>,
    links: &[String],
    timer: Option<&TimerState>,
) -> Plan {
    let check_in = mode == ActivityMode::Checkin;
    if check_in && confirms(pick, current, links, timer) {
        return Plan::Unchanged;
    }
    let Some(project_id) = timer_project(links, pick.tt_project_id.as_deref()) else {
        return without_timer(check_in, timer);
    };
    let keeps_timer =
        timer.is_some_and(|timer| timer.project_id == project_id && same_note(timer, &pick.note));
    if check_in && keeps_timer {
        return Plan::Unchanged;
    }
    Plan::Start {
        tag: tag(pick, &project_id, timer),
        project_id,
        note: pick.note.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pick(tt_project: Option<&str>, note: &str, tag: Option<&str>) -> ActivityPick {
        ActivityPick {
            project: "Oscorp".into(),
            tt_project_id: tt_project.map(str::to_string),
            note: note.into(),
            tag: tag.map(str::to_string),
        }
    }

    fn ids(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
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

    fn kept(note: &str) -> KeptActivity {
        KeptActivity {
            project: "Oscorp".into(),
            note: note.into(),
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
    fn the_timer_project_follows_the_number_of_links() {
        for (links, picked, expected) in [
            (&[][..], None, None),
            (&[][..], Some("sdk"), None),
            (&["sdk"][..], None, Some("sdk")),
            (&["sdk"][..], Some("other"), Some("sdk")),
            (&["sdk", "apollo"][..], None, None),
            (&["sdk", "apollo"][..], Some("apollo"), Some("apollo")),
            (&["sdk", "apollo"][..], Some("elsewhere"), None),
        ] {
            assert_eq!(
                timer_project(&ids(links), picked),
                expected.map(str::to_string),
                "{links:?} {picked:?}"
            );
        }
    }

    #[test]
    fn without_a_timer_project_a_switch_stops_a_running_timer_and_a_check_in_keeps_a_stopped_one() {
        let running = timer("sdk", Some("QRS-1"), None);
        let cases = [
            (ids(&[]), None),
            (ids(&["sdk", "apollo"]), None),
            (ids(&["sdk", "apollo"]), Some("elsewhere")),
        ];
        for (links, picked) in cases {
            let pick = pick(picked, "QRS-1", None);
            for (mode, timer, expected) in [
                (ActivityMode::Switch, None, Plan::NoTimer),
                (ActivityMode::Switch, Some(&running), Plan::NoTimer),
                (ActivityMode::Checkin, None, Plan::Unchanged),
                (ActivityMode::Checkin, Some(&running), Plan::NoTimer),
            ] {
                assert_eq!(
                    plan(mode, &pick, None, &links, timer),
                    expected,
                    "{links:?} {picked:?} {mode:?} {}",
                    timer.is_some()
                );
            }
        }
    }

    #[test]
    fn a_single_link_always_starts_its_timer_whatever_was_picked() {
        let links = ids(&["sdk"]);
        for picked in [None, Some("sdk"), Some("other")] {
            assert_eq!(
                plan(
                    ActivityMode::Switch,
                    &pick(picked, "QRS-1", None),
                    None,
                    &links,
                    None
                ),
                start("sdk", "QRS-1", None),
                "{picked:?}"
            );
        }
    }

    #[test]
    fn several_links_start_the_picked_one() {
        let links = ids(&["sdk", "apollo"]);
        assert_eq!(
            plan(
                ActivityMode::Switch,
                &pick(Some("apollo"), "QRS-1", None),
                None,
                &links,
                None
            ),
            start("apollo", "QRS-1", None)
        );
    }

    #[test]
    fn a_check_in_confirmed_as_it_opened_sends_nothing() {
        let running = timer("sdk", Some("QRS-1"), Some("Dev"));
        let plan = plan(
            ActivityMode::Checkin,
            &pick(None, "QRS-1", None),
            Some(&kept("QRS-1")),
            &ids(&["sdk"]),
            Some(&running),
        );
        assert_eq!(plan, Plan::Unchanged);
    }

    #[test]
    fn a_confirmed_check_in_keeps_a_timer_on_a_project_no_daybook_project_links() {
        let running = timer("elsewhere", Some("QRS-1"), None);
        let plan = plan(
            ActivityMode::Checkin,
            &pick(None, " QRS-1 ", None),
            Some(&kept("QRS-1")),
            &ids(&[]),
            Some(&running),
        );
        assert_eq!(plan, Plan::Unchanged);
    }

    #[test]
    fn a_confirmed_check_in_with_several_links_starts_the_picked_one_when_it_is_not_the_running_one()
     {
        let running = timer("sdk", Some("QRS-1"), None);
        let plan = plan(
            ActivityMode::Checkin,
            &pick(Some("apollo"), "QRS-1", None),
            Some(&kept("QRS-1")),
            &ids(&["sdk", "apollo"]),
            Some(&running),
        );
        assert_eq!(plan, start("apollo", "QRS-1", None));
    }

    #[test]
    fn a_confirmed_check_in_with_several_links_keeps_the_running_picked_one() {
        let running = timer("sdk", Some("QRS-1"), None);
        let plan = plan(
            ActivityMode::Checkin,
            &pick(Some("sdk"), "QRS-1", None),
            Some(&kept("QRS-1")),
            &ids(&["sdk", "apollo"]),
            Some(&running),
        );
        assert_eq!(plan, Plan::Unchanged);
    }

    #[test]
    fn a_check_in_with_a_changed_note_is_not_confirmed() {
        let running = timer("elsewhere", Some("QRS-1"), None);
        for (links, expected) in [
            (ids(&[]), Plan::NoTimer),
            (ids(&["sdk"]), start("sdk", "QRS-2", None)),
        ] {
            let plan = plan(
                ActivityMode::Checkin,
                &pick(None, "QRS-2", None),
                Some(&kept("QRS-1")),
                &links,
                Some(&running),
            );
            assert_eq!(plan, expected, "{links:?}");
        }
    }

    #[test]
    fn a_switch_to_the_kept_activity_follows_the_normal_rules() {
        let running = timer("elsewhere", Some("QRS-1"), None);
        for (links, expected) in [
            (ids(&[]), Plan::NoTimer),
            (ids(&["sdk"]), start("sdk", "QRS-1", None)),
        ] {
            let plan = plan(
                ActivityMode::Switch,
                &pick(None, "QRS-1", None),
                Some(&kept("QRS-1")),
                &links,
                Some(&running),
            );
            assert_eq!(plan, expected, "{links:?}");
        }
    }

    #[test]
    fn surrounding_spaces_and_a_missing_note_do_not_count_as_a_change() {
        for (timer_note, typed) in [(Some("QRS-1"), "  QRS-1 "), (None, "   "), (None, "")] {
            let running = timer("sdk", timer_note, None);
            let plan = plan(
                ActivityMode::Checkin,
                &pick(None, typed, None),
                None,
                &ids(&["sdk"]),
                Some(&running),
            );
            assert_eq!(plan, Plan::Unchanged, "{timer_note:?} {typed:?}");
        }
    }

    #[test]
    fn a_check_in_with_another_note_or_project_starts_a_timer() {
        let running = timer("sdk", Some("QRS-1"), None);
        for (project, note) in [("sdk", "QRS-2"), ("apollo", "QRS-1")] {
            let plan = plan(
                ActivityMode::Checkin,
                &pick(None, note, None),
                None,
                &ids(&[project]),
                Some(&running),
            );
            assert_eq!(plan, start(project, note, None), "{project} {note}");
        }
    }

    #[test]
    fn a_check_in_without_a_running_timer_starts_one_on_a_linked_project() {
        let plan = plan(
            ActivityMode::Checkin,
            &pick(None, "QRS-1", None),
            None,
            &ids(&["sdk"]),
            None,
        );
        assert_eq!(plan, start("sdk", "QRS-1", None));
    }

    #[test]
    fn a_switch_always_starts_even_when_nothing_changed() {
        let running = timer("sdk", Some("QRS-1"), None);
        let plan = plan(
            ActivityMode::Switch,
            &pick(None, "QRS-1", None),
            Some(&kept("QRS-1")),
            &ids(&["sdk"]),
            Some(&running),
        );
        assert_eq!(plan, start("sdk", "QRS-1", None));
    }

    #[test]
    fn the_note_is_sent_as_typed() {
        let plan = plan(
            ActivityMode::Switch,
            &pick(None, "  -x  ", None),
            None,
            &ids(&["sdk"]),
            None,
        );
        assert_eq!(plan, start("sdk", "  -x  ", None));
    }

    #[test]
    fn the_tag_is_the_picks_else_the_running_timers_on_the_same_project() {
        let running = timer("sdk", Some("old"), Some("Dev"));
        for (project, tag, expected) in [
            ("sdk", Some("Admin"), Some("Admin")),
            ("sdk", None, Some("Dev")),
            ("apollo", None, None),
            ("apollo", Some("Admin"), Some("Admin")),
        ] {
            let plan = plan(
                ActivityMode::Switch,
                &pick(None, "new", tag),
                None,
                &ids(&[project]),
                Some(&running),
            );
            assert_eq!(plan, start(project, "new", expected), "{project} {tag:?}");
        }
    }

    #[test]
    fn a_pick_is_read_from_camel_case_fields() {
        let parsed: ActivityPick = serde_json::from_value(json!({
            "project": "Oscorp",
            "ttProjectId": "sdk",
            "note": "x",
            "tag": "Dev",
        }))
        .unwrap();
        assert_eq!(parsed, pick(Some("sdk"), "x", Some("Dev")));
    }

    #[test]
    fn a_pick_without_a_timer_project_or_tag_reads_them_as_none() {
        let parsed: ActivityPick =
            serde_json::from_value(json!({ "project": "Hobby", "note": "" })).unwrap();
        assert_eq!(
            parsed,
            ActivityPick {
                project: "Hobby".into(),
                tt_project_id: None,
                note: String::new(),
                tag: None,
            }
        );
    }
}
