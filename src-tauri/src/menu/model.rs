//! The menu-bar menu as plain data; `tray` turns it into native items. Pure.

use super::activity::activity_entries;
use super::choices::{choices_of, overlay_then, with_choices};
pub use super::entry::MenuEntry;
use super::entry::{command, plain, submenu};
pub use super::prune::prune;
use super::prune::tidy;
pub use super::title::menu_bar_title;
pub use crate::bus::CommandStatus;
use crate::store::settings::{Feature, Hotkeys, ToolId, ToolPlacement};
use crate::tools::{Link, LinkItem, Status, ToolState, link_command};

pub struct MenuInput<'a> {
    pub state: &'a ToolState,
    /// The global shortcuts to show; empty while none are registered.
    pub hotkeys: &'a Hotkeys,
    pub tools: &'a [ToolPlacement],
}

const MAX_LINKS: usize = 10;

fn link_entry(link: &Link) -> MenuEntry {
    plain(&link.title, &link_command(&link.target))
}

fn link_entries(items: &[LinkItem]) -> Vec<MenuEntry> {
    items
        .iter()
        .map(|item| match item {
            LinkItem::Link(link) => link_entry(link),
            LinkItem::Group { title, links } => {
                submenu(title, links.iter().map(link_entry).collect())
            }
        })
        .collect()
}

/// The links that match now (at most ten, the rest under "More…"), then "More Links".
pub fn link_menu(state: &ToolState) -> Vec<MenuEntry> {
    let mut shown = link_entries(&state.links_now);
    if shown.len() > MAX_LINKS {
        let rest = shown.split_off(MAX_LINKS);
        shown.push(submenu("More…", rest));
    }
    let more = link_entries(&state.links_more);
    if !more.is_empty() {
        shown.push(submenu("More Links", more));
    }
    shown
}

fn music_entries(input: &MenuInput) -> Vec<MenuEntry> {
    let music = &input.state.music;
    let mut items = choices_of(ToolId::Music, input);
    if items.is_empty() {
        return Vec::new();
    }
    items.push(command("Music…", "music:open", &input.hotkeys.music));
    let label = music
        .playing_entry()
        .map_or("Music".into(), |entry| format!("Music: {}", entry.name));
    vec![submenu(label, items)]
}

fn context_entries(input: &MenuInput) -> Vec<MenuEntry> {
    let Some(current) = input.state.current_context() else {
        return Vec::new();
    };
    let mut items = choices_of(ToolId::Context, input);
    if items.is_empty() {
        return Vec::new();
    }
    items.push(MenuEntry::Separator);
    items.push(command(
        "Switch Context…",
        "context:switch",
        &input.hotkeys.switch_context,
    ));
    vec![submenu(format!("Context: {}", current.title()), items)]
}

fn status_name(status: Option<Status>) -> &'static str {
    match status {
        Some(Status::Working) => "Working",
        Some(Status::Break) => "Break",
        Some(Status::Off) => "Done for Today",
        None => "Not set yet",
    }
}

fn status_entries(input: &MenuInput) -> Vec<MenuEntry> {
    if !input.state.has_feature(Feature::Status) {
        return Vec::new();
    }
    let change = command(
        "Change Status…",
        "status:toggle",
        &input.hotkeys.toggle_status,
    );
    let label = format!("Status: {}", status_name(input.state.status));
    let mut entries = with_choices(&label, change, ToolId::Status, input);
    entries.push(command(
        "Today's Focus…",
        "focus:open",
        &input.hotkeys.focus,
    ));
    entries
}

/// "Journal", "Journal · 1/3" or "Journal · Done".
fn journal_title(input: &MenuInput) -> String {
    let progress = &input.state.journal;
    if progress.total == 0 || progress.done == 0 {
        "Journal".to_string()
    } else if progress.done < progress.total {
        format!("Journal · {}/{}", progress.done, progress.total)
    } else {
        "Journal · Done".to_string()
    }
}

