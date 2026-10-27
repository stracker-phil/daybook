//! Global shortcuts (Settings → Shortcuts): system-wide keys that run commands, and the pause while
//! Settings records a new one.

mod accelerator;
pub mod overview;

pub use accelerator::accelerator;

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::state::AppState;
use crate::store::settings::Hotkeys;
use crate::{bus, menu};

/// The debug build registers none, so the installed app keeps its keys (ADR 0030).
pub const REGISTERED: bool = !cfg!(debug_assertions);

pub struct HotkeyAction {
    /// Its key in `Settings.hotkeys`, as stored.
    pub action: &'static str,
    pub command: &'static str,
    pub shortcut: fn(&Hotkeys) -> &String,
}

const fn action(
    action: &'static str,
    command: &'static str,
    shortcut: fn(&Hotkeys) -> &String,
) -> HotkeyAction {
    HotkeyAction {
        action,
        command,
        shortcut,
    }
}

/// Every global shortcut and the command it runs, in the Shortcuts page's order.
pub const HOTKEY_ACTIONS: [HotkeyAction; 14] = [
    action("palette", "palette:open", |h| &h.palette),
    action("switchActivity", "activity:switch", |h| &h.switch_activity),
    action("hub", "hub:toggle", |h| &h.hub),
    action("switchContext", "context:switch", |h| &h.switch_context),
    action("toggleStatus", "status:toggle", |h| &h.toggle_status),
    action("focus", "focus:open", |h| &h.focus),
    action("jiraOpen", "jira:open", |h| &h.jira_open),
    action("journal", "journal:open", |h| &h.journal),
    action("search", "search:open", |h| &h.search),
    action("quickNotes", "notes:quick", |h| &h.quick_notes),
    action("links", "links:open", |h| &h.links),
    action("projects", "projects:open", |h| &h.projects),
    action("music", "music:open", |h| &h.music),
    action("toggleRail", "rail:toggle", |h| &h.toggle_rail),
];

pub fn hotkey_commands(hotkeys: &Hotkeys) -> [(&str, &'static str); 14] {
    HOTKEY_ACTIONS.map(|hotkey| ((hotkey.shortcut)(hotkeys).as_str(), hotkey.command))
}

/// Paused while a shortcut is being recorded, so its keys reach the page.
#[derive(Default)]
pub struct Pause(AtomicBool);

pub fn paused(app: &AppHandle) -> bool {
    app.state::<Pause>().0.load(Ordering::SeqCst)
}

/// What the menus show next to their items: the shortcuts that work.
pub fn shown(app: &AppHandle) -> Hotkeys {
    if REGISTERED {
        app.state::<AppState>().settings.get().hotkeys
    } else {
        Hotkeys::none()
    }
}

/// Registers the shortcuts from Settings whose commands exist, replacing the ones before.
pub fn apply(app: &AppHandle) {
    let shortcuts = app.global_shortcut();
    if let Err(e) = shortcuts.unregister_all() {
        log::warn!("Couldn't remove the global shortcuts: {e}");
    }
    if !REGISTERED || paused(app) {
        return;
    }
    let hotkeys = app.state::<AppState>().settings.get().hotkeys;
    let commands = app.state::<bus::Commands>();
    for (shortcut, command) in hotkey_commands(&hotkeys) {
        let Some(accelerator) = accelerator(shortcut) else {
            continue;
        };
        if !commands.knows(command) {
            continue;
        }
        let registered = shortcuts.on_shortcut(accelerator.as_str(), move |app, _, event| {
            if event.state == ShortcutState::Pressed {
                bus::spawn(app, command);
            }
        });
        if let Err(e) = registered {
            log::warn!("Couldn't register {accelerator} for {command}: {e}");
        }
    }
}

/// Settings records a shortcut: global shortcuts and the menu's key equivalents step aside meanwhile.
#[tauri::command]
pub fn shortcuts_pause(app: AppHandle, paused: bool) {
    if app.state::<Pause>().0.swap(paused, Ordering::SeqCst) == paused {
        return;
    }
    apply(&app);
    menu::rebuild(&app);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn distinct_hotkeys() -> Hotkeys {
        Hotkeys {
            switch_activity: "1".into(),
            toggle_status: "2".into(),
            jira_open: "3".into(),
            journal: "4".into(),
            search: "5".into(),
            toggle_rail: "6".into(),
            hub: "7".into(),
            links: "8".into(),
            quick_notes: "9".into(),
            switch_context: "10".into(),
            focus: "11".into(),
            music: "12".into(),
            palette: "13".into(),
            projects: "14".into(),
        }
    }

    #[test]
    fn each_hotkey_action_reads_the_field_it_is_named_after() {
        let hotkeys = distinct_hotkeys();
        let json = serde_json::to_value(&hotkeys).unwrap();
        for entry in &HOTKEY_ACTIONS {
            assert_eq!(
                json[entry.action].as_str(),
                Some((entry.shortcut)(&hotkeys).as_str()),
                "{}",
                entry.action
            );
        }
    }

    #[test]
    fn hotkey_actions_and_commands_are_unique_and_cover_every_field() {
        let json = serde_json::to_value(distinct_hotkeys()).unwrap();
        let mut actions: Vec<&str> = HOTKEY_ACTIONS.iter().map(|entry| entry.action).collect();
        let mut commands: Vec<&str> = HOTKEY_ACTIONS.iter().map(|entry| entry.command).collect();
        let mut fields: Vec<&str> = json
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        actions.sort_unstable();
        commands.sort_unstable();
        fields.sort_unstable();
        let (action_count, command_count) = (actions.len(), commands.len());
        actions.dedup();
        commands.dedup();
        assert_eq!(actions.len(), action_count);
        assert_eq!(commands.len(), command_count);
        assert_eq!(actions, fields);
    }

    #[test]
    fn each_hotkey_runs_its_command() {
        let hotkeys = Hotkeys {
            switch_activity: "1".into(),
            toggle_status: "2".into(),
            jira_open: "3".into(),
            journal: "4".into(),
            search: "5".into(),
            toggle_rail: "6".into(),
            hub: "7".into(),
            links: "8".into(),
            quick_notes: "9".into(),
            switch_context: "10".into(),
            focus: "11".into(),
            music: "12".into(),
            palette: "13".into(),
            projects: "14".into(),
        };
        let mut pairs = hotkey_commands(&hotkeys).to_vec();
        pairs.sort_by_key(|(key, _)| key.parse::<u32>().unwrap());
        assert_eq!(
            pairs,
            [
                ("1", "activity:switch"),
                ("2", "status:toggle"),
                ("3", "jira:open"),
                ("4", "journal:open"),
                ("5", "search:open"),
                ("6", "rail:toggle"),
                ("7", "hub:toggle"),
                ("8", "links:open"),
                ("9", "notes:quick"),
                ("10", "context:switch"),
                ("11", "focus:open"),
                ("12", "music:open"),
                ("13", "palette:open"),
                ("14", "projects:open"),
            ]
        );
    }
}
