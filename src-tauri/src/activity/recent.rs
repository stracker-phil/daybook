//! TimeTracker's recent timers as activities: only those on a project that counts now, named by that
//! Daybook project. Pure.

use std::collections::HashSet;

use crate::store::settings::Project;
use crate::timetracker::link::linked_project;
use crate::timetracker::reply::RecentTimer;
use crate::tools::RecentActivity;

/// How many `timer recent` lists.
pub const LIMIT: u8 = 8;

/// Newest first, one per project and note.
pub fn recent_activities(recent: &[RecentTimer], offered: &[&Project]) -> Vec<RecentActivity> {
    let mut seen = HashSet::new();
    recent
        .iter()
        .filter_map(|timer| {
            let project = linked_project(offered.iter().copied(), &timer.project_id)?;
            seen.insert((project.name.clone(), timer.note.clone()))
                .then(|| RecentActivity {
                    project: project.name.clone(),
                    note: timer.note.clone(),
                    tt_project_id: timer.project_id.clone(),
                    tag: timer.tag.clone(),
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::LinkedProject;

    fn project(name: &str, tt_ids: &[&str]) -> Project {
        let mut project = Project::new(name);
        project.timetracker_projects = tt_ids
            .iter()
            .map(|id| LinkedProject {
                id: (*id).into(),
                name: format!("TT {id}"),
            })
            .collect();
        project
    }

    fn timer(project_id: &str, note: &str, tag: Option<&str>) -> RecentTimer {
        RecentTimer {
            project_id: project_id.into(),
            project: "TimeTracker's name".into(),
            tag: tag.map(str::to_string),
            note: note.into(),
        }
    }

    #[test]
    fn a_recent_timer_is_named_by_its_daybook_project_and_carries_the_tracker_project_and_tag() {
        let oscorp = project("Oscorp", &["sdk", "apollo"]);
        let recent = recent_activities(&[timer("apollo", "QRS-1 review", Some("Dev"))], &[&oscorp]);
        assert_eq!(
            recent,
            vec![RecentActivity {
                project: "Oscorp".into(),
                note: "QRS-1 review".into(),
                tt_project_id: "apollo".into(),
                tag: Some("Dev".into()),
            }]
        );
    }

    #[test]
    fn a_timer_on_a_project_that_links_to_no_offered_project_is_dropped() {
        let oscorp = project("Oscorp", &["sdk"]);
        let recent = recent_activities(
            &[timer("other", "x", None), timer("sdk", "y", None)],
            &[&oscorp],
        );
        let notes: Vec<_> = recent
            .iter()
            .map(|activity| activity.note.as_str())
            .collect();
        assert_eq!(notes, ["y"]);
    }

    #[test]
    fn repeats_of_a_project_and_note_are_dropped_keeping_the_newest() {
        let oscorp = project("Oscorp", &["sdk", "apollo"]);
        let recent = recent_activities(
            &[
                timer("sdk", "review", Some("Dev")),
                timer("apollo", "review", None),
                timer("sdk", "", None),
            ],
            &[&oscorp],
        );
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].tt_project_id, "sdk");
        assert_eq!(recent[0].tag.as_deref(), Some("Dev"));
        assert_eq!(recent[1].note, "");
    }

    #[test]
    fn nothing_is_offered_without_projects() {
        assert_eq!(recent_activities(&[timer("sdk", "x", None)], &[]), vec![]);
    }
}
