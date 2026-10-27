//! Which tools a surface shows right now, in the Settings → General → Tools order.

use super::state::ToolState;
use crate::store::settings::{Feature, ToolId, ToolPlacement};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    Rail,
    Window,
}

fn is_separator(id: ToolId) -> bool {
    matches!(id, ToolId::Separator1 | ToolId::Separator2)
}

/// Drops separators at the start, at the end and right after another one.
pub fn tidy_separators(ids: Vec<ToolId>) -> Vec<ToolId> {
    let mut result: Vec<ToolId> = Vec::new();
    for id in ids {
        if is_separator(id) && result.last().is_none_or(|last| is_separator(*last)) {
            continue;
        }
        result.push(id);
    }
    while result.last().is_some_and(|last| is_separator(*last)) {
        result.pop();
    }
    result
}

/// Whether a tool has anything to offer right now (e.g. Jira only in contexts with Jira).
fn applies(id: ToolId, state: &ToolState) -> bool {
    match id {
        ToolId::Context => {
            state.contexts.len() > 1
                || state
                    .current_context()
                    .is_some_and(|context| !context.label.is_empty())
        }
        ToolId::Status => state.has_feature(Feature::Status),
        ToolId::Activity => state.has_feature(Feature::Status),
        ToolId::Call => state.meeting_since.is_some(),
        ToolId::Jira => state.has_feature(Feature::Jira),
        ToolId::Links => !state.links_now.is_empty() || !state.links_more.is_empty(),
        ToolId::Projects => !state.projects.is_empty(),
        ToolId::Music => !state.music.entries.is_empty(),
        _ => true,
    }
}

