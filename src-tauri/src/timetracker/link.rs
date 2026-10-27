//! Daybook projects linked to TimeTracker projects: linking by name once, following renames, and the
//! Daybook project the timer runs on. Pure.

use super::reply::{TimerState, TtProject};
use crate::store::settings::{LinkedProject, Project};
use crate::tools::TimerView;

/// Names of the active projects without TimeTracker projects.
pub fn unlinked_project_names(projects: &[Project]) -> Vec<String> {
    projects
        .iter()
        .filter(|project| project.active && project.timetracker_projects.is_empty())
        .map(|project| project.name.clone())
        .collect()
}

fn same(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// Links each project without TimeTracker projects to the active ones whose name or client is its name
/// (ignoring case). Returns the names of the projects left without any.
pub fn auto_link(projects: &mut [Project], listed: &[TtProject]) -> Vec<String> {
    let mut unmatched = Vec::new();
    for project in projects
        .iter_mut()
        .filter(|project| project.timetracker_projects.is_empty())
    {
        project.timetracker_projects = listed
            .iter()
            .filter(|found| !found.archived)
            .filter(|found| {
                same(&found.name, &project.name)
                    || found
                        .client
                        .as_deref()
                        .is_some_and(|client| same(client, &project.name))
            })
            .map(|found| LinkedProject {
                id: found.id.clone(),
                name: found.name.clone(),
            })
            .collect();
        if project.timetracker_projects.is_empty() {
            unmatched.push(project.name.clone());
        }
    }
    unmatched
}

/// Linked TimeTracker projects take the name `listed` has for their id; whether any changed.
pub fn follow_names(projects: &mut [Project], listed: &[TtProject]) -> bool {
    let mut changed = false;
    for linked in projects
        .iter_mut()
        .flat_map(|project| project.timetracker_projects.iter_mut())
    {
        if let Some(found) = listed.iter().find(|found| found.id == linked.id)
            && found.name != linked.name
        {
            linked.name = found.name.clone();
            changed = true;
        }
    }
    changed
}

/// The first of `projects` linked to TimeTracker's project `id`.
pub fn linked_project<'a>(
    projects: impl IntoIterator<Item = &'a Project>,
    id: &str,
) -> Option<&'a Project> {
    projects.into_iter().find(|project| {
        project
            .timetracker_projects
            .iter()
            .any(|linked| linked.id == id)
    })
}

/// The Daybook project of the running timer among `offered` (the projects that count now).
pub fn current_project(offered: &[&Project], timer: Option<&TimerState>) -> Option<String> {
    linked_project(offered.iter().copied(), &timer?.project_id).map(|project| project.name.clone())
}