/// With choices a submenu named by the progress; without, one item that carries it.
fn journal_entries(input: &MenuInput) -> Vec<MenuEntry> {
    let title = journal_title(input);
    let hotkey = &input.hotkeys.journal;
    let choices = choices_of(ToolId::Journal, input);
    if choices.is_empty() {
        let label = if title == "Journal" {
            "Journal…".to_string()
        } else {
            title
        };
        return vec![command(&label, "journal:open", hotkey)];
    }
    let open = command("Journal…", "journal:open", hotkey);
    vec![submenu(title, overlay_then(open, choices))]
}

fn tool_entries(id: ToolId, input: &MenuInput) -> Vec<MenuEntry> {
    let state = input.state;
    let hotkeys = input.hotkeys;
    match id {
        ToolId::Context => context_entries(input),
        ToolId::Status => status_entries(input),
        ToolId::Activity => activity_entries(input),
        ToolId::Call => state
            .meeting_since
            .iter()
            .map(|since| MenuEntry::Info(format!("In a meeting since {since}")))
            .collect(),
        ToolId::Journal => journal_entries(input),
        ToolId::Hub => {
            let mut entries = vec![command("Open Daybook", "hub:open", &hotkeys.hub)];
            if state.offers_day_activity() {
                entries.push(plain("Day Activity", "activity:showTab"));
            }
            entries.push(plain("Open in TimeTracker", "timetracker:show"));
            entries
        }
        ToolId::Search => with_choices(
            "Search Vault",
            command("Search Vault…", "search:open", &hotkeys.search),
            ToolId::Search,
            input,
        ),
        ToolId::Jira if state.has_feature(Feature::Jira) => with_choices(
            "Jira",
            command("Open Jira Issue…", "jira:open", &hotkeys.jira_open),
            ToolId::Jira,
            input,
        ),
        ToolId::Jira => Vec::new(),
        ToolId::QuickNotes => with_choices(
            "Quick Notes",
            command("Quick Notes…", "notes:quick", &hotkeys.quick_notes),
            ToolId::QuickNotes,
            input,
        ),
        ToolId::Links => vec![command("Links…", "links:open", &hotkeys.links)],
        ToolId::Projects if !state.projects.is_empty() => with_choices(
            "Projects",
            command("Projects…", "projects:open", &hotkeys.projects),
            ToolId::Projects,
            input,
        ),
        ToolId::Projects => Vec::new(),
        ToolId::Music => music_entries(input),
        ToolId::Separator1 | ToolId::Separator2 => vec![MenuEntry::Separator],
    }
}

/// Every tool in the Settings → General → Tools order.
fn tool_section(input: &MenuInput) -> Vec<MenuEntry> {
    input
        .tools
        .iter()
        .flat_map(|tool| tool_entries(tool.id, input))
        .collect()
}

fn today_note_entries(state: &ToolState) -> Vec<MenuEntry> {
    if state.today.exists {
        return Vec::new();
    }
    let mut entries = vec![MenuEntry::Command {
        label: if state.today.creating {
            "Creating Today's Note…".into()
        } else {
            "Create Today's Note".into()
        },
        command: "daily:create".into(),
        shortcut: None,
        enabled: !state.today.creating,
    }];
    let waiting = state.today.waiting;
    if waiting > 0 {
        let count = if waiting == 1 {
            "1 entry".to_string()
        } else {
            format!("{waiting} entries")
        };
        entries.push(MenuEntry::Info(format!("{count} waiting for today's note")));
    }
    entries.push(MenuEntry::Separator);
    entries
}

fn settings_and_quit() -> Vec<MenuEntry> {
    vec![
        // ⌘, is the app menu's key (`MENU_KEYS`), not a global shortcut.
        command("Settings…", "app:settings", "cmd+,"),
        plain("Quit Daybook", "app:quit"),
    ]
}

/// The whole menu before `prune`.
pub fn build_menu(input: &MenuInput) -> Vec<MenuEntry> {
    let state = input.state;
    if !state.has_vault {
        let mut menu = vec![plain("Choose Vault…", "app:settings"), MenuEntry::Separator];
        menu.extend(settings_and_quit());
        return menu;
    }
    let mut menu = today_note_entries(state);
    menu.extend(tool_section(input));
    let links = link_menu(state);
    if !links.is_empty() {
        menu.push(MenuEntry::Separator);
        menu.extend(links);
    }
    menu.push(MenuEntry::Separator);
    menu.push(plain("Open Today's Note", "daily:open"));
    menu.push(MenuEntry::Separator);
    menu.extend(settings_and_quit());
    tidy(menu)
}