/// The tools `surface` shows, in order, separators tidied.
pub fn visible_tools(tools: &[ToolPlacement], state: &ToolState, surface: Surface) -> Vec<ToolId> {
    let shown = tools
        .iter()
        .filter(|tool| match surface {
            Surface::Rail => tool.rail,
            Surface::Window => tool.window,
        })
        .filter(|tool| applies(tool.id, state))
        .map(|tool| tool.id)
        .collect();
    tidy_separators(shown)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::state::{
        ContextItem, LauncherProject, Link, LinkItem, Music, MusicItem, ProjectGroup, bare_state,
    };

    fn place(ids: &[ToolId], off_the_rail: &[ToolId]) -> Vec<ToolPlacement> {
        ids.iter()
            .map(|id| ToolPlacement {
                id: *id,
                rail: !off_the_rail.contains(id),
                window: true,
            })
            .collect()
    }

    /// One context without a label, Status only, no call, nothing to link, play or open.
    fn quiet() -> ToolState {
        ToolState {
            contexts: vec![ContextItem {
                id: "work".into(),
                label: String::new(),
                name: "Work".into(),
            }],
            features: vec![Feature::Status],
            meeting_detection: false,
            ..bare_state()
        }
    }

    fn rail(ids: &[ToolId], state: &ToolState) -> Vec<ToolId> {
        visible_tools(&place(ids, &[]), state, Surface::Rail)
    }

    #[test]
    fn tools_with_nothing_to_offer_are_hidden_and_their_separators_tidied() {
        let tools = place(
            &[
                ToolId::Context,
                ToolId::Status,
                ToolId::Separator1,
                ToolId::Jira,
                ToolId::Links,
                ToolId::Music,
                ToolId::Separator2,
                ToolId::Search,
                ToolId::Call,
            ],
            &[],
        );
        assert_eq!(
            visible_tools(&tools, &quiet(), Surface::Rail),
            [ToolId::Status, ToolId::Separator1, ToolId::Search]
        );
        let with_links = ToolState {
            links_now: vec![LinkItem::Link(Link {
                title: "A".into(),
                target: "https://a".into(),
            })],
            ..quiet()
        };
        assert_eq!(
            visible_tools(&tools, &with_links, Surface::Rail),
            [
                ToolId::Status,
                ToolId::Separator1,
                ToolId::Links,
                ToolId::Separator2,
                ToolId::Search
            ]
        );
    }

    #[test]
    fn separators_at_the_start_the_end_and_in_a_row_are_dropped() {
        let ids = [
            ToolId::Separator1,
            ToolId::Search,
            ToolId::Separator1,
            ToolId::Separator2,
            ToolId::Hub,
            ToolId::Separator2,
        ];
        assert_eq!(
            rail(&ids, &quiet()),
            [ToolId::Search, ToolId::Separator1, ToolId::Hub]
        );
        assert_eq!(
            rail(&[ToolId::Separator1, ToolId::Separator2], &quiet()),
            []
        );
    }

    #[test]
    fn each_surface_follows_its_own_switch() {
        let tools = place(&[ToolId::Status, ToolId::Search], &[ToolId::Search]);
        let state = quiet();
        assert_eq!(
            visible_tools(&tools, &state, Surface::Rail),
            [ToolId::Status]
        );
        assert_eq!(
            visible_tools(&tools, &state, Surface::Window),
            [ToolId::Status, ToolId::Search]
        );
        let window_off = [ToolPlacement {
            id: ToolId::Status,
            rail: true,
            window: false,
        }];
        assert_eq!(visible_tools(&window_off, &state, Surface::Window), []);
    }

    #[test]
    fn the_tools_keep_the_configured_order() {
        assert_eq!(
            rail(&[ToolId::Search, ToolId::Hub, ToolId::Status], &quiet()),
            [ToolId::Search, ToolId::Hub, ToolId::Status]
        );
    }

    #[test]
    fn the_context_shows_with_several_contexts_or_a_label() {
        let one_unlabelled = quiet();
        let two = ToolState {
            contexts: vec![
                ContextItem {
                    id: "work".into(),
                    label: String::new(),
                    name: "Work".into(),
                },
                ContextItem {
                    id: "private".into(),
                    label: String::new(),
                    name: "Private".into(),
                },
            ],
            ..quiet()
        };
        let one_labelled = ToolState {
            contexts: vec![ContextItem {
                id: "work".into(),
                label: "[W]".into(),
                name: "Work".into(),
            }],
            context: Some("work".into()),
            ..quiet()
        };
        let none = ToolState {
            contexts: vec![],
            context: None,
            ..quiet()
        };
        for (name, state, shown) in [
            ("one unlabelled", one_unlabelled, false),
            ("two", two, true),
            ("one labelled", one_labelled, true),
            ("none", none, false),
        ] {
            assert_eq!(
                rail(&[ToolId::Context], &state) == [ToolId::Context],
                shown,
                "{name}"
            );
        }
    }

    #[test]
    fn status_and_jira_need_their_features() {
        let neither = ToolState {
            features: vec![],
            ..quiet()
        };
        let jira_only = ToolState {
            features: vec![Feature::Jira],
            ..quiet()
        };
        let ids = [ToolId::Status, ToolId::Jira];
        assert_eq!(rail(&ids, &neither), []);
        assert_eq!(rail(&ids, &jira_only), [ToolId::Jira]);
        assert_eq!(rail(&ids, &quiet()), [ToolId::Status]);
    }

    #[test]
    fn the_call_shows_while_a_meeting_runs() {
        let in_a_call = ToolState {
            meeting_since: Some("10:15".into()),
            ..quiet()
        };
        assert_eq!(rail(&[ToolId::Call], &in_a_call), [ToolId::Call]);
        assert_eq!(rail(&[ToolId::Call], &quiet()), []);
    }

    #[test]
    fn projects_show_only_with_linked_projects() {
        let linked = ToolState {
            projects: vec![ProjectGroup {
                project: "Oscorp".into(),
                projects: vec![LauncherProject {
                    id: "p1".into(),
                    name: "QRS".into(),
                }],
            }],
            ..quiet()
        };
        let ids = [ToolId::Status, ToolId::Projects];
        assert_eq!(rail(&ids, &quiet()), [ToolId::Status]);
        assert_eq!(rail(&ids, &linked), [ToolId::Status, ToolId::Projects]);
    }

    #[test]
    fn links_show_when_either_list_has_a_link() {
        let link = || {
            LinkItem::Link(Link {
                title: "A".into(),
                target: "https://a".into(),
            })
        };
        let now = ToolState {
            links_now: vec![link()],
            ..quiet()
        };
        let more = ToolState {
            links_more: vec![link()],
            ..quiet()
        };
        assert_eq!(rail(&[ToolId::Links], &now), [ToolId::Links]);
        assert_eq!(rail(&[ToolId::Links], &more), [ToolId::Links]);
        assert_eq!(rail(&[ToolId::Links], &quiet()), []);
    }

    #[test]
    fn music_shows_once_there_is_something_to_play() {
        let playable = ToolState {
            music: Music {
                entries: vec![MusicItem {
                    id: "cell".into(),
                    name: "Cell".into(),
                }],
                ..Music::default()
            },
            ..quiet()
        };
        assert_eq!(rail(&[ToolId::Music], &playable), [ToolId::Music]);
        assert_eq!(rail(&[ToolId::Music], &quiet()), []);
    }

    #[test]
    fn the_activity_tool_shows_with_the_status_feature() {
        assert_eq!(rail(&[ToolId::Activity], &bare_state()), [ToolId::Activity]);
        let without_status = ToolState {
            features: vec![Feature::Jira],
            ..bare_state()
        };
        assert_eq!(rail(&[ToolId::Activity], &without_status), []);
    }

    #[test]
    fn journal_hub_search_and_quick_notes_always_show() {
        let ids = [
            ToolId::Journal,
            ToolId::Hub,
            ToolId::Search,
            ToolId::QuickNotes,
        ];
        assert_eq!(rail(&ids, &quiet()), ids);
    }
}
