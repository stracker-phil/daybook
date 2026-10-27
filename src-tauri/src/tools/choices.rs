//! What each tool lets you pick directly, as plain data.

use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use serde::Serialize;
use ts_rs::TS;

use super::state::{LinkItem, Progress, Status, ToolState};
use crate::contexts::context_title;
use crate::launcher::open_command;
use crate::store::settings::{Feature, ToolId};

/// The rail's tooltip lists them as they serialize.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Choice {
    Item {
        label: String,
        command: String,
        detail: Option<String>,
        /// Some for a pick among several (a radio item); `true` marks the current one.
        checked: Option<bool>,
    },
    Heading {
        label: String,
    },
    Separator,
    /// A scale such as the volume: one step is active.
    Steps {
        label: String,
        steps: Vec<Step>,
    },
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "ChoiceStep")]
pub struct Step {
    pub label: String,
    pub command: String,
    pub active: bool,
}

fn item(label: impl Into<String>, command: impl Into<String>) -> Choice {
    Choice::Item {
        label: label.into(),
        command: command.into(),
        detail: None,
        checked: None,
    }
}

fn heading(label: impl Into<String>) -> Choice {
    Choice::Heading {
        label: label.into(),
    }
}

fn item_with_detail(
    label: impl Into<String>,
    command: impl Into<String>,
    detail: Option<String>,
) -> Choice {
    Choice::Item {
        label: label.into(),
        command: command.into(),
        detail,
        checked: None,
    }
}

/// Two sections joined; the headings only show when both have items, to tell them apart.
fn sections(
    first: Vec<Choice>,
    first_heading: Option<&str>,
    second: Vec<Choice>,
    second_heading: &str,
) -> Vec<Choice> {
    if first.is_empty() || second.is_empty() {
        return [first, second].concat();
    }
    let mut choices: Vec<Choice> = first_heading.map(heading).into_iter().collect();
    choices.extend(first);
    choices.push(heading(second_heading));
    choices.extend(second);
    choices
}

fn radio(label: impl Into<String>, command: impl Into<String>, checked: bool) -> Choice {
    Choice::Item {
        label: label.into(),
        command: command.into(),
        detail: None,
        checked: Some(checked),
    }
}

const STATUSES: [(Status, &str, &str); 3] = [
    (Status::Working, "Working", "status:working"),
    (Status::Break, "Break", "status:break"),
    (Status::Off, "Done for Today", "status:off"),
];

const VOLUME_STEPS: [u8; 5] = [10, 25, 50, 75, 100];

/// What `encodeURIComponent` leaves alone.
const COMPONENT: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'_')
    .remove(b'.')
    .remove(b'!')
    .remove(b'~')
    .remove(b'*')
    .remove(b'\'')
    .remove(b'(')
    .remove(b')');

const LINK_PREFIX: &str = "link:";

/// The command id carries the target, so a list built earlier still opens the right link.
pub fn link_command(target: &str) -> String {
    format!("{LINK_PREFIX}{}", link_argument(target))
}

/// The `link:` family's argument for `target`: the target percent-encoded.
pub fn link_argument(target: &str) -> String {
    utf8_percent_encode(target, COMPONENT).to_string()
}

/// The target behind a `link:` argument; an argument that isn't encoded is the target itself.
pub fn link_target(argument: &str) -> String {
    percent_decode_str(argument)
        .decode_utf8()
        .map_or_else(|_| argument.to_string(), |target| target.into_owned())
}

fn progress(progress: &Progress) -> Option<String> {
    match progress {
        Progress { total: 0, .. } => None,
        Progress { done, total } if done >= total => Some("Done".into()),
        Progress { done, total } => Some(format!("{done}/{total}")),
    }
}

fn link_items(items: &[LinkItem]) -> Vec<Choice> {
    items
        .iter()
        .flat_map(|entry| match entry {
            LinkItem::Link(link) => vec![item(&link.title, link_command(&link.target))],
            LinkItem::Group { title, links } => links
                .iter()
                .map(|link| {
                    item_with_detail(&link.title, link_command(&link.target), Some(title.clone()))
                })
                .collect(),
        })
        .collect()
}

