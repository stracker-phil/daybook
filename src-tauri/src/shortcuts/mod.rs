//! Global shortcuts (Settings → Shortcuts): system-wide keys that run commands, and the pause while
//! Settings records a new one.

mod accelerator;

pub use accelerator::accelerator;

use std::sync::atomic::{AtomicBool, Ordering};

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};

use crate::state::AppState;
use crate::store::settings::Hotkeys;
use crate::{bus, menu};

/// Release builds register none (ADR 0018).
pub const REGISTERED: bool = cfg!(debug_assertions);

/// The command each global shortcut runs.
pub fn hotkey_commands(hotkeys: &Hotkeys) -> [(&str, &'static str); 14] {
    [
        (&hotkeys.switch_activity, "activity:switch"),
        (&hotkeys.toggle_status, "status:toggle"),
        (&hotkeys.jira_open, "jira:open"),
        (&hotkeys.journal, "journal:open"),
        (&hotkeys.search, "search:open"),
        (&hotkeys.toggle_rail, "rail:toggle"),
        (&hotkeys.hub, "hub:toggle"),
        (&hotkeys.links, "links:open"),
        (&hotkeys.quick_notes, "notes:quick"),
        (&hotkeys.switch_context, "context:switch"),
        (&hotkeys.focus, "focus:open"),
        (&hotkeys.music, "music:open"),
        (&hotkeys.palette, "palette:open"),
        (&hotkeys.projects, "projects:open"),
    ]
    .map(|(shortcut, command)| (shortcut.as_str(), command))
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
        assert_eq!(
            hotkey_commands(&hotkeys),
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
