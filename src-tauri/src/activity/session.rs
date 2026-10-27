//! What the activity overlay opens with: the prompt, the Daybook projects that count now (each with
//! the TimeTracker projects its timer row offers) and the values filled in. Pure.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::current::KeptActivity;
use super::pick::timer_project;
use super::prefill::Prefill;
use crate::store::settings::Project;
use crate::timetracker::TtState;
use crate::timetracker::reply::{TimerState, TtProject};

const ASK: &str = "What are you working on?";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum ActivityMode {
    /// Opened by the owner.
    Switch,
    /// Opened by the clock.
    Checkin,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ActivityTtProject {
    pub id: String,
    pub name: String,
    pub client: Option<String>,
    /// An accent name, e.g. `blue`; empty before TimeTracker listed its projects.
    pub color: String,
    pub shade: u8,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ActivityProject {
    pub name: String,
    /// Its active TimeTracker projects: none, no timer; one, always a timer on it.
    pub tt_projects: Vec<ActivityTtProject>,
    /// More than one: the timer row offers "None" and `tt_projects`.
    pub timer_row: bool,
}

/// The `activity` overlay's session args.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ActivitySession {
    pub mode: ActivityMode,
    pub prompt: String,
    /// The projects that count now, in Settings order.
    pub projects: Vec<ActivityProject>,
    /// The Daybook project picked first.
    pub project: Option<String>,
    /// The TimeTracker project picked first: the only link, else the prefill's or the timer's; `None`:
    /// no link, or several and neither matches.
    pub tt_project_id: Option<String>,
    /// The note filled in (and selected): the prefill's, the timer's, or the kept activity's.
    pub note: String,
    /// The prefill's tag; send it back while its project and note stay.
    pub tag: Option<String>,
    pub tt: TtState,
}

pub struct SessionInput<'a> {
    pub mode: ActivityMode,
    pub offered: &'a [&'a Project],
    /// TimeTracker's projects as listed last.
    pub listed: Option<&'a [TtProject]>,
    pub tt: TtState,
    pub timer: Option<&'a TimerState>,
    /// The activity kept without a timer (or with one, then the timer counts).
    pub kept: Option<&'a KeptActivity>,
    /// The current activity as the surfaces show it, e.g. `Oscorp · QRS-6805`.
    pub current_text: Option<String>,
    pub prefill: Option<&'a Prefill>,
}

/// `Still on Oscorp · QRS-6805?` for a check-in with a current activity, else the question.
pub fn prompt(mode: ActivityMode, current_text: Option<&str>) -> String {
    match (mode, current_text) {
        (ActivityMode::Checkin, Some(text)) if !text.is_empty() => format!("Still on {text}?"),
        _ => ASK.to_string(),
    }
}

/// The active TimeTracker projects linked to `project`, named as TimeTracker lists them.
pub fn tt_projects(project: &Project, listed: Option<&[TtProject]>) -> Vec<ActivityTtProject> {
    project
        .timetracker_projects
        .iter()
        .filter_map(|linked| {
            let found = listed.and_then(|listed| listed.iter().find(|tt| tt.id == linked.id));
            match found {
                Some(found) if found.archived => None,
                Some(found) => Some(ActivityTtProject {
                    id: found.id.clone(),
                    name: found.name.clone(),
                    client: found.client.clone(),
                    color: found.color.clone(),
                    shade: found.shade,
                }),
                None => Some(ActivityTtProject {
                    id: linked.id.clone(),
                    name: linked.name.clone(),
                    client: None,
                    color: String::new(),
                    shade: 0,
                }),
            }
        })
        .collect()
}

pub fn activity_projects(
    offered: &[&Project],
    listed: Option<&[TtProject]>,
) -> Vec<ActivityProject> {
    offered
        .iter()
        .map(|project| {
            let tt_projects = tt_projects(project, listed);
            ActivityProject {
                name: project.name.clone(),
                timer_row: tt_projects.len() > 1,
                tt_projects,
            }
        })
        .collect()
}

fn link_ids(project: &ActivityProject) -> Vec<String> {
    project.tt_projects.iter().map(|tt| tt.id.clone()).collect()
}

fn holds(project: &ActivityProject, id: Option<&str>) -> bool {
    id.is_some_and(|id| project.tt_projects.iter().any(|tt| tt.id == id))
}

