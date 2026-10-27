//! The current activity: the Daybook project and note picked last, with or without a timer. Kept in
//! `activity.json` so a restart keeps it; it ends with the day. While a timer runs, the timer is what
//! the surfaces show, and a timer changed in TimeTracker moves the activity along.

use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::store::json_store::JsonStore;
use crate::store::settings::Project;
use crate::timetracker::link::{self, linked_project};
use crate::timetracker::reply::TimerState;
use crate::tools::TimerView;

pub const FILE_NAME: &str = "activity.json";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct KeptActivity {
    /// The Daybook project; TimeTracker's project for a timer on an unlinked one.
    pub project: String,
    pub note: String,
}

impl From<&KeptActivity> for TimerView {
    fn from(activity: &KeptActivity) -> Self {
        TimerView {
            project: activity.project.clone(),
            note: activity.note.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
struct Stored {
    day: Option<NaiveDate>,
    activity: Option<KeptActivity>,
}

pub struct CurrentActivity {
    store: JsonStore<Stored>,
}

impl CurrentActivity {
    pub fn open(config_dir: &Path) -> Self {
        Self {
            store: JsonStore::open(config_dir.join(FILE_NAME)),
        }
    }

    /// Today's activity; one kept on an earlier day is over.
    pub fn get(&self, today: NaiveDate) -> Option<KeptActivity> {
        self.store.read(|stored| {
            stored
                .activity
                .as_ref()
                .filter(|_| stored.day == Some(today))
                .cloned()
        })
    }

    /// Keeps `activity` for today; whether it changed.
    pub fn set(&self, today: NaiveDate, activity: Option<KeptActivity>) -> AppResult<bool> {
        if self.get(today) == activity {
            return Ok(false);
        }
        self.store.update(|stored| {
            stored.day = Some(today);
            stored.activity = activity;
        })?;
        Ok(true)
    }
}

fn links_timetracker_project(project: &Project, tt_project_id: &str) -> bool {
    project
        .timetracker_projects
        .iter()
        .any(|linked| linked.id == tt_project_id)
}

/// The activity after TimeTracker's timer changed: a running timer sets it (its note, and the kept
/// project while that links the timer's project, else the first project linking it, else
/// TimeTracker's name); without a timer it stays.
pub fn follow_timer(
    kept: Option<&KeptActivity>,
    timer: Option<&TimerState>,
    projects: &[Project],
) -> Option<KeptActivity> {
    let Some(timer) = timer else {
        return kept.cloned();
    };
    let kept_project = kept.and_then(|activity| {
        projects.iter().find(|project| {
            project.name == activity.project
                && links_timetracker_project(project, &timer.project_id)
        })
    });
    let project = kept_project
        .or_else(|| linked_project(projects, &timer.project_id))
        .map_or_else(|| timer.project.clone(), |project| project.name.clone());
    Some(KeptActivity {
        project,
        note: timer.note.clone().unwrap_or_default(),
    })
}

/// The current activity as the surfaces show it: the running timer, else the kept activity.
pub fn current_view(timer: Option<&TimerView>, kept: Option<&KeptActivity>) -> Option<TimerView> {
    timer.cloned().or_else(|| kept.map(TimerView::from))
}

/// The Daybook project among `offered` that is current: the running timer's, else the kept
/// activity's.
pub fn current_project(
    offered: &[&Project],
    timer: Option<&TimerState>,
    kept: Option<&KeptActivity>,
) -> Option<String> {
    link::current_project(offered, timer).or_else(|| {
        kept.filter(|activity| {
            offered
                .iter()
                .any(|project| project.name == activity.project)
        })
        .map(|activity| activity.project.clone())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::LinkedProject;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
    }

    fn kept(project: &str, note: &str) -> KeptActivity {
        KeptActivity {
            project: project.into(),
            note: note.into(),
        }
    }

    fn project(name: &str, links: &[&str]) -> Project {
        let mut project = Project::new(name);
        project.timetracker_projects = links
            .iter()
            .map(|id| LinkedProject {
                id: (*id).into(),
                name: (*id).into(),
            })
            .collect();
        project
    }

    fn timer(project_id: &str, note: Option<&str>) -> TimerState {
        TimerState {
            recording_id: "r1".into(),
            project_id: project_id.into(),
            project: "TimeTracker name".into(),
            client: None,
            tag_id: None,
            tag: None,
            note: note.map(str::to_string),
            started_at: 0,
        }
    }

    #[test]
    fn there_is_no_activity_before_one_is_kept() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(CurrentActivity::open(dir.path()).get(day(27)), None);
    }

    #[test]
    fn a_kept_activity_is_returned_and_reported_as_changed_once() {
        let dir = tempfile::tempdir().unwrap();
        let current = CurrentActivity::open(dir.path());
        let activity = kept("Oscorp", "QRS-6805");
        assert!(current.set(day(27), Some(activity.clone())).unwrap());
        assert_eq!(current.get(day(27)), Some(activity.clone()));
        assert!(!current.set(day(27), Some(activity)).unwrap());
    }

    #[test]
    fn a_new_activity_replaces_the_old_one_and_none_clears_it() {
        let dir = tempfile::tempdir().unwrap();
        let current = CurrentActivity::open(dir.path());
        current.set(day(27), Some(kept("Oscorp", "a"))).unwrap();
        assert!(current.set(day(27), Some(kept("Hobby", "b"))).unwrap());
        assert_eq!(current.get(day(27)), Some(kept("Hobby", "b")));
        assert!(current.set(day(27), None).unwrap());
        assert_eq!(current.get(day(27)), None);
    }

    #[test]
    fn the_activity_ends_with_the_day() {
        let dir = tempfile::tempdir().unwrap();
        let current = CurrentActivity::open(dir.path());
        current.set(day(27), Some(kept("Oscorp", "a"))).unwrap();
        assert_eq!(current.get(day(28)), None);
    }

    #[test]
    fn the_same_activity_on_the_next_day_is_kept_again() {
        let dir = tempfile::tempdir().unwrap();
        let current = CurrentActivity::open(dir.path());
        current.set(day(27), Some(kept("Oscorp", "a"))).unwrap();
        assert!(current.set(day(28), Some(kept("Oscorp", "a"))).unwrap());
        assert_eq!(current.get(day(28)), Some(kept("Oscorp", "a")));
    }

    #[test]
    fn the_activity_survives_reopening() {
        let dir = tempfile::tempdir().unwrap();
        CurrentActivity::open(dir.path())
            .set(day(27), Some(kept("Oscorp", "QRS-6805")))
            .unwrap();
        let reopened = CurrentActivity::open(dir.path());
        assert_eq!(reopened.get(day(27)), Some(kept("Oscorp", "QRS-6805")));
        assert_eq!(reopened.get(day(28)), None);
    }

    #[test]
    fn a_kept_activity_shows_as_a_timer_view() {
        assert_eq!(
            TimerView::from(&kept("Oscorp", "QRS-6805")),
            TimerView {
                project: "Oscorp".into(),
                note: "QRS-6805".into(),
            }
        );
    }

    fn view(project: &str, note: &str) -> TimerView {
        TimerView {
            project: project.into(),
            note: note.into(),
        }
    }

    #[test]
    fn the_running_timer_wins_over_the_kept_activity_in_the_current_view() {
        let running = view("Oscorp", "timer note");
        assert_eq!(
            current_view(Some(&running), Some(&kept("Hobby", "kept note"))),
            Some(running)
        );
    }

    #[test]
    fn the_kept_activity_shows_in_the_current_view_without_a_timer() {
        assert_eq!(
            current_view(None, Some(&kept("Hobby", "weeding"))),
            Some(view("Hobby", "weeding"))
        );
    }

    #[test]
    fn the_current_view_is_empty_without_timer_and_activity() {
        assert_eq!(current_view(None, None), None);
    }

    #[test]
    fn the_timers_offered_project_is_the_current_project_over_the_kept_one() {
        let linked = project("Oscorp", &["sdk"]);
        let hobby = project("Hobby", &[]);
        let offered = [&linked, &hobby];
        assert_eq!(
            current_project(
                &offered,
                Some(&timer("sdk", None)),
                Some(&kept("Hobby", ""))
            ),
            Some("Oscorp".to_string())
        );
    }

    #[test]
    fn the_kept_project_is_current_only_when_offered() {
        let linked = project("Oscorp", &["sdk"]);
        let hobby = project("Hobby", &[]);
        let offered = [&linked, &hobby];
        for (activity, expected) in [
            (kept("Hobby", ""), Some("Hobby".to_string())),
            (kept("Gone", ""), None),
        ] {
            assert_eq!(
                current_project(&offered, None, Some(&activity)),
                expected,
                "{activity:?}"
            );
            assert_eq!(
                current_project(&offered, Some(&timer("unlinked", None)), Some(&activity)),
                expected,
                "unlinked timer, {activity:?}"
            );
        }
    }

    #[test]
    fn there_is_no_current_project_without_timer_and_activity() {
        let hobby = project("Hobby", &[]);
        assert_eq!(current_project(&[&hobby], None, None), None);
    }

    #[test]
    fn without_a_timer_the_activity_stays() {
        let projects = [project("Oscorp", &["sdk"])];
        for activity in [None, Some(kept("Hobby", "weeding"))] {
            assert_eq!(
                follow_timer(activity.as_ref(), None, &projects),
                activity,
                "{activity:?}"
            );
        }
    }

    #[test]
    fn a_running_timer_sets_the_activity_to_its_note_and_the_project_linking_it() {
        let projects = [project("Hobby", &[]), project("Oscorp", &["sdk"])];
        let next = follow_timer(None, Some(&timer("sdk", Some("QRS-6805"))), &projects);
        assert_eq!(next, Some(kept("Oscorp", "QRS-6805")));
    }

    #[test]
    fn a_timer_without_a_note_sets_an_empty_note() {
        let projects = [project("Oscorp", &["sdk"])];
        let next = follow_timer(
            Some(&kept("Oscorp", "old")),
            Some(&timer("sdk", None)),
            &projects,
        );
        assert_eq!(next, Some(kept("Oscorp", "")));
    }

    #[test]
    fn the_kept_project_stays_while_it_links_the_timers_project() {
        let projects = [project("Client A", &["sdk"]), project("Client B", &["sdk"])];
        let next = follow_timer(
            Some(&kept("Client B", "old")),
            Some(&timer("sdk", Some("new"))),
            &projects,
        );
        assert_eq!(next, Some(kept("Client B", "new")));
    }

    #[test]
    fn another_timer_project_moves_the_activity_to_the_first_project_linking_it() {
        let projects = [
            project("Client A", &["sdk"]),
            project("Client B", &["shared"]),
            project("Client C", &["shared"]),
        ];
        for kept_project in ["Client A", "Gone"] {
            let next = follow_timer(
                Some(&kept(kept_project, "old")),
                Some(&timer("shared", Some("new"))),
                &projects,
            );
            assert_eq!(next, Some(kept("Client B", "new")), "{kept_project}");
        }
    }

    #[test]
    fn a_timer_on_an_unlinked_project_is_named_as_timetracker_names_it() {
        let projects = [project("Oscorp", &["sdk"])];
        let next = follow_timer(
            Some(&kept("Oscorp", "old")),
            Some(&timer("unlinked", Some("note"))),
            &projects,
        );
        assert_eq!(next, Some(kept("TimeTracker name", "note")));
    }
}
