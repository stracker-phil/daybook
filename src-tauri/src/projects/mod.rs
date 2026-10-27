//! Projects: which count right now (the one gate), the meeting a title is for, their Launcher projects
//! (and their names following Project Launcher), and links following a renamed or removed project. Pure.

use crate::launcher::parse::ListedProject;
use crate::store::settings::{Project, QuickLink};
use crate::tools::{LauncherProject, ProjectGroup};

/// Active projects with no contexts or with `context`: the only list anything offers, picks or matches from.
pub fn current_projects<'a>(projects: &'a [Project], context: Option<&str>) -> Vec<&'a Project> {
    projects
        .iter()
        .filter(|project| {
            project.active
                && (project.contexts.is_empty()
                    || context.is_some_and(|id| project.contexts.iter().any(|c| c == id)))
        })
        .collect()
}

/// The first of `projects` whose name or one of its meeting words is in the title, ignoring case.
pub fn meeting_project(title: &str, projects: &[&Project]) -> Option<String> {
    let text = title.to_lowercase();
    projects
        .iter()
        .find(|project| {
            std::iter::once(&project.name)
                .chain(&project.keywords)
                .any(|word| !word.is_empty() && text.contains(&word.to_lowercase()))
        })
        .map(|project| project.name.clone())
}

/// The Launcher projects of `projects` that have any, `current` first, then in Settings order.
pub fn project_groups(projects: &[&Project], current: Option<&str>) -> Vec<ProjectGroup> {
    let mut groups: Vec<ProjectGroup> = projects
        .iter()
        .filter(|project| !project.launcher_projects.is_empty())
        .map(|project| ProjectGroup {
            project: project.name.clone(),
            projects: project
                .launcher_projects
                .iter()
                .map(|linked| LauncherProject {
                    id: linked.id.clone(),
                    name: linked.name.clone(),
                })
                .collect(),
        })
        .collect();
    groups.sort_by_key(|group| Some(group.project.as_str()) != current);
    groups
}

/// Linked Launcher projects take the name `listed` has for their id; whether any changed.
pub fn follow_launcher_names(projects: &mut [Project], listed: &[ListedProject]) -> bool {
    let mut changed = false;
    for linked in projects
        .iter_mut()
        .flat_map(|p| p.launcher_projects.iter_mut())
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

/// Projects renamed in place: same position, the old name gone and the new one new.
fn renamed<'a>(previous: &'a [Project], next: &'a [Project]) -> Vec<(&'a str, &'a str)> {
    if previous.len() != next.len() {
        return Vec::new();
    }
    previous
        .iter()
        .zip(next)
        .filter(|(before, after)| {
            before.name != after.name
                && !next.iter().any(|project| project.name == before.name)
                && !previous.iter().any(|project| project.name == after.name)
        })
        .map(|(before, after)| (before.name.as_str(), after.name.as_str()))
        .collect()
}

