//! What the activity overlay opens with: the prompt, the projects to pick (TimeTracker's projects
//! grouped under the Daybook projects that count now) and the values filled in. Pure.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
    /// Only the active ones; empty: it can't be picked.
    pub tt_projects: Vec<ActivityTtProject>,
    /// Why it can't be picked.
    pub hint: Option<String>,
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
    /// The TimeTracker project picked first.
    pub tt_project_id: Option<String>,
    /// The note filled in (and selected): the timer's, or the prefill's.
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
    /// The timer as the surfaces show it, e.g. `Oscorp · QRS-6805`.
    pub timer_text: Option<String>,
    pub prefill: Option<&'a Prefill>,
}

/// `Still on Oscorp · QRS-6805?` for a check-in with a timer, else the question.
pub fn prompt(mode: ActivityMode, timer_text: Option<&str>) -> String {
    match (mode, timer_text) {
        (ActivityMode::Checkin, Some(text)) if !text.is_empty() => format!("Still on {text}?"),
        _ => ASK.to_string(),
    }
}

fn tt_projects(project: &Project, listed: Option<&[TtProject]>) -> Vec<ActivityTtProject> {
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

pub fn link_hint(name: &str) -> String {
    format!("Link {name} to a TimeTracker project in Settings → Projects")
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
                hint: tt_projects.is_empty().then(|| link_hint(&project.name)),
                name: project.name.clone(),
                tt_projects,
            }
        })
        .collect()
}

fn holds(project: &ActivityProject, id: Option<&str>) -> bool {
    id.is_some_and(|id| project.tt_projects.iter().any(|tt| tt.id == id))
}

/// The prefill's project, else the timer's, else the first one with TimeTracker projects.
fn first_pick<'a>(
    projects: &'a [ActivityProject],
    prefill: Option<&Prefill>,
    timer: Option<&TimerState>,
) -> Option<&'a ActivityProject> {
    let pickable = |project: &&ActivityProject| !project.tt_projects.is_empty();
    let named = |name: Option<&str>| {
        projects
            .iter()
            .filter(pickable)
            .find(|project| Some(project.name.as_str()) == name)
    };
    let by_prefill = prefill.and_then(|prefill| {
        named(prefill.project.as_deref()).or_else(|| {
            projects
                .iter()
                .find(|project| holds(project, prefill.tt_project_id.as_deref()))
        })
    });
    by_prefill
        .or_else(|| {
            projects
                .iter()
                .find(|project| holds(project, timer.map(|timer| timer.project_id.as_str())))
        })
        .or_else(|| projects.iter().find(pickable))
}

/// Within `project`: the prefill's TimeTracker project, else the timer's, else its first.
fn pick_tt_project(
    project: &ActivityProject,
    prefill: Option<&Prefill>,
    timer: Option<&TimerState>,
) -> Option<String> {
    let prefill_id = prefill.and_then(|prefill| prefill.tt_project_id.as_deref());
    let timer_id = timer.map(|timer| timer.project_id.as_str());
    [prefill_id, timer_id]
        .into_iter()
        .find(|id| holds(project, *id))
        .flatten()
        .or_else(|| project.tt_projects.first().map(|tt| tt.id.as_str()))
        .map(str::to_string)
}