/// The links that match now, then the rest; the headings only tell two groups apart.
fn link_choices(state: &ToolState) -> Vec<Choice> {
    sections(
        link_items(&state.links_now),
        Some("Now"),
        link_items(&state.links_more),
        "More",
    )
}

/// Each project's Launcher projects; headings only when there are several projects.
fn project_choices(state: &ToolState) -> Vec<Choice> {
    let open = |id: &str, name: &str| item(name, open_command(id));
    if let [group] = state.projects.as_slice() {
        return group
            .projects
            .iter()
            .map(|p| open(&p.id, &p.name))
            .collect();
    }
    state
        .projects
        .iter()
        .flat_map(|group| {
            std::iter::once(heading(&group.project))
                .chain(group.projects.iter().map(|p| open(&p.id, &p.name)))
        })
        .collect()
}

fn journal_choices(state: &ToolState) -> Vec<Choice> {
    let groups: Vec<Choice> = state
        .journal_groups
        .iter()
        .map(|group| {
            item_with_detail(
                &group.title,
                format!("journal:open:{}", group.title),
                progress(&group.progress),
            )
        })
        .collect();
    let routines: Vec<Choice> = state
        .routines
        .iter()
        .map(|routine| {
            item_with_detail(
                &routine.name,
                format!("routine:{}", routine.id),
                progress(&routine.progress),
            )
        })
        .collect();
    sections(groups, None, routines, "Routines")
}

fn nearest_step(volume: u8) -> u8 {
    VOLUME_STEPS
        .into_iter()
        .min_by_key(|step| step.abs_diff(volume))
        .unwrap_or(volume)
}

fn music_choices(state: &ToolState) -> Vec<Choice> {
    let music = &state.music;
    if music.entries.is_empty() {
        return Vec::new();
    }
    let mut choices = vec![
        item(if music.playing { "Pause" } else { "Play" }, "music:toggle"),
        Choice::Separator,
    ];
    choices.extend(music.entries.iter().map(|entry| {
        radio(
            &entry.name,
            format!("music:play:{}", entry.id),
            music.entry.as_deref() == Some(entry.id.as_str()),
        )
    }));
    if let Some(volume) = music.volume {
        let nearest = nearest_step(volume);
        choices.push(Choice::Separator);
        choices.push(Choice::Steps {
            label: format!("Volume: {volume} %"),
            steps: VOLUME_STEPS
                .into_iter()
                .map(|step| Step {
                    label: step.to_string(),
                    command: format!("music:volume:{step}"),
                    active: step == nearest,
                })
                .collect(),
        });
    }
    choices
}

fn context_choices(state: &ToolState) -> Vec<Choice> {
    if state.contexts.len() < 2 {
        return Vec::new();
    }
    state
        .contexts
        .iter()
        .map(|context| {
            radio(
                context_title(&context.label, &context.name),
                format!("mode:{}", context.id),
                state.context.as_deref() == Some(context.id.as_str()),
            )
        })
        .collect()
}

fn status_choices(state: &ToolState) -> Vec<Choice> {
    if !state.has_feature(Feature::Status) {
        return Vec::new();
    }
    STATUSES
        .iter()
        .map(|(status, label, command)| radio(*label, *command, state.status == Some(*status)))
        .collect()
}

fn search_choices(state: &ToolState) -> Vec<Choice> {
    state
        .search_scopes
        .iter()
        .map(|scope| item(scope, format!("search:scope:{scope}")))
        .collect()
}

fn ticket_choices(state: &ToolState) -> Vec<Choice> {
    if !state.has_feature(Feature::Jira) {
        return Vec::new();
    }
    state
        .tickets
        .iter()
        .map(|key| item(key, format!("jira:issue:{key}")))
        .collect()
}

fn note_topic_choices(state: &ToolState) -> Vec<Choice> {
    state
        .note_topics
        .iter()
        .map(|topic| {
            item(
                &topic.title,
                format!("notes:topic:{}:{}", topic.level, topic.title),
            )
        })
        .collect()
}

