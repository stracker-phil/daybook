//! What the palette offers: everything the menu-bar menu can do, flattened, plus a few direct commands.
//! Pure.

use std::collections::HashSet;

use serde::Serialize;
use ts_rs::TS;

use crate::bus::CommandStatus;
use crate::menu::model::{MenuEntry, MenuInput, build_menu, prune};
use crate::store::settings::{Hotkeys, default_tools};
use crate::tools::ToolState;
use crate::window::settings::{PAGES, SECTIONS};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum PaletteKind {
    Command,
    Link,
    Project,
    Music,
    Context,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct PaletteEntry {
    pub kind: PaletteKind,
    pub label: String,
    /// Dimmed after the label: where a link belongs, or "Current".
    pub detail: String,
    /// A global shortcut in Settings notation.
    pub shortcut: Option<String>,
    pub command: String,
}

/// In the menu but not here: the palette has the statuses themselves and can't open itself.
const LEFT_OUT: [&str; 2] = ["status:toggle", "palette:open"];

/// Submenus that only hold overflow say nothing about the item.
const OVERFLOW: [&str; 2] = ["More Links", "More…"];

struct Flat {
    label: String,
    command: String,
    /// The submenus the item sits in, outermost first.
    path: Vec<String>,
    shortcut: Option<String>,
    checked: bool,
}

/// `Context: Work` → `Context`; the part after the colon is the submenu's current value.
fn group_name(label: &str) -> String {
    label
        .split([':', '·'])
        .next()
        .unwrap_or(label)
        .trim()
        .to_string()
}

fn flatten(entries: Vec<MenuEntry>, path: &[String]) -> Vec<Flat> {
    entries
        .into_iter()
        .flat_map(|entry| match entry {
            MenuEntry::Submenu { label, items } => {
                let mut inner = path.to_vec();
                inner.push(group_name(&label));
                flatten(items, &inner)
            }
            MenuEntry::Command {
                label,
                command,
                shortcut,
                enabled: true,
            } => vec![Flat {
                label,
                command,
                path: path.to_vec(),
                shortcut,
                checked: false,
            }],
            MenuEntry::Radio {
                label,
                command,
                checked,
            }
            | MenuEntry::Checkbox {
                label,
                command,
                checked,
            } => vec![Flat {
                label,
                command,
                path: path.to_vec(),
                shortcut: None,
                checked,
            }],
            _ => Vec::new(),
        })
        .collect()
}

fn kind_of(command: &str) -> PaletteKind {
    match command {
        c if c.starts_with("link:") => PaletteKind::Link,
        c if c.starts_with("projects:open:") => PaletteKind::Project,
        c if c.starts_with("music:play:") => PaletteKind::Music,
        c if c.starts_with("mode:") => PaletteKind::Context,
        _ => PaletteKind::Command,
    }
}

/// An overflow submenu says nothing about the item, and a submenu the label already names would repeat
/// it ("Journal…" inside the Journal submenu, not "Journal: Journal…").
fn is_redundant_part(part: &str, label: &str) -> bool {
    OVERFLOW.contains(&part) || label.contains(part)
}

/// A command reads as its menu path: "Music: Pause".
fn command_entry(item: Flat, path: Vec<String>) -> PaletteEntry {
    let mut label = path;
    label.push(item.label);
    PaletteEntry {
        kind: PaletteKind::Command,
        label: label.join(": "),
        detail: if item.checked {
            "Current".into()
        } else {
            String::new()
        },
        shortcut: item.shortcut,
        command: item.command,
    }
}

/// A link, project, playlist or context is named by itself; a link's group goes into the detail.
fn named_entry(item: Flat, kind: PaletteKind, path: Vec<String>) -> PaletteEntry {
    let mut detail = if kind == PaletteKind::Link {
        path
    } else {
        Vec::new()
    };
    if item.checked {
        detail.push("Current".into());
    }
    PaletteEntry {
        kind,
        label: item.label,
        detail: detail.join(" · "),
        shortcut: None,
        command: item.command,
    }
}

fn to_entry(mut item: Flat) -> PaletteEntry {
    let kind = kind_of(&item.command);
    let path: Vec<String> = std::mem::take(&mut item.path)
        .into_iter()
        .filter(|part| !is_redundant_part(part, &item.label))
        .collect();
    match kind {
        PaletteKind::Command => command_entry(item, path),
        _ => named_entry(item, kind, path),
    }
}

fn direct(state: &ToolState, hotkeys: &Hotkeys) -> Vec<PaletteEntry> {
    let command = |label: &str, id: &str, shortcut: &str| PaletteEntry {
        kind: PaletteKind::Command,
        label: label.into(),
        detail: String::new(),
        shortcut: (!shortcut.is_empty()).then(|| shortcut.to_string()),
        command: id.into(),
    };
    let mut entries = vec![command(
        "Show or Hide the Rail",
        "rail:toggle",
        &hotkeys.toggle_rail,
    )];
    if state.meeting_detection {
        entries.push(command("Check for a Call Now", "meetings:check", ""));
    }
    entries.push(command("Install Command Line Tool", "cli:install", ""));
    entries.push(command("Open Log Folder", "app:logs", ""));
    let sections = SECTIONS.iter().map(|&(id, _, title)| (id, title));
    for (page, title) in PAGES.into_iter().chain(sections) {
        entries.push(command(
            &format!("Settings: {title}"),
            &format!("settings:{page}"),
            "",
        ));
    }
    if cfg!(debug_assertions) {
        entries.push(command("Sample Overlay", "sample:open", ""));
        entries.push(command("Sample Overlay (Sticky)", "sample:sticky", ""));
    }
    entries
}

fn runnable(commands: &dyn CommandStatus, id: &str) -> bool {
    commands.knows(id) && commands.is_enabled(id)
}

/// Every tool's commands, wherever Settings → General → Tools places the tool, each once and only those
/// that can run now.
pub fn palette_entries(
    state: &ToolState,
    hotkeys: &Hotkeys,
    commands: &dyn CommandStatus,
) -> Vec<PaletteEntry> {
    if !state.has_vault {
        return Vec::new();
    }
    let tools = default_tools();
    let menu = build_menu(&MenuInput {
        state,
        hotkeys,
        tools: &tools,
    });
    let all = flatten(prune(menu, commands), &[])
        .into_iter()
        .map(to_entry)
        .chain(
            direct(state, hotkeys)
                .into_iter()
                .filter(|entry| runnable(commands, &entry.command)),
        )
        .filter(|entry| !LEFT_OUT.contains(&entry.command.as_str()));
    let mut seen = HashSet::new();
    let mut entries = Vec::new();
    for entry in all {
        if seen.insert(entry.command.clone()) {
            entries.push(entry);
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::model::CommandsStub;
    use crate::store::settings::Feature;
    use crate::tools::{Link, LinkItem, Status, bare_state, fixture};

    fn hotkeys() -> Hotkeys {
        Hotkeys {
            journal: "cmd+j".into(),
            toggle_rail: "cmd+r".into(),
            ..Hotkeys::none()
        }
    }

    fn list(state: &ToolState) -> Vec<PaletteEntry> {
        palette_entries(state, &hotkeys(), &CommandsStub::default())
    }

    fn find<'a>(entries: &'a [PaletteEntry], command: &str) -> Option<&'a PaletteEntry> {
        entries.iter().find(|entry| entry.command == command)
    }

    fn entry(
        kind: PaletteKind,
        label: &str,
        detail: &str,
        shortcut: Option<&str>,
        command: &str,
    ) -> PaletteEntry {
        PaletteEntry {
            kind,
            label: label.into(),
            detail: detail.into(),
            shortcut: shortcut.map(String::from),
            command: command.into(),
        }
    }

    #[test]
    fn commands_are_listed_with_their_menu_path_and_shortcut() {
        let entries = list(&fixture());
        let label = |command: &str| find(&entries, command).map(|e| e.label.as_str());
        assert_eq!(label("music:toggle"), Some("Music: Pause"));
        assert_eq!(label("music:open"), Some("Music…"));
        assert_eq!(label("journal:open"), Some("Journal…"));
        assert_eq!(label("journal:open:Dawn"), Some("Journal: Dawn · 1/3"));
        assert_eq!(
            label("journal:open:Health"),
            Some("Journal: Health")
        );
        assert_eq!(label("routine:wrap"), Some("Journal: Wrap-up · 1/3"));
        assert_eq!(label("search:scope:work"), Some("Search Vault: work"));
        assert_eq!(label("jira:issue:QRS-1"), Some("Jira: QRS-1"));
        assert_eq!(label("notes:topic:3:Daily"), Some("Quick Notes: Daily"));
        assert_eq!(label("context:switch"), Some("Switch Context…"));
        assert_eq!(label("music:volume:50"), Some("Music: Volume: 50 %"));
        assert_eq!(
            find(&entries, "journal:open"),
            Some(&entry(
                PaletteKind::Command,
                "Journal…",
                "",
                Some("cmd+j"),
                "journal:open"
            ))
        );
    }

    #[test]
    fn statuses_are_commands_and_the_current_one_says_so() {
        let entries = list(&fixture());
        assert_eq!(
            find(&entries, "status:break"),
            Some(&entry(
                PaletteKind::Command,
                "Status: Break",
                "",
                None,
                "status:break"
            ))
        );
        assert_eq!(
            find(&entries, "status:working").map(|e| e.detail.as_str()),
            Some("Current")
        );
        let on_break = ToolState {
            status: Some(Status::Break),
            ..fixture()
        };
        let entries = list(&on_break);
        assert_eq!(
            find(&entries, "status:break").map(|e| e.detail.as_str()),
            Some("Current")
        );
        assert_eq!(
            find(&entries, "status:working").map(|e| e.detail.as_str()),
            Some("")
        );
    }

    #[test]
    fn links_music_projects_and_contexts_are_named_by_themselves() {
        let entries = list(&fixture());
        let wiki = "link:https%3A%2F%2Fwiki";
        assert_eq!(
            find(&entries, wiki),
            Some(&entry(PaletteKind::Link, "Wiki", "Oscorp", None, wiki))
        );
        let board = "link:https%3A%2F%2Fboard";
        assert_eq!(
            find(&entries, board),
            Some(&entry(PaletteKind::Link, "Board", "", None, board))
        );
        assert_eq!(
            find(&entries, "music:play:cell"),
            Some(&entry(
                PaletteKind::Music,
                "Cell",
                "Current",
                None,
                "music:play:cell"
            ))
        );
        assert_eq!(
            find(&entries, "projects:open:p1"),
            Some(&entry(
                PaletteKind::Project,
                "QRS",
                "",
                None,
                "projects:open:p1"
            ))
        );
        assert_eq!(
            find(&entries, "mode:work"),
            Some(&entry(
                PaletteKind::Context,
                "[W] Work",
                "Current",
                None,
                "mode:work"
            ))
        );
        assert_eq!(
            find(&entries, "mode:private"),
            Some(&entry(
                PaletteKind::Context,
                "Private",
                "",
                None,
                "mode:private"
            ))
        );
    }

    #[test]
    fn links_in_the_overflow_submenu_carry_no_group_detail() {
        let state = ToolState {
            links_now: (0..11)
                .map(|i| {
                    LinkItem::Link(Link {
                        title: format!("L{i}"),
                        target: format!("https://l/{i}"),
                    })
                })
                .collect(),
            ..bare_state()
        };
        let entries = list(&state);
        let last = find(&entries, "link:https%3A%2F%2Fl%2F10");
        assert_eq!(
            last,
            Some(&entry(
                PaletteKind::Link,
                "L10",
                "",
                None,
                "link:https%3A%2F%2Fl%2F10"
            ))
        );
    }

    #[test]
    fn every_command_is_listed_once() {
        let entries = list(&fixture());
        let commands: std::collections::HashSet<&str> =
            entries.iter().map(|e| e.command.as_str()).collect();
        assert_eq!(commands.len(), entries.len());
    }

    #[test]
    fn nothing_is_offered_without_a_vault() {
        let state = ToolState {
            has_vault: false,
            ..fixture()
        };
        assert_eq!(list(&state), vec![]);
    }

    #[test]
    fn the_status_toggle_and_the_palette_itself_are_left_out() {
        let entries = list(&fixture());
        assert_eq!(find(&entries, "status:toggle"), None);
        assert_eq!(find(&entries, "palette:open"), None);
    }

    #[test]
    fn the_activity_commands_are_listed_and_day_recap_is_gone() {
        let state = ToolState {
            check_in_minutes: 15,
            check_ins_paused: true,
            ..fixture()
        };
        let entries = list(&state);
        for entry in &entries {
            assert!(!entry.label.contains("Day Recap"), "{entry:?}");
        }
        assert_eq!(
            find(&entries, "activity:switch"),
            Some(&entry(
                PaletteKind::Command,
                "Log Activity…",
                "",
                None,
                "activity:switch"
            ))
        );
        assert_eq!(
            find(&entries, "checkin:pause"),
            Some(&entry(
                PaletteKind::Command,
                "Pause Check-ins Today",
                "Current",
                None,
                "checkin:pause"
            ))
        );
        assert_eq!(find(&list(&fixture()), "checkin:pause"), None);
        assert_eq!(
            find(&entries, "hub:open").map(|e| e.label.as_str()),
            Some("Open Daybook")
        );
    }

    #[test]
    fn the_status_entries_and_the_focus_need_the_status_feature() {
        let state = ToolState {
            features: vec![Feature::Jira],
            ..fixture()
        };
        let entries = list(&state);
        for command in ["status:working", "status:break", "status:off", "focus:open"] {
            assert_eq!(find(&entries, command), None, "{command}");
        }
        let with = list(&fixture());
        assert!(find(&with, "focus:open").is_some());
    }

    #[test]
    fn the_direct_commands_come_with_their_shortcut() {
        let entries = list(&fixture());
        assert_eq!(
            find(&entries, "rail:toggle"),
            Some(&entry(
                PaletteKind::Command,
                "Show or Hide the Rail",
                "",
                Some("cmd+r"),
                "rail:toggle"
            ))
        );
        assert_eq!(
            find(&entries, "cli:install").map(|e| e.label.as_str()),
            Some("Install Command Line Tool")
        );
        assert_eq!(
            find(&entries, "app:logs").map(|e| e.label.as_str()),
            Some("Open Log Folder")
        );
    }

    #[test]
    fn every_settings_page_has_a_direct_entry() {
        let entries = list(&fixture());
        assert_eq!(
            find(&entries, "settings:vault"),
            Some(&entry(
                PaletteKind::Command,
                "Settings: Vault",
                "",
                None,
                "settings:vault"
            ))
        );
        for (page, title) in PAGES {
            let command = format!("settings:{page}");
            assert_eq!(
                find(&entries, &command).map(|e| e.label.clone()),
                Some(format!("Settings: {title}")),
                "{page}"
            );
        }
    }

    #[test]
    fn every_settings_section_has_a_direct_entry() {
        let entries = list(&fixture());
        assert_eq!(
            find(&entries, "settings:calendar"),
            Some(&entry(
                PaletteKind::Command,
                "Settings: Calendar",
                "",
                None,
                "settings:calendar"
            ))
        );
        for (id, _, title) in SECTIONS {
            let command = format!("settings:{id}");
            assert_eq!(
                find(&entries, &command).map(|e| e.label.clone()),
                Some(format!("Settings: {title}")),
                "{id}"
            );
        }
    }

    #[test]
    fn settings_pages_nobody_registered_are_absent() {
        let commands = CommandsStub {
            unknown: vec!["settings:vault"],
            ..CommandsStub::default()
        };
        let entries = palette_entries(&fixture(), &hotkeys(), &commands);
        assert_eq!(find(&entries, "settings:vault"), None);
        assert!(find(&entries, "settings:tools").is_some());
    }

    #[test]
    fn the_sample_overlays_are_a_debug_build_thing() {
        let entries = list(&fixture());
        for command in ["sample:open", "sample:sticky"] {
            assert_eq!(
                find(&entries, command).is_some(),
                cfg!(debug_assertions),
                "{command}"
            );
        }
    }

    #[test]
    fn checking_for_a_call_needs_meeting_detection() {
        let on = list(&fixture());
        assert_eq!(
            find(&on, "meetings:check").map(|e| e.label.as_str()),
            Some("Check for a Call Now")
        );
        let off = ToolState {
            meeting_detection: false,
            ..fixture()
        };
        assert_eq!(find(&list(&off), "meetings:check"), None);
    }

    #[test]
    fn commands_that_cannot_run_now_are_absent() {
        let commands = CommandsStub {
            disabled: vec!["journal:open", "rail:toggle", "search:open"],
            ..CommandsStub::default()
        };
        let entries = palette_entries(&fixture(), &hotkeys(), &commands);
        for command in ["journal:open", "rail:toggle", "search:open"] {
            assert_eq!(find(&entries, command), None, "{command}");
        }
        assert!(find(&entries, "journal:open:Dawn").is_some());
        assert!(find(&entries, "cli:install").is_some());
    }

    #[test]
    fn commands_nobody_registered_are_absent() {
        let commands = CommandsStub {
            unknown: vec!["cli:install", "app:logs", "status:break", "hub:open"],
            ..CommandsStub::default()
        };
        let entries = palette_entries(&fixture(), &hotkeys(), &commands);
        for command in ["cli:install", "app:logs", "status:break", "hub:open"] {
            assert_eq!(find(&entries, command), None, "{command}");
        }
        assert!(find(&entries, "status:off").is_some());
    }

    #[test]
    fn every_tool_counts_wherever_settings_show_it() {
        let entries = list(&fixture());
        for command in [
            "hub:open",
            "search:open",
            "links:open",
            "projects:open",
            "notes:quick",
        ] {
            assert!(find(&entries, command).is_some(), "{command}");
        }
    }
}