/// The prefill's project (by name, else by its TimeTracker project), else the timer's, else the kept
/// activity's, else the first.
fn first_pick<'a>(
    projects: &'a [ActivityProject],
    input: &SessionInput,
) -> Option<&'a ActivityProject> {
    let named = |name: Option<&str>| {
        projects
            .iter()
            .find(|project| name.is_some_and(|name| project.name == name))
    };
    let holding = |id: Option<&str>| projects.iter().find(|project| holds(project, id));
    let by_prefill = input.prefill.and_then(|prefill| {
        named(prefill.project.as_deref()).or_else(|| holding(prefill.tt_project_id.as_deref()))
    });
    by_prefill
        .or_else(|| holding(input.timer.map(|timer| timer.project_id.as_str())))
        .or_else(|| named(input.kept.map(|kept| kept.project.as_str())))
        .or_else(|| projects.first())
}

/// Within `project`: the prefill's TimeTracker project, else the timer's; the rules of
/// `timer_project` decide (one link: that one; several: the wish or none).
fn pick_tt_project(project: &ActivityProject, input: &SessionInput) -> Option<String> {
    let prefill_id = input
        .prefill
        .and_then(|prefill| prefill.tt_project_id.as_deref());
    let timer_id = input.timer.map(|timer| timer.project_id.as_str());
    let wish = [prefill_id, timer_id]
        .into_iter()
        .flatten()
        .find(|id| holds(project, Some(*id)));
    timer_project(&link_ids(project), wish)
}

/// The prefill's note and tag (the tag only on its own TimeTracker project), else the timer's note,
/// else the kept activity's.
fn note_and_tag(input: &SessionInput, tt_project_id: Option<&str>) -> (String, Option<String>) {
    let Some(prefill) = input.prefill else {
        let note = match input.timer {
            Some(timer) => timer.note.clone().unwrap_or_default(),
            None => input.kept.map(|kept| kept.note.clone()).unwrap_or_default(),
        };
        return (note, None);
    };
    let tag_kept =
        prefill.tt_project_id.is_some() && tt_project_id == prefill.tt_project_id.as_deref();
    (
        prefill.note.clone(),
        prefill.tag.clone().filter(|_| tag_kept),
    )
}