/// A `CommandStatus` that knows and enables every command except the listed ones.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct CommandsStub {
    pub unknown: Vec<&'static str>,
    pub disabled: Vec<&'static str>,
}

#[cfg(test)]
impl CommandStatus for CommandsStub {
    fn knows(&self, id: &str) -> bool {
        !self.unknown.contains(&id)
    }

    fn is_enabled(&self, id: &str) -> bool {
        !self.disabled.contains(&id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::timetracker::TtState;
    use crate::tools::{
        ContextItem, GroupProgress, Link, Music, MusicItem, Progress, Topic, bare_state, fixture,
    };

    fn place(ids: &[ToolId]) -> Vec<ToolPlacement> {
        ids.iter()
            .map(|id| ToolPlacement {
                id: *id,
                rail: true,
                window: true,
            })
            .collect()
    }

    fn status_hotkeys() -> Hotkeys {
        Hotkeys {
            toggle_status: "cmd+f1".into(),
            ..Hotkeys::none()
        }
    }

    fn menu_with(state: &ToolState, hotkeys: &Hotkeys, ids: &[ToolId]) -> Vec<MenuEntry> {
        build_menu(&MenuInput {
            state,
            hotkeys,
            tools: &place(ids),
        })
    }

    fn menu_of(state: &ToolState, ids: &[ToolId]) -> Vec<MenuEntry> {
        menu_with(state, &status_hotkeys(), ids)
    }

    fn labels(entries: &[MenuEntry]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| entry.label().unwrap_or("—").to_string())
            .collect()
    }

    fn items(entry: &MenuEntry) -> &[MenuEntry] {
        match entry {
            MenuEntry::Submenu { items, .. } => items,
            other => panic!("expected a submenu, got {other:?}"),
        }
    }

    fn command_entry(label: &str, command: &str, shortcut: Option<&str>) -> MenuEntry {
        MenuEntry::Command {
            label: label.into(),
            command: command.into(),
            shortcut: shortcut.map(String::from),
            enabled: true,
        }
    }

    fn radio(label: &str, command: &str, checked: bool) -> MenuEntry {
        MenuEntry::Radio {
            label: label.into(),
            command: command.into(),
            checked,
        }
    }

    fn link(title: &str, target: &str) -> Link {
        Link {
            title: title.into(),
            target: target.into(),
        }
    }

    fn group(title: &str, done: u32, total: u32) -> GroupProgress {
        GroupProgress {
            title: title.into(),
            progress: Progress { done, total },
        }
    }

    fn many_links(count: usize) -> Vec<LinkItem> {
        (0..count)
            .map(|i| LinkItem::Link(link(&format!("L{i}"), &format!("https://l/{i}"))))
            .collect()
    }

    fn with_music(playing: bool, entry: Option<&str>) -> ToolState {
        ToolState {
            music: Music {
                entries: vec![
                    MusicItem {
                        id: "focus".into(),
                        name: "Deep Focus".into(),
                    },
                    MusicItem {
                        id: "radio".into(),
                        name: "Radio".into(),
                    },
                ],
                playing,
                volume: Some(48),
                entry: entry.map(String::from),
            },
            ..bare_state()
        }
    }

    const BASELINE: [&str; 4] = ["Open Today's Note", "—", "Settings…", "Quit Daybook"];

    #[test]
    fn tools_are_listed_in_the_configured_order_with_their_shortcut() {
        let menu = menu_of(&bare_state(), &[ToolId::Search, ToolId::Status]);
        assert_eq!(
            labels(&menu)[..3],
            ["Search Vault…", "Status: Working", "Today's Focus…"]
        );
        let status = items(&menu[1]);
        assert_eq!(
            labels(status),
            ["Change Status…", "—", "Working", "Break", "Done for Today"]
        );
        assert_eq!(
            status[0],
            command_entry("Change Status…", "status:toggle", Some("cmd+f1"))
        );
        assert_eq!(status[2], radio("Working", "status:working", true));
        assert_eq!(status[3], radio("Break", "status:break", false));
        assert_eq!(menu[0], command_entry("Search Vault…", "search:open", None));
    }

    #[test]
    fn reversing_the_tools_reverses_the_menu() {
        let menu = menu_of(&bare_state(), &[ToolId::Status, ToolId::Search]);
        assert_eq!(
            labels(&menu)[..3],
            ["Status: Working", "Today's Focus…", "Search Vault…"]
        );
    }

    #[test]
    fn the_status_submenu_names_the_status() {
        for (status, label) in [
            (Some(Status::Working), "Status: Working"),
            (Some(Status::Break), "Status: Break"),
            (Some(Status::Off), "Status: Done for Today"),
            (None, "Status: Not set yet"),
        ] {
            let state = ToolState {
                status,
                ..bare_state()
            };
            assert_eq!(labels(&menu_of(&state, &[ToolId::Status]))[0], label);
        }
    }

    #[test]
    fn the_status_and_the_focus_need_the_status_feature() {
        let state = ToolState {
            features: vec![Feature::Jira],
            ..bare_state()
        };
        assert_eq!(labels(&menu_of(&state, &[ToolId::Status])), BASELINE);
    }

    #[test]
    fn the_hub_opens_daybook_then_the_day_activity_then_timetracker() {
        let menu = menu_with(
            &bare_state(),
            &Hotkeys {
                hub: "cmd+d".into(),
                ..Hotkeys::none()
            },
            &[ToolId::Hub],
        );
        assert_eq!(
            labels(&menu),
            [
                "Open Daybook",
                "Day Activity",
                "Open in TimeTracker",
                "—",
                "Open Today's Note",
                "—",
                "Settings…",
                "Quit Daybook"
            ]
        );
        assert_eq!(
            menu[0],
            command_entry("Open Daybook", "hub:open", Some("cmd+d"))
        );
        assert_eq!(
            menu[1],
            command_entry("Day Activity", "activity:showTab", None)
        );
        assert_eq!(
            menu[2],
            command_entry("Open in TimeTracker", "timetracker:show", None)
        );
    }

    #[test]
    fn the_hub_hides_the_day_activity_without_the_status_feature_or_with_timetracker_missing() {
        for (features, tt, shown) in [
            (vec![Feature::Status], TtState::Available, true),
            (vec![Feature::Jira], TtState::Available, false),
            (vec![Feature::Status], TtState::Missing, false),
        ] {
            let state = ToolState {
                features: features.clone(),
                tt,
                ..bare_state()
            };
            let menu = labels(&menu_of(&state, &[ToolId::Hub]));
            assert_eq!(
                menu.contains(&"Day Activity".to_string()),
                shown,
                "{features:?} {tt:?}"
            );
            assert_eq!(menu[0], "Open Daybook", "{features:?} {tt:?}");
            assert!(
                menu.contains(&"Open in TimeTracker".to_string()),
                "{features:?} {tt:?}"
            );
        }
    }

    #[test]
    fn a_tool_with_choices_becomes_a_submenu_with_its_overlay_first() {
        let state = ToolState {
            journal_groups: vec![group("Dawn", 1, 3)],
            search_scopes: vec!["work".into()],
            ..bare_state()
        };
        let menu = menu_of(
            &state,
            &[ToolId::Journal, ToolId::Search, ToolId::QuickNotes],
        );
        assert_eq!(
            labels(&menu)[..3],
            ["Journal", "Search Vault", "Quick Notes…"]
        );
        assert_eq!(labels(items(&menu[0])), ["Journal…", "—", "Dawn · 1/3"]);
        assert_eq!(labels(items(&menu[1])), ["Search Vault…", "—", "work"]);
        assert_eq!(menu[2], command_entry("Quick Notes…", "notes:quick", None));
        assert_eq!(
            items(&menu[1])[2],
            command_entry("work", "search:scope:work", None)
        );
    }

    #[test]
    fn quick_notes_with_topics_list_them_under_the_overlay() {
        let state = ToolState {
            note_topics: vec![Topic {
                title: "Daily".into(),
                level: 3,
            }],
            ..bare_state()
        };
        let menu = menu_of(&state, &[ToolId::QuickNotes]);
        assert_eq!(labels(items(&menu[0])), ["Quick Notes…", "—", "Daily"]);
        assert_eq!(
            items(&menu[0])[2],
            command_entry("Daily", "notes:topic:3:Daily", None)
        );
    }

    #[test]
    fn separators_at_the_start_the_end_and_in_a_row_are_dropped() {
        let menu = menu_of(
            &bare_state(),
            &[
                ToolId::Separator1,
                ToolId::Search,
                ToolId::Separator1,
                ToolId::Separator2,
                ToolId::QuickNotes,
                ToolId::Separator2,
            ],
        );
        assert_eq!(
            labels(&menu),
            [
                "Search Vault…",
                "—",
                "Quick Notes…",
                "—",
                "Open Today's Note",
                "—",
                "Settings…",
                "Quit Daybook"
            ]
        );
    }

    #[test]
    fn the_activity_tool_adds_the_timer_line_and_log_activity() {
        let state = ToolState {
            links_now: vec![],
            links_more: vec![],
            ..fixture()
        };
        let menu = menu_of(&state, &[ToolId::Activity]);
        assert_eq!(
            labels(&menu),
            [
                "TimeTracker isn't running",
                "Log Activity…",
                "—",
                "Open Today's Note",
                "—",
                "Settings…",
                "Quit Daybook"
            ]
        );
        assert_eq!(
            menu[1],
            command_entry("Log Activity…", "activity:switch", None)
        );
    }

    #[test]
    fn a_meeting_shows_as_a_line_that_tells_since_when() {
        let state = ToolState {
            meeting_since: Some("10:15".into()),
            ..bare_state()
        };
        let menu = menu_of(&state, &[ToolId::Call]);
        assert_eq!(menu[0], MenuEntry::Info("In a meeting since 10:15".into()));
        assert_eq!(labels(&menu_of(&bare_state(), &[ToolId::Call])), BASELINE);
    }

    #[test]
    fn music_appears_as_a_submenu_once_there_is_something_to_play() {
        let without = menu_of(&bare_state(), &[ToolId::Music]);
        assert_eq!(labels(&without), BASELINE);

        let menu = menu_of(&with_music(true, Some("focus")), &[ToolId::Music]);
        assert_eq!(menu[0].label(), Some("Music: Deep Focus"));
        let music = items(&menu[0]);
        assert_eq!(
            labels(music),
            [
                "Pause",
                "—",
                "Deep Focus",
                "Radio",
                "—",
                "Volume: 48 %",
                "Music…"
            ]
        );
        assert_eq!(music[2], radio("Deep Focus", "music:play:focus", true));
        assert_eq!(music[3], radio("Radio", "music:play:radio", false));
        let volume: Vec<&MenuEntry> = items(&music[5])
            .iter()
            .filter(|item| matches!(item, MenuEntry::Radio { checked: true, .. }))
            .collect();
        assert_eq!(volume, [&radio("50 %", "music:volume:50", true)]);
        assert_eq!(
            labels(items(&music[5])),
            ["10 %", "25 %", "50 %", "75 %", "100 %"]
        );
    }

    #[test]
    fn the_music_submenu_names_the_entry_only_while_it_plays() {
        for (playing, entry, label) in [
            (true, Some("focus"), "Music: Deep Focus"),
            (false, Some("focus"), "Music"),
            (true, None, "Music"),
            (true, Some("gone"), "Music"),
        ] {
            let menu = menu_of(&with_music(playing, entry), &[ToolId::Music]);
            assert_eq!(menu[0].label(), Some(label), "{playing} {entry:?}");
        }
    }

    #[test]
    fn the_music_overlay_item_carries_its_shortcut() {
        let hotkeys = Hotkeys {
            music: "cmd+m".into(),
            ..Hotkeys::none()
        };
        let menu = menu_with(&with_music(false, None), &hotkeys, &[ToolId::Music]);
        assert_eq!(
            items(&menu[0]).last(),
            Some(&command_entry("Music…", "music:open", Some("cmd+m")))
        );
    }

    #[test]
    fn the_context_submenu_names_the_current_context_and_ends_with_switching() {
        let hotkeys = Hotkeys {
            switch_context: "cmd+k".into(),
            ..Hotkeys::none()
        };
        let menu = menu_with(&bare_state(), &hotkeys, &[ToolId::Context]);
        assert_eq!(menu[0].label(), Some("Context: [W] Work"));
        let context = items(&menu[0]);
        assert_eq!(
            labels(context),
            ["[W] Work", "Private", "—", "Switch Context…"]
        );
        assert_eq!(context[0], radio("[W] Work", "mode:work", true));
        assert_eq!(context[1], radio("Private", "mode:private", false));
        assert_eq!(
            context[3],
            command_entry("Switch Context…", "context:switch", Some("cmd+k"))
        );
    }

    #[test]
    fn a_single_context_or_none_current_adds_no_context_submenu() {
        let single = ToolState {
            contexts: vec![ContextItem {
                id: "work".into(),
                label: String::new(),
                name: "Work".into(),
            }],
            ..bare_state()
        };
        let no_current = ToolState {
            context: None,
            ..bare_state()
        };
        assert_eq!(labels(&menu_of(&single, &[ToolId::Context])), BASELINE);
        assert_eq!(labels(&menu_of(&no_current, &[ToolId::Context])), BASELINE);
    }

    #[test]
    fn jira_lists_its_tickets_and_needs_the_feature() {
        let menu = menu_of(&fixture(), &[ToolId::Jira]);
        assert_eq!(menu[0].label(), Some("Jira"));
        assert_eq!(labels(items(&menu[0])), ["Open Jira Issue…", "—", "QRS-1"]);
        let no_tickets = ToolState {
            tickets: vec![],
            ..fixture()
        };
        assert_eq!(
            menu_of(&no_tickets, &[ToolId::Jira])[0],
            command_entry("Open Jira Issue…", "jira:open", None)
        );
        let no_feature = ToolState {
            features: vec![Feature::Status],
            links_now: vec![],
            links_more: vec![],
            ..fixture()
        };
        assert_eq!(labels(&menu_of(&no_feature, &[ToolId::Jira])), BASELINE);
    }

    #[test]
    fn projects_list_their_launcher_projects_and_need_linked_projects() {
        let menu = menu_of(&fixture(), &[ToolId::Projects]);
        assert_eq!(menu[0].label(), Some("Projects"));
        assert_eq!(labels(items(&menu[0])), ["Projects…", "—", "QRS"]);
        assert_eq!(
            items(&menu[0])[2],
            command_entry("QRS", "projects:open:p1", None)
        );
        assert_eq!(
            labels(&menu_of(&bare_state(), &[ToolId::Projects])),
            BASELINE
        );
    }

    #[test]
    fn the_journal_label_tells_how_far_the_questions_are() {
        for (done, total, label) in [
            (0, 0, "Journal…"),
            (0, 6, "Journal…"),
            (2, 6, "Journal · 2/6"),
            (6, 6, "Journal · Done"),
        ] {
            let state = ToolState {
                journal: Progress { done, total },
                ..bare_state()
            };
            let menu = menu_of(&state, &[ToolId::Journal]);
            assert_eq!(
                menu[0],
                command_entry(label, "journal:open", None),
                "{done}/{total}"
            );
        }
    }

    #[test]
    fn the_journal_submenu_label_drops_the_ellipsis() {
        for (done, total, label) in [
            (0, 6, "Journal"),
            (2, 6, "Journal · 2/6"),
            (6, 6, "Journal · Done"),
        ] {
            let state = ToolState {
                journal: Progress { done, total },
                journal_groups: vec![group("Dawn", done, total)],
                ..bare_state()
            };
            let menu = menu_of(&state, &[ToolId::Journal]);
            assert_eq!(menu[0].label(), Some(label), "{done}/{total}");
        }
    }

    #[test]
    fn the_journal_open_item_carries_the_shortcut() {
        let hotkeys = Hotkeys {
            journal: "cmd+j".into(),
            ..Hotkeys::none()
        };
        let menu = menu_with(&fixture(), &hotkeys, &[ToolId::Journal]);
        assert_eq!(
            items(&menu[0])[0],
            command_entry("Journal…", "journal:open", Some("cmd+j"))
        );
    }

    #[test]
    fn the_links_tool_opens_the_overlay_and_the_links_follow_below() {
        let menu = menu_of(&fixture(), &[ToolId::Links]);
        assert_eq!(
            labels(&menu),
            [
                "Links…",
                "—",
                "Board",
                "More Links",
                "—",
                "Open Today's Note",
                "—",
                "Settings…",
                "Quit Daybook"
            ]
        );
        assert_eq!(
            menu[2],
            command_entry("Board", "link:https%3A%2F%2Fboard", None)
        );
    }

    #[test]
    fn at_most_ten_links_show_and_the_rest_go_under_more() {
        let ten = ToolState {
            links_now: many_links(10),
            ..bare_state()
        };
        assert_eq!(link_menu(&ten).len(), 10);

        let twelve = ToolState {
            links_now: many_links(12),
            ..bare_state()
        };
        let menu = link_menu(&twelve);
        assert_eq!(menu.len(), 11);
        assert_eq!(menu[10].label(), Some("More…"));
        assert_eq!(labels(items(&menu[10])), ["L10", "L11"]);
        assert_eq!(menu[9].label(), Some("L9"));
    }

    #[test]
    fn the_other_links_sit_under_more_links_after_the_ones_for_now() {
        let state = ToolState {
            links_now: many_links(11),
            links_more: many_links(1),
            ..bare_state()
        };
        let menu = link_menu(&state);
        assert_eq!(labels(&menu[10..]), ["More…", "More Links"]);
        assert_eq!(labels(items(&menu[11])), ["L0"]);
    }

    #[test]
    fn a_projects_links_become_a_submenu_under_its_name() {
        let menu = link_menu(&fixture());
        assert_eq!(
            menu,
            vec![
                command_entry("Board", "link:https%3A%2F%2Fboard", None),
                MenuEntry::Submenu {
                    label: "More Links".into(),
                    items: vec![MenuEntry::Submenu {
                        label: "Oscorp".into(),
                        items: vec![command_entry("Wiki", "link:https%3A%2F%2Fwiki", None)],
                    }],
                },
            ]
        );
    }

    #[test]
    fn no_links_make_an_empty_link_menu() {
        assert_eq!(link_menu(&bare_state()), vec![]);
    }

    #[test]
    fn without_a_vault_the_menu_only_offers_choosing_one() {
        let state = ToolState {
            has_vault: false,
            ..fixture()
        };
        let menu = menu_of(&state, &[ToolId::Search, ToolId::Status]);
        assert_eq!(
            labels(&menu),
            ["Choose Vault…", "—", "Settings…", "Quit Daybook"]
        );
        assert_eq!(
            menu[0],
            command_entry("Choose Vault…", "app:settings", None)
        );
        assert_eq!(
            menu[2],
            command_entry("Settings…", "app:settings", Some("cmd+,"))
        );
        assert_eq!(menu[3], command_entry("Quit Daybook", "app:quit", None));
    }

    #[test]
    fn a_missing_todays_note_can_be_created_from_the_menu() {
        let state = ToolState {
            today: crate::tools::TodayNote::default(),
            ..bare_state()
        };
        let menu = menu_of(&state, &[]);
        assert_eq!(
            labels(&menu),
            [
                "Create Today's Note",
                "—",
                "Open Today's Note",
                "—",
                "Settings…",
                "Quit Daybook"
            ]
        );
        assert_eq!(
            menu[0],
            command_entry("Create Today's Note", "daily:create", None)
        );
    }

    #[test]
    fn creating_todays_note_greys_out_the_create_item() {
        let state = ToolState {
            today: crate::tools::TodayNote {
                exists: false,
                creating: true,
                waiting: 0,
            },
            ..bare_state()
        };
        assert_eq!(
            menu_of(&state, &[])[0],
            MenuEntry::Command {
                label: "Creating Today's Note…".into(),
                command: "daily:create".into(),
                shortcut: None,
                enabled: false,
            }
        );
    }

    #[test]
    fn entries_waiting_for_todays_note_are_counted() {
        for (waiting, line) in [
            (1, "1 entry waiting for today's note"),
            (3, "3 entries waiting for today's note"),
        ] {
            let state = ToolState {
                today: crate::tools::TodayNote {
                    exists: false,
                    creating: false,
                    waiting,
                },
                ..bare_state()
            };
            let menu = menu_of(&state, &[]);
            assert_eq!(menu[1], MenuEntry::Info(line.into()), "{waiting}");
            assert_eq!(menu[2], MenuEntry::Separator);
        }
    }

    fn cmd(id: &str) -> MenuEntry {
        command_entry(&format!("Label {id}"), id, Some("cmd+1"))
    }

    fn stub(unknown: &[&'static str], disabled: &[&'static str]) -> CommandsStub {
        CommandsStub {
            unknown: unknown.to_vec(),
            disabled: disabled.to_vec(),
        }
    }

    #[test]
    fn prune_drops_commands_nobody_knows() {
        let entries = vec![
            cmd("a"),
            cmd("b"),
            radio("R", "r", true),
            radio("S", "s", false),
        ];
        assert_eq!(
            prune(entries, &stub(&["b", "s"], &[])),
            vec![cmd("a"), radio("R", "r", true)]
        );
    }

    #[test]
    fn prune_greys_out_commands_that_cannot_run_now() {
        let pruned = prune(vec![cmd("a"), cmd("b")], &stub(&[], &["b"]));
        assert_eq!(
            pruned,
            vec![
                cmd("a"),
                MenuEntry::Command {
                    label: "Label b".into(),
                    command: "b".into(),
                    shortcut: Some("cmd+1".into()),
                    enabled: false,
                },
            ]
        );
    }

    #[test]
    fn prune_never_enables_a_command_that_was_disabled() {
        let creating = MenuEntry::Command {
            label: "Creating…".into(),
            command: "daily:create".into(),
            shortcut: None,
            enabled: false,
        };
        assert_eq!(
            prune(vec![creating.clone()], &CommandsStub::default()),
            vec![creating]
        );
    }

    #[test]
    fn prune_keeps_info_lines_between_commands() {
        let entries = vec![cmd("a"), MenuEntry::Info("note".into()), cmd("b")];
        assert_eq!(prune(entries.clone(), &CommandsStub::default()), entries);
    }

    #[test]
    fn prune_prunes_inside_submenus() {
        let submenu = |items| MenuEntry::Submenu {
            label: "Sub".into(),
            items,
        };
        let entries = vec![submenu(vec![cmd("a"), cmd("b"), cmd("c")])];
        assert_eq!(
            prune(entries, &stub(&["b"], &["c"])),
            vec![submenu(vec![
                cmd("a"),
                MenuEntry::Command {
                    label: "Label c".into(),
                    command: "c".into(),
                    shortcut: Some("cmd+1".into()),
                    enabled: false,
                },
            ])]
        );
    }

    #[test]
    fn prune_drops_submenus_with_nothing_actionable_left() {
        let submenu = |label: &str, items| MenuEntry::Submenu {
            label: label.into(),
            items,
        };
        let entries = vec![
            cmd("keep"),
            submenu("Unknown", vec![cmd("x"), cmd("y")]),
            submenu(
                "Only text",
                vec![MenuEntry::Info("hi".into()), MenuEntry::Separator],
            ),
            submenu("Empty", vec![]),
            submenu("Nested", vec![submenu("Inner", vec![cmd("x")])]),
        ];
        assert_eq!(prune(entries, &stub(&["x", "y"], &[])), vec![cmd("keep")]);
    }

    #[test]
    fn prune_tidies_the_separators_it_leaves_behind() {
        let entries = vec![
            cmd("x"),
            MenuEntry::Separator,
            cmd("a"),
            MenuEntry::Separator,
            cmd("y"),
            MenuEntry::Separator,
            cmd("b"),
            MenuEntry::Separator,
            cmd("z"),
        ];
        assert_eq!(
            prune(entries, &stub(&["x", "y", "z"], &[])),
            vec![cmd("a"), MenuEntry::Separator, cmd("b")]
        );
    }

    #[test]
    fn a_whole_menu_loses_what_the_commands_do_not_know() {
        let menu = menu_of(&bare_state(), &[ToolId::Hub]);
        let pruned = prune(menu, &stub(&["daily:open", "app:settings"], &[]));
        assert_eq!(
            labels(&pruned),
            [
                "Open Daybook",
                "Day Activity",
                "Open in TimeTracker",
                "—",
                "Quit Daybook"
            ]
        );
    }
}