/// Links follow renamed projects and drop removed ones; a link left with none shows for every project.
pub fn follow_projects(previous: &[Project], next: &[Project], links: &mut [QuickLink]) {
    let renamed = renamed(previous, next);
    let kept = |name: &str| next.iter().any(|project| project.name == name);
    for link in links.iter_mut() {
        let Some(names) = link.projects.as_mut() else {
            continue;
        };
        for name in names.iter_mut() {
            if let Some((_, to)) = renamed.iter().find(|(from, _)| from == name) {
                *name = to.to_string();
            }
        }
        names.retain(|name| kept(name));
        if names.is_empty() {
            link.projects = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::LinkedProject;
    use serde_json::json;

    fn project(name: &str, edit: impl FnOnce(&mut Project)) -> Project {
        let mut project = Project::new(name);
        edit(&mut project);
        project
    }

    fn linked(id: &str, name: &str) -> LinkedProject {
        LinkedProject {
            id: id.into(),
            name: name.into(),
        }
    }

    fn projects() -> Vec<Project> {
        vec![
            Project::new("Internal"),
            project("Oscorp", |p| {
                p.contexts = vec!["work".into()];
                p.keywords = vec!["QRS".into(), "Initech".into()];
                p.launcher_projects = vec![linked("p1", "QRS")];
            }),
            project("Zeno", |p| {
                p.launcher_projects = vec![linked("c1", "Portal")]
            }),
            project("Home", |p| {
                p.contexts = vec!["personal".into()];
                p.keywords = vec!["weekly".into()];
                p.launcher_projects = vec![linked("h1", "Garden")];
            }),
            project("Old", |p| {
                p.active = false;
                p.keywords = vec!["weekly".into()];
                p.launcher_projects = vec![linked("o1", "Legacy")];
            }),
        ]
    }

    fn names(projects: &[&Project]) -> Vec<String> {
        projects.iter().map(|p| p.name.clone()).collect()
    }

    fn group_names(groups: &[ProjectGroup]) -> Vec<&str> {
        groups.iter().map(|g| g.project.as_str()).collect()
    }

    fn link(projects: Option<&[&str]>) -> QuickLink {
        QuickLink::read(
            &json!({ "title": "L", "target": "https://example.com", "projects": projects }),
        )
        .unwrap()
    }

    fn link_projects(link: &QuickLink) -> Option<Vec<&str>> {
        link.projects
            .as_ref()
            .map(|names| names.iter().map(String::as_str).collect())
    }

    #[test]
    fn active_projects_without_contexts_or_with_the_current_one_are_offered() {
        let all = projects();
        assert_eq!(
            names(&current_projects(&all, Some("work"))),
            ["Internal", "Oscorp", "Zeno"]
        );
        assert_eq!(
            names(&current_projects(&all, Some("personal"))),
            ["Internal", "Zeno", "Home"]
        );
    }

    #[test]
    fn without_a_current_context_only_projects_without_contexts_are_offered() {
        let all = projects();
        assert_eq!(names(&current_projects(&all, None)), ["Internal", "Zeno"]);
    }

    #[test]
    fn a_meeting_is_found_by_the_project_name_or_a_keyword_ignoring_case() {
        let all = projects();
        let work = current_projects(&all, Some("work"));
        assert_eq!(
            meeting_project("qrs refinement", &work).as_deref(),
            Some("Oscorp")
        );
        assert_eq!(
            meeting_project("Sync with oscorp", &work).as_deref(),
            Some("Oscorp")
        );
        assert_eq!(meeting_project("Zeno Weekly", &work).as_deref(), Some("Zeno"));
    }

    #[test]
    fn a_meeting_is_matched_only_among_the_offered_projects() {
        let all = projects();
        let work = current_projects(&all, Some("work"));
        let personal = current_projects(&all, Some("personal"));
        assert_eq!(meeting_project("QRS refinement", &personal), None);
        assert_eq!(meeting_project("Weekly", &work), None);
        assert_eq!(
            meeting_project("Weekly", &personal).as_deref(),
            Some("Home")
        );
    }

    #[test]
    fn an_empty_keyword_matches_no_meeting() {
        let all = [project("Blank", |p| p.keywords = vec![String::new()])];
        let offered: Vec<&Project> = all.iter().collect();
        assert_eq!(meeting_project("anything", &offered), None);
    }

    #[test]
    fn launcher_projects_are_grouped_by_project_with_the_current_one_first() {
        let all = projects();
        let work = current_projects(&all, Some("work"));
        let personal = current_projects(&all, Some("personal"));
        assert_eq!(group_names(&project_groups(&work, None)), ["Oscorp", "Zeno"]);
        assert_eq!(
            group_names(&project_groups(&work, Some("Zeno"))),
            ["Zeno", "Oscorp"]
        );
        assert_eq!(
            group_names(&project_groups(&personal, Some("Old"))),
            ["Zeno", "Home"]
        );
    }

    #[test]
    fn a_current_project_without_launcher_projects_leaves_the_order_alone() {
        let all = projects();
        let work = current_projects(&all, Some("work"));
        assert_eq!(
            group_names(&project_groups(&work, Some("Internal"))),
            ["Oscorp", "Zeno"]
        );
        assert_eq!(
            group_names(&project_groups(&work, Some("Unknown"))),
            ["Oscorp", "Zeno"]
        );
    }

    #[test]
    fn a_group_lists_the_launcher_projects_of_its_project() {
        let all = projects();
        let work = current_projects(&all, Some("work"));
        assert_eq!(
            project_groups(&work, None)[0],
            ProjectGroup {
                project: "Oscorp".into(),
                projects: vec![LauncherProject {
                    id: "p1".into(),
                    name: "QRS".into(),
                }],
            }
        );
    }

    #[test]
    fn a_renamed_project_is_renamed_in_the_links() {
        let previous = [Project::new("Oscorp"), Project::new("Zeno")];
        let next = [Project::new("Oscorp Inc"), Project::new("Zeno")];
        let mut links = [link(Some(&["Oscorp", "Zeno"]))];
        follow_projects(&previous, &next, &mut links);
        assert_eq!(link_projects(&links[0]), Some(vec!["Oscorp Inc", "Zeno"]));
    }

    #[test]
    fn a_removed_project_is_dropped_from_the_links() {
        let previous = [Project::new("Oscorp"), Project::new("Zeno")];
        let next = [Project::new("Zeno")];
        let mut links = [link(Some(&["Oscorp", "Zeno"]))];
        follow_projects(&previous, &next, &mut links);
        assert_eq!(link_projects(&links[0]), Some(vec!["Zeno"]));
    }

    #[test]
    fn a_link_left_without_projects_shows_for_every_project() {
        let previous = [Project::new("Oscorp"), Project::new("Zeno")];
        let next = [Project::new("Zeno")];
        let mut links = [link(Some(&["Oscorp"]))];
        follow_projects(&previous, &next, &mut links);
        assert_eq!(links[0].projects, None);
    }

    #[test]
    fn links_of_untouched_projects_and_links_without_projects_stay_unchanged() {
        let previous = [Project::new("Oscorp"), Project::new("Zeno")];
        let next = [Project::new("Oscorp Inc"), Project::new("Zeno")];
        let mut links = [link(Some(&["Zeno"])), link(None)];
        let before = links.clone();
        follow_projects(&previous, &next, &mut links);
        assert_eq!(links, before);
    }

    #[test]
    fn reordering_projects_changes_nothing() {
        let previous = [Project::new("Oscorp"), Project::new("Zeno")];
        let next = [Project::new("Zeno"), Project::new("Oscorp")];
        let mut links = [link(Some(&["Oscorp", "Zeno"]))];
        let before = links.clone();
        follow_projects(&previous, &next, &mut links);
        assert_eq!(links, before);
    }

    fn listed(id: &str, name: &str) -> ListedProject {
        ListedProject {
            id: id.into(),
            name: name.into(),
            archived: false,
        }
    }

    fn linked_names(projects: &[Project]) -> Vec<Vec<(String, String)>> {
        projects
            .iter()
            .map(|p| {
                p.launcher_projects
                    .iter()
                    .map(|l| (l.id.clone(), l.name.clone()))
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_linked_project_listed_under_another_name_takes_that_name() {
        let mut all = projects();
        let changed = follow_launcher_names(&mut all, &[listed("c1", "Customer Portal")]);
        assert!(changed);
        assert_eq!(
            all[2].launcher_projects,
            vec![linked("c1", "Customer Portal")]
        );
        assert_eq!(all[1].launcher_projects, vec![linked("p1", "QRS")]);
    }

    #[test]
    fn linked_projects_that_are_not_listed_keep_their_names() {
        let mut all = projects();
        let before = linked_names(&all);
        let changed = follow_launcher_names(&mut all, &[listed("zzz", "Other")]);
        assert!(!changed);
        assert_eq!(linked_names(&all), before);
    }

    #[test]
    fn identical_names_change_nothing() {
        let mut all = projects();
        let before = linked_names(&all);
        let changed =
            follow_launcher_names(&mut all, &[listed("p1", "QRS"), listed("c1", "Portal")]);
        assert!(!changed);
        assert_eq!(linked_names(&all), before);
    }

    #[test]
    fn every_project_linking_the_same_id_is_renamed() {
        let mut all = vec![
            project("A", |p| p.launcher_projects = vec![linked("x", "Old")]),
            project("B", |p| {
                p.launcher_projects = vec![linked("y", "Keep"), linked("x", "Older")]
            }),
        ];
        let changed = follow_launcher_names(&mut all, &[listed("x", "New")]);
        assert!(changed);
        assert_eq!(all[0].launcher_projects, vec![linked("x", "New")]);
        assert_eq!(
            all[1].launcher_projects,
            vec![linked("y", "Keep"), linked("x", "New")]
        );
    }
}