/// The prefill's note and tag (the tag only on its own TimeTracker project), else the timer's note.
fn note_and_tag(
    prefill: Option<&Prefill>,
    timer: Option<&TimerState>,
    tt_project_id: Option<&str>,
) -> (String, Option<String>) {
    let Some(prefill) = prefill else {
        let note = timer
            .and_then(|timer| timer.note.clone())
            .unwrap_or_default();
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
    let picked = first_pick(&projects, input.prefill, input.timer);
    let tt_project_id =
        picked.and_then(|project| pick_tt_project(project, input.prefill, input.timer));
    let (note, tag) = note_and_tag(input.prefill, input.timer, tt_project_id.as_deref());
    ActivitySession {
        mode: input.mode,
        prompt: prompt(input.mode, input.timer_text.as_deref()),
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

    fn session(
        setup: &Setup,
        timer: Option<&TimerState>,
        prefill: Option<&Prefill>,
    ) -> ActivitySession {
        let offered = [&setup.hobby, &setup.oscorp, &setup.internal];
        build_session(&SessionInput {
            mode: ActivityMode::Switch,
            offered: &offered,
            listed: Some(&setup.listed),
            tt: TtState::Available,
            timer,
            timer_text: None,
            prefill,
        })
    }

    #[test]
    fn the_prompt_follows_the_mode_and_the_running_timer() {
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
    fn the_session_prompt_uses_the_timer_text_in_a_check_in() {
        let setup = setup();
        let offered = [&setup.oscorp];
        let built = build_session(&SessionInput {
            mode: ActivityMode::Checkin,
            offered: &offered,
            listed: Some(&setup.listed),
            tt: TtState::Available,
            timer: None,
            timer_text: Some("Oscorp · QRS-6805".into()),
            prefill: None,
        });
        assert_eq!(built.prompt, "Still on Oscorp · QRS-6805?");
        assert_eq!(built.mode, ActivityMode::Checkin);
    }

    #[test]
    fn without_timer_or_prefill_the_first_project_with_tracker_projects_is_picked() {
        let built = session(&setup(), None, None);
        assert_eq!(built.project.as_deref(), Some("Oscorp"));
        assert_eq!(built.tt_project_id.as_deref(), Some("sdk"));
        assert_eq!(built.note, "");
        assert_eq!(built.tag, None);
    }

    #[test]
    fn the_running_timers_project_and_note_are_filled_in() {
        let timer = timer("apollo", Some("QRS-6805"), Some("Dev"));
        let built = session(&setup(), Some(&timer), None);
        assert_eq!(built.project.as_deref(), Some("Oscorp"));
        assert_eq!(built.tt_project_id.as_deref(), Some("apollo"));
        assert_eq!(built.note, "QRS-6805");
        assert_eq!(built.tag, None);
    }

    #[test]
    fn a_timer_without_a_note_fills_in_an_empty_one() {
        let timer = timer("int", None, None);
        let built = session(&setup(), Some(&timer), None);
        assert_eq!(built.project.as_deref(), Some("Internal"));
        assert_eq!(built.note, "");
    }

    #[test]
    fn a_prefill_wins_over_the_running_timer() {
        let timer = timer("int", Some("timer note"), None);
        let prefill = prefill(Some("Oscorp"), "from prefill", None, None);
        let built = session(&setup(), Some(&timer), Some(&prefill));
        assert_eq!(built.project.as_deref(), Some("Oscorp"));
        assert_eq!(built.tt_project_id.as_deref(), Some("sdk"));
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
    fn an_unlinked_prefill_project_falls_back_to_the_timers_project() {
        let timer = timer("int", Some("x"), None);
        let prefill = prefill(Some("Hobby"), "note", None, None);
        let built = session(&setup(), Some(&timer), Some(&prefill));
        assert_eq!(built.project.as_deref(), Some("Internal"));
        assert_eq!(built.note, "note");
    }

    #[test]
    fn no_project_is_picked_when_none_has_tracker_projects() {
        let hobby = project("Hobby", &[]);
        let offered = [&hobby];
        let built = build_session(&SessionInput {
            mode: ActivityMode::Switch,
            offered: &offered,
            listed: None,
            tt: TtState::NotRunning,
            timer: None,
            timer_text: None,
            prefill: None,
        });
        assert_eq!(built.project, None);
        assert_eq!(built.tt_project_id, None);
    }

    #[test]
    fn archived_tracker_projects_are_left_out_and_a_project_without_others_cannot_be_picked() {
        let mut setup = setup();
        setup.listed[0].archived = true;
        setup.listed[1].archived = true;
        let built = session(&setup, None, None);
        let oscorp = &built.projects[1];
        assert_eq!(oscorp.name, "Oscorp");
        assert_eq!(oscorp.tt_projects, vec![]);
        assert_eq!(
            oscorp.hint.as_deref(),
            Some("Link Oscorp to a TimeTracker project in Settings → Projects")
        );
        assert_eq!(built.project.as_deref(), Some("Internal"));
    }

    #[test]
    fn projects_are_listed_in_order_with_the_trackers_name_client_and_colour() {
        let built = session(&setup(), None, None);
        let names: Vec<_> = built.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Hobby", "Oscorp", "Internal"]);
        assert!(built.projects[0].tt_projects.is_empty());
        assert!(built.projects[0].hint.is_some());
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
        assert_eq!(built.projects[1].hint, None);
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
                hint: None,
            }]
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
            timer_text: None,
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