pub fn build_session(input: &SessionInput) -> ActivitySession {
    let projects = activity_projects(input.offered, input.listed);
    let picked = first_pick(&projects, input);
    let tt_project_id = picked.and_then(|project| pick_tt_project(project, input));
    let (note, tag) = note_and_tag(input, tt_project_id.as_deref());
    ActivitySession {
        mode: input.mode,
        prompt: prompt(input.mode, input.current_text.as_deref()),
        project: picked.map(|project| project.name.clone()),
        projects,
        tt_project_id,
        note,
        tag,
        tt: input.tt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::LinkedProject;
    use serde_json::json;

    fn project(name: &str, tt: &[(&str, &str)]) -> Project {
        let mut project = Project::new(name);
        project.timetracker_projects = tt
            .iter()
            .map(|(id, name)| LinkedProject {
                id: (*id).into(),
                name: (*name).into(),
            })
            .collect();
        project
    }

    fn listed(id: &str, name: &str, archived: bool) -> TtProject {
        TtProject {
            id: id.into(),
            name: name.into(),
            client_id: None,
            client: Some("Oscorp".into()),
            color: "blue".into(),
            shade: 6,
            billable: true,
            archived,
        }
    }

    fn timer(project_id: &str, note: Option<&str>, tag: Option<&str>) -> TimerState {
        TimerState {
            recording_id: "r1".into(),
            project_id: project_id.into(),
            project: "TT".into(),
            client: None,
            tag_id: None,
            tag: tag.map(str::to_string),
            note: note.map(str::to_string),
            started_at: 0,
        }
    }

    fn kept(project: &str, note: &str) -> KeptActivity {
        KeptActivity {
            project: project.into(),
            note: note.into(),
        }
    }

    fn prefill(project: Option<&str>, note: &str, tt: Option<&str>, tag: Option<&str>) -> Prefill {
        Prefill {
            project: project.map(str::to_string),
            note: note.into(),
            tt_project_id: tt.map(str::to_string),
            tag: tag.map(str::to_string),
        }
    }

    struct Setup {
        hobby: Project,
        oscorp: Project,
        internal: Project,
        listed: Vec<TtProject>,
    }

    /// Hobby links none, Oscorp two, Internal one.
    fn setup() -> Setup {
        Setup {
            hobby: project("Hobby", &[]),
            oscorp: project("Oscorp", &[("sdk", "SDK v2"), ("apollo", "Apollo")]),
            internal: project("Internal", &[("int", "Internal")]),
            listed: vec![
                listed("sdk", "SDK v2", false),
                listed("apollo", "Apollo", false),
                listed("int", "Internal", false),
            ],
        }
    }

    fn session_in(
        setup: &Setup,
        mode: ActivityMode,
        timer: Option<&TimerState>,
        kept: Option<&KeptActivity>,
        prefill: Option<&Prefill>,
    ) -> ActivitySession {
        let offered = [&setup.hobby, &setup.oscorp, &setup.internal];
        build_session(&SessionInput {
            mode,
            offered: &offered,
            listed: Some(&setup.listed),
            tt: TtState::Available,
            timer,
            kept,
            current_text: None,
            prefill,
        })
    }

    fn session(
        setup: &Setup,
        timer: Option<&TimerState>,
        prefill: Option<&Prefill>,
    ) -> ActivitySession {
        session_in(setup, ActivityMode::Switch, timer, None, prefill)
    }

    #[test]
    fn the_prompt_follows_the_mode_and_the_current_activity() {
        for (mode, text, expected) in [
            (ActivityMode::Switch, None, "What are you working on?"),
            (
                ActivityMode::Switch,
                Some("Oscorp · QRS-6805"),
                "What are you working on?",
            ),
            (ActivityMode::Checkin, None, "What are you working on?"),
            (ActivityMode::Checkin, Some(""), "What are you working on?"),
            (
                ActivityMode::Checkin,
                Some("Oscorp · QRS-6805"),
                "Still on Oscorp · QRS-6805?",
            ),
        ] {
            assert_eq!(prompt(mode, text), expected, "{mode:?} {text:?}");
        }
    }

    #[test]
    fn a_check_in_asks_about_an_activity_that_has_no_timer() {
        let setup = setup();
        let offered = [&setup.oscorp];
        let built = build_session(&SessionInput {
            mode: ActivityMode::Checkin,
            offered: &offered,
            listed: Some(&setup.listed),
            tt: TtState::Available,
            timer: None,
            kept: Some(&kept("Oscorp", "QRS-6805")),
            current_text: Some("Oscorp · QRS-6805".into()),
            prefill: None,
        });
        assert_eq!(built.prompt, "Still on Oscorp · QRS-6805?");
        assert_eq!(built.mode, ActivityMode::Checkin);
        assert_eq!(built.project.as_deref(), Some("Oscorp"));
        assert_eq!(built.note, "QRS-6805");
    }

    #[test]
    fn without_timer_prefill_or_kept_activity_the_first_project_is_picked_without_a_timer() {
        let built = session(&setup(), None, None);
        assert_eq!(built.project.as_deref(), Some("Hobby"));
        assert_eq!(built.tt_project_id, None);
        assert_eq!(built.note, "");
        assert_eq!(built.tag, None);
    }

    #[test]
    fn a_first_project_with_one_link_starts_on_that_link() {
        let setup = Setup {
            hobby: project("Internal", &[("int", "Internal")]),
            ..setup()
        };
        let built = session(&setup, None, None);
        assert_eq!(built.project.as_deref(), Some("Internal"));
        assert_eq!(built.tt_project_id.as_deref(), Some("int"));
    }

    #[test]
    fn a_first_project_with_several_links_starts_on_none() {
        let setup = Setup {
            hobby: project("Oscorp", &[("sdk", "SDK v2"), ("apollo", "Apollo")]),
            ..setup()
        };
        let built = session(&setup, None, None);
        assert_eq!(built.project.as_deref(), Some("Oscorp"));
        assert_eq!(built.tt_project_id, None);
    }

    #[test]
    fn the_running_timers_project_and_note_are_filled_in() {
        let running = timer("apollo", Some("QRS-6805"), Some("Dev"));
        let built = session(&setup(), Some(&running), None);
        assert_eq!(built.project.as_deref(), Some("Oscorp"));
        assert_eq!(built.tt_project_id.as_deref(), Some("apollo"));
        assert_eq!(built.note, "QRS-6805");
        assert_eq!(built.tag, None);
    }

    #[test]
    fn a_timer_without_a_note_fills_in_an_empty_one() {
        let running = timer("int", None, None);
        let kept = kept("Internal", "kept note");
        let built = session_in(
            &setup(),
            ActivityMode::Switch,
            Some(&running),
            Some(&kept),
            None,
        );
        assert_eq!(built.project.as_deref(), Some("Internal"));
        assert_eq!(built.note, "");
    }

    #[test]
    fn a_timer_wins_over_the_kept_activity() {
        let running = timer("int", Some("timer note"), None);
        let kept = kept("Hobby", "kept note");
        let built = session_in(
            &setup(),
            ActivityMode::Switch,
            Some(&running),
            Some(&kept),
            None,
        );
        assert_eq!(built.project.as_deref(), Some("Internal"));
        assert_eq!(built.tt_project_id.as_deref(), Some("int"));
        assert_eq!(built.note, "timer note");
    }

    #[test]
    fn a_timer_on_an_unknown_project_leaves_the_kept_activity_in_place() {
        let running = timer("gone", Some("timer note"), None);
        let kept = kept("Internal", "kept note");
        let built = session_in(
            &setup(),
            ActivityMode::Switch,
            Some(&running),
            Some(&kept),
            None,
        );
        assert_eq!(built.project.as_deref(), Some("Internal"));
        assert_eq!(built.note, "timer note");
    }

    #[test]
    fn the_kept_activity_fills_in_its_project_and_note_without_a_timer() {
        for (project, note, expected_tt) in [
            ("Hobby", "weeding", None),
            ("Oscorp", "QRS-1", None),
            ("Internal", "planning", Some("int")),
        ] {
            let kept = kept(project, note);
            let built = session_in(&setup(), ActivityMode::Switch, None, Some(&kept), None);
            assert_eq!(built.project.as_deref(), Some(project));
            assert_eq!(built.tt_project_id.as_deref(), expected_tt, "{project}");
            assert_eq!(built.note, note, "{project}");
            assert_eq!(built.tag, None, "{project}");
        }
    }

    #[test]
    fn a_kept_activity_on_a_project_that_no_longer_counts_falls_back_to_the_first_project() {
        let kept = kept("Gone", "old note");
        let built = session_in(&setup(), ActivityMode::Switch, None, Some(&kept), None);
        assert_eq!(built.project.as_deref(), Some("Hobby"));
        assert_eq!(built.note, "old note");
    }

    #[test]
    fn a_prefill_wins_over_the_running_timer_and_the_kept_activity() {
        let running = timer("int", Some("timer note"), None);
        let kept = kept("Hobby", "kept note");
        let prefill = prefill(Some("Oscorp"), "from prefill", None, None);
        let built = session_in(
            &setup(),
            ActivityMode::Switch,
            Some(&running),
            Some(&kept),
            Some(&prefill),
        );
        assert_eq!(built.project.as_deref(), Some("Oscorp"));
        assert_eq!(built.tt_project_id, None);
        assert_eq!(built.note, "from prefill");
    }

    #[test]
    fn a_prefill_with_a_tracker_project_keeps_its_project_and_tag() {
        let prefill = prefill(Some("Oscorp"), "review", Some("apollo"), Some("Dev"));
        let built = session(&setup(), None, Some(&prefill));
        assert_eq!(built.tt_project_id.as_deref(), Some("apollo"));
        assert_eq!(built.tag.as_deref(), Some("Dev"));
    }

    #[test]
    fn a_prefill_finds_its_project_by_the_tracker_project_when_the_name_is_unknown() {
        let prefill = prefill(Some("Gone"), "review", Some("int"), Some("Dev"));
        let built = session(&setup(), None, Some(&prefill));
        assert_eq!(built.project.as_deref(), Some("Internal"));
        assert_eq!(built.tt_project_id.as_deref(), Some("int"));
        assert_eq!(built.tag.as_deref(), Some("Dev"));
    }

    #[test]
    fn a_prefill_tag_is_dropped_when_the_tracker_project_is_not_the_prefills_own() {
        for (project, tt, label) in [
            (Some("Oscorp"), None, "no tracker project"),
            (
                Some("Oscorp"),
                Some("int"),
                "a project of another Daybook project",
            ),
        ] {
            let prefill = prefill(project, "review", tt, Some("Dev"));
            let built = session(&setup(), None, Some(&prefill));
            assert_eq!(built.tag, None, "{label}");
        }
    }

    #[test]
    fn an_unlinked_prefill_project_is_picked_without_a_timer_project() {
        let running = timer("int", Some("x"), None);
        let prefill = prefill(Some("Hobby"), "note", None, None);
        let built = session(&setup(), Some(&running), Some(&prefill));
        assert_eq!(built.project.as_deref(), Some("Hobby"));
        assert_eq!(built.tt_project_id, None);
        assert_eq!(built.note, "note");
    }

    #[test]
    fn nothing_is_picked_when_no_project_counts() {
        let built = build_session(&SessionInput {
            mode: ActivityMode::Switch,
            offered: &[],
            listed: None,
            tt: TtState::NotRunning,
            timer: None,
            kept: None,
            current_text: None,
            prefill: None,
        });
        assert_eq!(built.project, None);
        assert_eq!(built.tt_project_id, None);
        assert_eq!(built.tt, TtState::NotRunning);
    }

    #[test]
    fn the_timer_row_is_offered_only_for_projects_linking_several() {
        let built = session(&setup(), None, None);
        let rows: Vec<_> = built
            .projects
            .iter()
            .map(|project| {
                (
                    project.name.as_str(),
                    project.tt_projects.len(),
                    project.timer_row,
                )
            })
            .collect();
        assert_eq!(
            rows,
            [
                ("Hobby", 0, false),
                ("Oscorp", 2, true),
                ("Internal", 1, false)
            ]
        );
    }

    #[test]
    fn archived_tracker_projects_are_left_out_of_the_links() {
        let mut setup = setup();
        setup.listed[0].archived = true;
        let built = session(&setup, None, None);
        let oscorp = &built.projects[1];
        let ids: Vec<_> = oscorp.tt_projects.iter().map(|tt| tt.id.as_str()).collect();
        assert_eq!(ids, ["apollo"]);
        assert!(!oscorp.timer_row);

        setup.listed[1].archived = true;
        let built = session(&setup, None, None);
        assert_eq!(built.projects[1].tt_projects, vec![]);
        assert!(!built.projects[1].timer_row);
    }

    #[test]
    fn a_project_left_with_one_active_link_picks_it_for_a_prefill() {
        let mut setup = setup();
        setup.listed[0].archived = true;
        let prefill = prefill(Some("Oscorp"), "review", None, None);
        let built = session(&setup, None, Some(&prefill));
        assert_eq!(built.project.as_deref(), Some("Oscorp"));
        assert_eq!(built.tt_project_id.as_deref(), Some("apollo"));
    }

    #[test]
    fn projects_are_listed_in_order_with_the_trackers_name_client_and_colour() {
        let built = session(&setup(), None, None);
        let names: Vec<_> = built.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Hobby", "Oscorp", "Internal"]);
        assert!(built.projects[0].tt_projects.is_empty());
        assert_eq!(
            built.projects[1].tt_projects[0],
            ActivityTtProject {
                id: "sdk".into(),
                name: "SDK v2".into(),
                client: Some("Oscorp".into()),
                color: "blue".into(),
                shade: 6,
            }
        );
    }

    #[test]
    fn before_the_tracker_listed_its_projects_the_linked_name_shows_with_an_empty_colour() {
        let oscorp = project("Oscorp", &[("sdk", "SDK v2")]);
        let projects = activity_projects(&[&oscorp], None);
        assert_eq!(
            projects,
            vec![ActivityProject {
                name: "Oscorp".into(),
                tt_projects: vec![ActivityTtProject {
                    id: "sdk".into(),
                    name: "SDK v2".into(),
                    client: None,
                    color: String::new(),
                    shade: 0,
                }],
                timer_row: false,
            }]
        );
    }

    #[test]
    fn a_project_serializes_with_camel_case_fields() {
        let oscorp = project("Oscorp", &[("sdk", "SDK v2"), ("apollo", "Apollo")]);
        let projects = activity_projects(&[&oscorp], None);
        assert_eq!(
            serde_json::to_value(&projects).unwrap(),
            json!([{
                "name": "Oscorp",
                "ttProjects": [
                    { "id": "sdk", "name": "SDK v2", "client": null, "color": "", "shade": 0 },
                    { "id": "apollo", "name": "Apollo", "client": null, "color": "", "shade": 0 },
                ],
                "timerRow": true,
            }])
        );
    }

    #[test]
    fn the_session_serializes_with_camel_case_fields() {
        let built = build_session(&SessionInput {
            mode: ActivityMode::Checkin,
            offered: &[],
            listed: None,
            tt: TtState::Available,
            timer: None,
            kept: None,
            current_text: None,
            prefill: None,
        });
        assert_eq!(
            serde_json::to_value(&built).unwrap(),
            json!({
                "mode": "checkin",
                "prompt": "What are you working on?",
                "projects": [],
                "project": null,
                "ttProjectId": null,
                "note": "",
                "tag": null,
                "tt": "available",
            })
        );
    }
}
