//! What Settings → Shortcuts lists: the global shortcuts of the commands that exist, and the fixed keys
//! of the app menu.

use serde::Serialize;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use super::{HOTKEY_ACTIONS, REGISTERED};
use crate::bus::{self, CommandBus, Host};
use crate::menu::command_shortcuts::MENU_KEYS;

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct GlobalShortcut {
    /// Its key in `Settings.hotkeys`.
    pub action: String,
    pub command: String,
    pub title: String,
    pub help: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct MenuShortcut {
    pub command: String,
    pub title: String,
    /// Settings notation (`cmd+k`).
    pub shortcut: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ShortcutOverview {
    /// Whether this build registers global shortcuts (ADR 0018).
    pub registered: bool,
    pub global: Vec<GlobalShortcut>,
    pub menu: Vec<MenuShortcut>,
}

/// The shortcuts of the commands `bus` knows.
pub fn overview<H: Host>(bus: &CommandBus<H>, registered: bool) -> ShortcutOverview {
    ShortcutOverview {
        registered,
        global: HOTKEY_ACTIONS
            .iter()
            .filter_map(|hotkey| {
                let info = bus.info(hotkey.command)?;
                Some(GlobalShortcut {
                    action: hotkey.action.to_string(),
                    command: hotkey.command.to_string(),
                    title: info.title.to_string(),
                    help: info.help.to_string(),
                })
            })
            .collect(),
        menu: MENU_KEYS
            .iter()
            .filter_map(|key| {
                Some(MenuShortcut {
                    command: key.command.to_string(),
                    title: bus.info(key.command)?.title.to_string(),
                    shortcut: key.shortcut.to_string(),
                })
            })
            .collect(),
    }
}

#[tauri::command]
pub fn shortcut_overview(app: AppHandle) -> ShortcutOverview {
    let commands = app.state::<bus::Commands>();
    overview(&commands, REGISTERED)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bus::Command;
    use crate::error::AppError;

    #[derive(Clone)]
    struct TestHost;

    impl Host for TestHost {
        fn has_vault(&self) -> bool {
            true
        }

        fn choose_vault(&self) {}

        fn report_failure(&self, _id: &str, _error: &AppError) {}
    }

    fn bus_with(commands: &[(&'static str, &'static str, &'static str)]) -> CommandBus<TestHost> {
        let mut bus = CommandBus::default();
        for &(id, title, help) in commands {
            bus.add(Command::new(id, title).help(help).run_now(|_| {}));
        }
        bus
    }

    #[test]
    fn global_shortcuts_list_the_existing_commands_in_the_pages_order() {
        let bus = bus_with(&[
            ("search:open", "Search vault", "Searches the vault."),
            ("palette:open", "Command palette", "Opens the palette."),
            ("app:settings", "Settings", "Opens Settings."),
        ]);

        let global = overview(&bus, true).global;

        assert_eq!(
            global,
            [
                GlobalShortcut {
                    action: "palette".into(),
                    command: "palette:open".into(),
                    title: "Command palette".into(),
                    help: "Opens the palette.".into(),
                },
                GlobalShortcut {
                    action: "search".into(),
                    command: "search:open".into(),
                    title: "Search vault".into(),
                    help: "Searches the vault.".into(),
                },
            ]
        );
    }

    #[test]
    fn menu_shortcuts_list_the_existing_commands_with_their_keys() {
        let bus = bus_with(&[
            ("window:close", "Close window", ""),
            ("app:settings", "Settings", ""),
            ("search:open", "Search vault", ""),
        ]);

        let menu = overview(&bus, true).menu;

        let row = |command: &str, title: &str, shortcut: &str| MenuShortcut {
            command: command.into(),
            title: title.into(),
            shortcut: shortcut.into(),
        };
        assert_eq!(
            menu,
            [
                row("app:settings", "Settings", "cmd+,"),
                row("window:close", "Close window", "cmd+q"),
                row("search:open", "Search vault", "cmd+f"),
                row("window:close", "Close window", "cmd+w"),
            ]
        );
    }

    #[test]
    fn an_empty_bus_lists_nothing_and_registered_is_passed_through() {
        let bus = CommandBus::<TestHost>::default();
        for registered in [true, false] {
            assert_eq!(
                overview(&bus, registered),
                ShortcutOverview {
                    registered,
                    global: Vec::new(),
                    menu: Vec::new(),
                },
                "{registered}"
            );
        }
    }
}