/// The timer as the surfaces show it: the linked Daybook project's name (any project, so it reads the
/// same in every context), else TimeTracker's.
pub fn timer_view(projects: &[Project], timer: &TimerState) -> TimerView {
    TimerView {
        project: linked_project(projects, &timer.project_id)
            .map_or_else(|| timer.project.clone(), |project| project.name.clone()),
        note: timer.note.clone().unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listed(id: &str, name: &str, client: Option<&str>, archived: bool) -> TtProject {
        TtProject {
            id: id.into(),
            name: name.into(),
            client_id: None,
            client: client.map(String::from),
            color: "blue".into(),
            shade: 4,
            billable: false,
            archived,
        }
    }

    fn linked(id: &str, name: &str) -> LinkedProject {
        LinkedProject {
            id: id.into(),
            name: name.into(),
        }
    }

    fn linked_to(name: &str, links: &[(&str, &str)]) -> Project {
        let mut project = Project::new(name);
        project.timetracker_projects = links.iter().map(|(id, name)| linked(id, name)).collect();
        project
    }

    fn timer(project_id: &str, project: &str, note: Option<&str>) -> TimerState {
        TimerState {
            recording_id: "r1".into(),
            project_id: project_id.into(),
            project: project.into(),
            client: None,
            tag_id: None,
            tag: None,
            note: note.map(String::from),
            started_at: 0,
        }
    }

    #[test]
    fn projects_link_to_the_timetracker_projects_with_their_name_or_client_ignoring_case() {
        let mut projects = [Project::new("Oscorp"), Project::new("  zeno ")];
        let found = [
            listed("t1", "oscorp", None, false),
            listed("t2", "Checkout", Some("OSCORP"), false),
            listed("t3", "Portal", Some("Zeno"), false),
            listed("t4", "Other", Some("Acme"), false),
        ];

        let unmatched = auto_link(&mut projects, &found);

        assert_eq!(unmatched, Vec::<String>::new());
        assert_eq!(
            projects[0].timetracker_projects,
            [linked("t1", "oscorp"), linked("t2", "Checkout")]
        );
        assert_eq!(projects[1].timetracker_projects, [linked("t3", "Portal")]);
    }

    #[test]
    fn archived_timetracker_projects_are_not_linked() {
        let mut projects = [Project::new("Oscorp")];
        let found = [
            listed("t1", "Oscorp", None, true),
            listed("t2", "Checkout", Some("Oscorp"), true),
        ];

        assert_eq!(auto_link(&mut projects, &found), ["Oscorp"]);
        assert_eq!(projects[0].timetracker_projects, []);
    }

    #[test]
    fn projects_that_are_linked_already_are_left_alone() {
        let mut projects = [
            linked_to("Oscorp", &[("old", "Legacy")]),
            Project::new("Home"),
        ];
        let found = [listed("t1", "Oscorp", None, false)];

        let unmatched = auto_link(&mut projects, &found);

        assert_eq!(unmatched, ["Home"]);
        assert_eq!(projects[0].timetracker_projects, [linked("old", "Legacy")]);
        assert_eq!(projects[1].timetracker_projects, []);
    }

    #[test]
    fn linking_returns_the_names_without_a_match_in_order() {
        let mut projects = [Project::new("A"), Project::new("Oscorp"), Project::new("B")];
        let found = [listed("t1", "Oscorp", None, false)];

        assert_eq!(auto_link(&mut projects, &found), ["A", "B"]);
    }

    #[test]
    fn linked_projects_take_the_name_timetracker_has_now() {
        let mut projects = [
            linked_to("Oscorp", &[("t1", "Old name"), ("t2", "Same")]),
            linked_to("Gone", &[("t9", "Deleted")]),
        ];
        let found = [
            listed("t1", "New name", None, false),
            listed("t2", "Same", None, false),
        ];

        assert!(follow_names(&mut projects, &found));
        assert_eq!(
            projects[0].timetracker_projects,
            [linked("t1", "New name"), linked("t2", "Same")]
        );
        assert_eq!(projects[1].timetracker_projects, [linked("t9", "Deleted")]);
        assert!(!follow_names(&mut projects, &found));
    }

    #[test]
    fn the_current_project_is_the_offered_one_linked_to_the_timer() {
        let oscorp = linked_to("Oscorp", &[("t1", "Core"), ("t2", "Portal")]);
        let home = linked_to("Home", &[("t3", "Garden")]);
        let offered = [&oscorp, &home];

        for (project_id, expected) in [
            ("t1", Some("Oscorp")),
            ("t2", Some("Oscorp")),
            ("t3", Some("Home")),
        ] {
            assert_eq!(
                current_project(&offered, Some(&timer(project_id, "x", None))).as_deref(),
                expected,
                "{project_id}"
            );
        }
    }

    #[test]
    fn the_current_project_is_none_without_a_timer_or_a_link_among_the_offered() {
        let oscorp = linked_to("Oscorp", &[("t1", "Core")]);

        assert_eq!(current_project(&[&oscorp], None), None);
        assert_eq!(
            current_project(&[&oscorp], Some(&timer("unlinked", "x", None))),
            None
        );
        assert_eq!(
            current_project(&[&oscorp], Some(&timer("t3", "Garden", None))),
            None
        );
    }

    #[test]
    fn the_timer_view_names_the_daybook_project_else_the_timetracker_one() {
        let projects = [linked_to("Oscorp", &[("t1", "Core")])];

        assert_eq!(
            timer_view(&projects, &timer("t1", "Core", Some("review"))),
            TimerView {
                project: "Oscorp".into(),
                note: "review".into(),
            }
        );
        assert_eq!(
            timer_view(&projects, &timer("t7", "Garden", None)),
            TimerView {
                project: "Garden".into(),
                note: String::new(),
            }
        );
    }

    #[test]
    fn the_timer_text_joins_the_project_and_the_note() {
        let projects = [linked_to("Oscorp", &[("t1", "Core")])];

        assert_eq!(
            timer_view(&projects, &timer("t1", "Core", Some("QRS-6805 review"))).text(),
            "Oscorp · QRS-6805 review"
        );
        assert_eq!(
            timer_view(&projects, &timer("t1", "Core", None)).text(),
            "Oscorp"
        );
    }
}