pub fn tool_choices(id: ToolId, state: &ToolState) -> Vec<Choice> {
    match id {
        ToolId::Context => context_choices(state),
        ToolId::Status => status_choices(state),
        ToolId::Journal => journal_choices(state),
        ToolId::Search => search_choices(state),
        ToolId::Jira => ticket_choices(state),
        ToolId::QuickNotes => note_topic_choices(state),
        ToolId::Links => link_choices(state),
        ToolId::Projects => project_choices(state),
        ToolId::Music => music_choices(state),
        _ => Vec::new(),
    }
}

/// Every command a list of choices can run.
pub fn choice_commands(choices: &[Choice]) -> Vec<String> {
    choices
        .iter()
        .flat_map(|choice| match choice {
            Choice::Item { command, .. } => vec![command.clone()],
            Choice::Steps { steps, .. } => steps.iter().map(|step| step.command.clone()).collect(),
            _ => Vec::new(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::state::{
        GroupProgress, LauncherProject, Link, Music, MusicItem, ProjectGroup, RoutineProgress,
        Topic, bare_state, fixture,
    };

    fn labels(choices: &[Choice]) -> Vec<String> {
        choices
            .iter()
            .map(|choice| match choice {
                Choice::Separator => "—".to_string(),
                Choice::Heading { label } => format!("[{label}]"),
                Choice::Item { label, .. } | Choice::Steps { label, .. } => label.clone(),
            })
            .collect()
    }

    fn picked(label: &str, command: &str, detail: Option<&str>, checked: Option<bool>) -> Choice {
        Choice::Item {
            label: label.into(),
            command: command.into(),
            detail: detail.map(String::from),
            checked,
        }
    }

    fn group(title: &str, done: u32, total: u32) -> GroupProgress {
        GroupProgress {
            title: title.into(),
            progress: Progress { done, total },
        }
    }

    fn launcher_group(project: &str, id: &str, name: &str) -> ProjectGroup {
        ProjectGroup {
            project: project.into(),
            projects: vec![LauncherProject {
                id: id.into(),
                name: name.into(),
            }],
        }
    }

    fn music_with(volume: Option<u8>, playing: bool, entry: Option<&str>) -> ToolState {
        ToolState {
            music: Music {
                entries: vec![
                    MusicItem {
                        id: "cell".into(),
                        name: "Cell".into(),
                    },
                    MusicItem {
                        id: "radio".into(),
                        name: "Radio".into(),
                    },
                ],
                playing,
                volume,
                entry: entry.map(String::from),
            },
            ..bare_state()
        }
    }

    #[test]
    fn the_current_status_is_marked() {
        let state = ToolState {
            status: Some(Status::Break),
            ..fixture()
        };
        assert_eq!(
            tool_choices(ToolId::Status, &state),
            vec![
                picked("Working", "status:working", None, Some(false)),
                picked("Break", "status:break", None, Some(true)),
                picked("Done for Today", "status:off", None, Some(false)),
            ]
        );
    }

    #[test]
    fn no_status_is_marked_before_one_was_set() {
        let state = ToolState {
            status: None,
            ..fixture()
        };
        let checked: Vec<Option<bool>> = tool_choices(ToolId::Status, &state)
            .into_iter()
            .map(|choice| match choice {
                Choice::Item { checked, .. } => checked,
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(checked, vec![Some(false); 3]);
    }

    #[test]
    fn the_status_needs_the_status_feature() {
        let state = ToolState {
            features: vec![Feature::Jira],
            ..fixture()
        };
        assert_eq!(tool_choices(ToolId::Status, &state), vec![]);
    }

    #[test]
    fn the_current_context_is_marked_and_named_with_its_label() {
        assert_eq!(
            tool_choices(ToolId::Context, &fixture()),
            vec![
                picked("[W] Work", "mode:work", None, Some(true)),
                picked("Private", "mode:private", None, Some(false)),
            ]
        );
    }

    #[test]
    fn a_single_context_has_nothing_to_pick() {
        let mut state = fixture();
        state.contexts.truncate(1);
        assert_eq!(tool_choices(ToolId::Context, &state), vec![]);
    }

    #[test]
    fn journal_groups_show_their_progress_and_routines_follow_under_a_heading() {
        let state = ToolState {
            journal_groups: vec![
                group("Dawn", 3, 3),
                group("Notiz", 0, 0),
                group("Dusk", 1, 4),
            ],
            ..fixture()
        };
        let choices = tool_choices(ToolId::Journal, &state);
        assert_eq!(
            choices,
            vec![
                picked("Dawn", "journal:open:Dawn", Some("Done"), None),
                picked("Notiz", "journal:open:Notiz", None, None),
                picked("Dusk", "journal:open:Dusk", Some("1/4"), None),
                heading("Routines"),
                picked("Wrap-up", "routine:wrap", Some("1/3"), None),
            ]
        );
    }

    #[test]
    fn a_finished_routine_reads_done() {
        let state = ToolState {
            journal_groups: vec![],
            routines: vec![RoutineProgress {
                id: "wrap".into(),
                name: "Wrap-up".into(),
                progress: Progress { done: 5, total: 3 },
            }],
            ..fixture()
        };
        assert_eq!(
            tool_choices(ToolId::Journal, &state),
            vec![picked("Wrap-up", "routine:wrap", Some("Done"), None)]
        );
    }

    #[test]
    fn the_routines_heading_only_appears_next_to_journal_groups() {
        let only_routines = ToolState {
            journal_groups: vec![],
            ..fixture()
        };
        assert_eq!(
            labels(&tool_choices(ToolId::Journal, &only_routines)),
            ["Wrap-up"]
        );
        let only_groups = ToolState {
            routines: vec![],
            ..fixture()
        };
        assert_eq!(
            labels(&tool_choices(ToolId::Journal, &only_groups)),
            ["Dawn", "Health"]
        );
    }

    #[test]
    fn projects_have_headings_only_for_several_projects() {
        let one = ToolState {
            projects: vec![launcher_group("Oscorp", "p1", "QRS")],
            ..fixture()
        };
        assert_eq!(
            tool_choices(ToolId::Projects, &one),
            vec![picked("QRS", "projects:open:p1", None, None)]
        );
        let two = ToolState {
            projects: vec![
                launcher_group("Oscorp", "p1", "QRS"),
                launcher_group("Zeno", "c1", "Portal"),
            ],
            ..fixture()
        };
        assert_eq!(
            labels(&tool_choices(ToolId::Projects, &two)),
            ["[Oscorp]", "QRS", "[Zeno]", "Portal"]
        );
        assert_eq!(
            choice_commands(&tool_choices(ToolId::Projects, &two)),
            ["projects:open:p1", "projects:open:c1"]
        );
    }

    #[test]
    fn links_for_now_are_separated_from_the_rest() {
        let choices = tool_choices(ToolId::Links, &fixture());
        assert_eq!(
            choices,
            vec![
                heading("Now"),
                picked("Board", "link:https%3A%2F%2Fboard", None, None),
                heading("More"),
                picked("Wiki", "link:https%3A%2F%2Fwiki", Some("Oscorp"), None),
            ]
        );
    }

    #[test]
    fn links_have_no_headings_when_only_one_list_has_any() {
        let only_more = ToolState {
            links_now: vec![],
            ..fixture()
        };
        assert_eq!(labels(&tool_choices(ToolId::Links, &only_more)), ["Wiki"]);
        let only_now = ToolState {
            links_more: vec![],
            ..fixture()
        };
        assert_eq!(labels(&tool_choices(ToolId::Links, &only_now)), ["Board"]);
        assert_eq!(tool_choices(ToolId::Links, &bare_state()), vec![]);
    }

    #[test]
    fn a_projects_links_carry_the_project_as_detail() {
        let state = ToolState {
            links_now: vec![LinkItem::Group {
                title: "Oscorp".into(),
                links: vec![
                    Link {
                        title: "Wiki".into(),
                        target: "https://wiki".into(),
                    },
                    Link {
                        title: "Board".into(),
                        target: "https://board".into(),
                    },
                ],
            }],
            links_more: vec![],
            ..fixture()
        };
        let details: Vec<Option<String>> = tool_choices(ToolId::Links, &state)
            .into_iter()
            .map(|choice| match choice {
                Choice::Item { detail, .. } => detail,
                other => panic!("unexpected {other:?}"),
            })
            .collect();
        assert_eq!(details, vec![Some("Oscorp".to_string()); 2]);
    }

    #[test]
    fn search_scopes_note_topics_and_tickets_are_commands() {
        let state = fixture();
        assert_eq!(
            choice_commands(&tool_choices(ToolId::Search, &state)),
            ["search:scope:work", "search:scope:all"]
        );
        assert_eq!(
            tool_choices(ToolId::QuickNotes, &state),
            vec![picked("Daily", "notes:topic:3:Daily", None, None)]
        );
        assert_eq!(
            tool_choices(ToolId::Jira, &state),
            vec![picked("QRS-1", "jira:issue:QRS-1", None, None)]
        );
    }

    #[test]
    fn tickets_need_the_jira_feature() {
        let state = ToolState {
            features: vec![Feature::Status],
            ..fixture()
        };
        assert_eq!(tool_choices(ToolId::Jira, &state), vec![]);
    }

    #[test]
    fn the_music_offers_play_the_entries_and_volume_steps() {
        let choices = tool_choices(ToolId::Music, &fixture());
        assert_eq!(
            labels(&choices),
            ["Pause", "—", "Cell", "—", "Volume: 48 %"]
        );
        assert_eq!(choices[0], picked("Pause", "music:toggle", None, None));
        assert_eq!(
            choices[2],
            picked("Cell", "music:play:cell", None, Some(true))
        );
        let Choice::Steps { steps, .. } = &choices[4] else {
            panic!("expected steps, got {:?}", choices[4]);
        };
        let seen: Vec<(&str, &str, bool)> = steps
            .iter()
            .map(|s| (s.label.as_str(), s.command.as_str(), s.active))
            .collect();
        assert_eq!(
            seen,
            vec![
                ("10", "music:volume:10", false),
                ("25", "music:volume:25", false),
                ("50", "music:volume:50", true),
                ("75", "music:volume:75", false),
                ("100", "music:volume:100", false),
            ]
        );
    }

    #[test]
    fn the_active_volume_step_is_the_nearest_one() {
        for (volume, active) in [
            (0, "10"),
            (17, "10"),
            (18, "25"),
            (60, "50"),
            (63, "75"),
            (90, "100"),
            (100, "100"),
        ] {
            let choices = tool_choices(ToolId::Music, &music_with(Some(volume), true, None));
            let Some(Choice::Steps { steps, .. }) = choices.last() else {
                panic!("no steps for {volume}");
            };
            let on: Vec<&str> = steps
                .iter()
                .filter(|s| s.active)
                .map(|s| s.label.as_str())
                .collect();
            assert_eq!(on, [active], "{volume}");
        }
    }

    #[test]
    fn music_offers_play_while_paused_and_marks_no_entry_before_one_played() {
        let choices = tool_choices(ToolId::Music, &music_with(None, false, None));
        assert_eq!(
            choices,
            vec![
                picked("Play", "music:toggle", None, None),
                Choice::Separator,
                picked("Cell", "music:play:cell", None, Some(false)),
                picked("Radio", "music:play:radio", None, Some(false)),
            ]
        );
    }

    #[test]
    fn music_without_entries_has_nothing_to_pick() {
        assert_eq!(tool_choices(ToolId::Music, &bare_state()), vec![]);
    }

    #[test]
    fn choice_commands_list_items_and_steps_but_not_headings_or_separators() {
        let choices = tool_choices(ToolId::Music, &fixture());
        assert_eq!(
            choice_commands(&choices),
            [
                "music:toggle",
                "music:play:cell",
                "music:volume:10",
                "music:volume:25",
                "music:volume:50",
                "music:volume:75",
                "music:volume:100",
            ]
        );
        assert_eq!(
            choice_commands(&[heading("x"), Choice::Separator]),
            Vec::<String>::new()
        );
    }

    #[test]
    fn tools_without_choices_offer_nothing() {
        let state = fixture();
        for id in [
            ToolId::Hub,
            ToolId::Activity,
            ToolId::Call,
            ToolId::Separator1,
            ToolId::Separator2,
        ] {
            assert_eq!(tool_choices(id, &state), vec![], "{id:?}");
        }
        for id in [
            ToolId::Search,
            ToolId::QuickNotes,
            ToolId::Journal,
            ToolId::Links,
            ToolId::Projects,
        ] {
            assert_eq!(tool_choices(id, &bare_state()), vec![], "{id:?}");
        }
    }

    #[test]
    fn a_link_command_carries_the_encoded_target() {
        for (target, command) in [
            ("https://wiki", "link:https%3A%2F%2Fwiki"),
            ("a b&c", "link:a%20b%26c"),
            ("it's-(1)_~.!*", "link:it's-(1)_~.!*"),
            ("grüße", "link:gr%C3%BC%C3%9Fe"),
        ] {
            assert_eq!(link_command(target), command, "{target}");
        }
    }

    #[test]
    fn a_link_target_is_decoded_from_its_argument() {
        for (argument, target) in [
            ("https%3A%2F%2Fwiki", "https://wiki"),
            ("a%20b%26c", "a b&c"),
            ("https://plain", "https://plain"),
            ("%FF", "%FF"),
        ] {
            assert_eq!(link_target(argument), target, "{argument}");
        }
    }

    #[test]
    fn every_link_target_survives_the_command_round_trip() {
        for target in [
            "https://wiki?q=a b#x",
            "obsidian://open?vault=A&file=B%20C",
            "grüße 100%",
        ] {
            let command = link_command(target);
            let argument = command.strip_prefix("link:").expect("prefix");
            assert_eq!(link_target(argument), target);
        }
    }

    #[test]
    fn note_topics_keep_their_level_in_the_command() {
        let state = ToolState {
            note_topics: vec![
                Topic {
                    title: "Daily".into(),
                    level: 3,
                },
                Topic {
                    title: "Ideas".into(),
                    level: 2,
                },
            ],
            ..fixture()
        };
        assert_eq!(
            choice_commands(&tool_choices(ToolId::QuickNotes, &state)),
            ["notes:topic:3:Daily", "notes:topic:2:Ideas"]
        );
    }

    #[test]
    fn choices_serialize_with_their_kind_as_tag() {
        let choices = vec![
            heading("Now"),
            picked("Pause", "music:toggle", Some("Playing"), Some(true)),
            Choice::Separator,
            Choice::Steps {
                label: "Volume: 50 %".into(),
                steps: vec![Step {
                    label: "50".into(),
                    command: "music:volume:50".into(),
                    active: true,
                }],
            },
        ];
        assert_eq!(
            serde_json::to_value(&choices).expect("serializes"),
            serde_json::json!([
                { "kind": "heading", "label": "Now" },
                {
                    "kind": "item",
                    "label": "Pause",
                    "command": "music:toggle",
                    "detail": "Playing",
                    "checked": true,
                },
                { "kind": "separator" },
                {
                    "kind": "steps",
                    "label": "Volume: 50 %",
                    "steps": [
                        { "label": "50", "command": "music:volume:50", "active": true },
                    ],
                },
            ])
        );
    }

    #[test]
    fn an_item_without_detail_or_pick_state_serializes_them_as_null() {
        assert_eq!(
            serde_json::to_value(item("Board", "link:x")).expect("serializes"),
            serde_json::json!({
                "kind": "item",
                "label": "Board",
                "command": "link:x",
                "detail": null,
                "checked": null,
            })
        );
    }
}
