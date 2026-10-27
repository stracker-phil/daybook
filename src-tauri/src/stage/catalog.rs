//! The bus's commands and the global shortcuts as `describe` lists them: each command with its key
//! (the menu's, else its global shortcut, in canonical notation) and whether it can run; families with
//! their argument and up to 30 choices. Pure over any bus host.

use serde::Serialize;

use super::keys::notation;
use crate::bus::{CommandBus, Family, Host, Listed};
use crate::menu::command_shortcuts::COMMAND_SHORTCUTS;
use crate::shortcuts::HOTKEY_ACTIONS;
use crate::store::settings::Hotkeys;

const MAX_CHOICES: usize = 30;

/// Whether a command can run for a presenter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandStatus {
    Unknown,
    NeedsVault,
    Disabled,
    Ready,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Choice {
    pub id: String,
    pub title: String,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct StageCommand {
    pub id: String,
    pub help: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut: Option<String>,
    pub enabled: bool,
    /// A family: `id` + an argument runs it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arg: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub choices: Option<Vec<Choice>>,
}

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct StageHotkey {
    /// Its key in `Settings.hotkeys`.
    pub action: &'static str,
    pub shortcut: String,
    pub command: &'static str,
}

pub fn command_status<H: Host>(bus: &CommandBus<H>, host: &H, id: &str) -> CommandStatus {
    let Some(info) = bus.info(id) else {
        return CommandStatus::Unknown;
    };
    if info.needs_vault && !host.has_vault() {
        CommandStatus::NeedsVault
    } else if !bus.is_enabled(host, id) {
        CommandStatus::Disabled
    } else {
        CommandStatus::Ready
    }
}

/// The menu key equivalent, else the set global shortcut that runs the command.
pub fn shortcut_of(command: &str, hotkeys: &Hotkeys) -> Option<String> {
    if let Some(key) = COMMAND_SHORTCUTS.iter().find(|key| key.command == command) {
        return Some(notation(key.shortcut));
    }
    HOTKEY_ACTIONS
        .iter()
        .filter(|hotkey| hotkey.command == command)
        .map(|hotkey| (hotkey.shortcut)(hotkeys))
        .find(|shortcut| !shortcut.is_empty())
        .map(|shortcut| notation(shortcut))
}

/// Every command that isn't hidden, in registration order.
pub fn stage_commands<H: Host>(
    bus: &CommandBus<H>,
    host: &H,
    hotkeys: &Hotkeys,
) -> Vec<StageCommand> {
    bus.list()
        .filter(|listed| !listed.info().hidden)
        .map(|listed| {
            let id = listed.id();
            let mut command = StageCommand {
                id: id.to_string(),
                help: listed.info().help.to_string(),
                shortcut: shortcut_of(id, hotkeys),
                enabled: command_status(bus, host, id) == CommandStatus::Ready,
                arg: None,
                choices: None,
            };
            if let Listed::Family(family) = listed {
                command.arg = Some(family.arg.to_string());
                command.choices = Some(family_choices(family, id, host));
            }
            command
        })
        .collect()
}

/// A family's first 30 choices as full command ids.
fn family_choices<H: Host>(family: &Family<H>, prefix: &str, host: &H) -> Vec<Choice> {
    family
        .choices(host)
        .unwrap_or_default()
        .into_iter()
        .take(MAX_CHOICES)
        .map(|choice| Choice {
            id: format!("{prefix}{}", choice.argument),
            title: choice.title.unwrap_or(choice.word),
        })
        .collect()
}

/// The global shortcuts that are set, in the Shortcuts page's order.
pub fn stage_hotkeys(hotkeys: &Hotkeys) -> Vec<StageHotkey> {
    HOTKEY_ACTIONS
        .iter()
        .filter_map(|hotkey| {
            let shortcut = (hotkey.shortcut)(hotkeys);
            (!shortcut.is_empty()).then(|| StageHotkey {
                action: hotkey.action,
                shortcut: notation(shortcut),
                command: hotkey.command,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    use serde_json::json;

    use super::*;
    use crate::bus::{ArgChoice, Command};
    use crate::error::AppError;

    #[derive(Clone)]
    struct TestHost {
        vault: bool,
        enabled: Arc<AtomicBool>,
    }

    impl TestHost {
        fn new(vault: bool) -> Self {
            Self {
                vault,
                enabled: Arc::new(AtomicBool::new(true)),
            }
        }
    }

    impl Host for TestHost {
        fn has_vault(&self) -> bool {
            self.vault
        }

        fn choose_vault(&self) {}

        fn report_failure(&self, _id: &str, _error: &AppError) {}
    }

    fn bus_with(commands: Vec<Command<TestHost>>) -> CommandBus<TestHost> {
        let mut bus = CommandBus::default();
        for command in commands {
            bus.add(command);
        }
        bus
    }

    fn plain(id: &'static str, help: &'static str) -> Command<TestHost> {
        Command::new(id, help).run_now(|_| {})
    }

    fn family_with(choices: Vec<ArgChoice>) -> CommandBus<TestHost> {
        let mut bus = CommandBus::default();
        bus.add_family(
            Command::new("context:set:", "Context")
                .help("Switches the context.")
                .family("<context>")
                .choices(move |_| choices.clone())
                .run(|_, _| async { Ok(()) }),
        );
        bus
    }

    fn listed(bus: &CommandBus<TestHost>, host: &TestHost, hotkeys: &Hotkeys) -> serde_json::Value {
        serde_json::to_value(stage_commands(bus, host, hotkeys)).unwrap()
    }

    #[test]
    fn hidden_commands_are_left_out_of_the_list() {
        let bus = bus_with(vec![
            plain("hub:open", "Opens the window."),
            Command::new("window:close", "Close")
                .hidden()
                .run_now(|_| {}),
            plain("palette:open", "Opens the palette."),
        ]);

        let ids: Vec<String> = stage_commands(&bus, &TestHost::new(true), &Hotkeys::none())
            .into_iter()
            .map(|c| c.id)
            .collect();

        assert_eq!(ids, ["hub:open", "palette:open"]);
    }

    #[test]
    fn a_command_is_listed_with_its_help_and_whether_it_can_run() {
        let bus = bus_with(vec![plain("links:open", "Opens the links.")]);

        assert_eq!(
            listed(&bus, &TestHost::new(true), &Hotkeys::none()),
            json!([{ "id": "links:open", "help": "Opens the links.", "enabled": true }])
        );
    }

    #[test]
    fn a_command_that_needs_a_vault_is_disabled_without_one() {
        let bus = bus_with(vec![
            Command::new("search:open", "Search")
                .needs_vault()
                .run_now(|_| {}),
        ]);

        let without = stage_commands(&bus, &TestHost::new(false), &Hotkeys::none());
        let with = stage_commands(&bus, &TestHost::new(true), &Hotkeys::none());

        assert!(!without[0].enabled);
        assert!(with[0].enabled);
    }

    #[test]
    fn a_command_switched_off_by_its_check_is_disabled() {
        let bus = bus_with(vec![
            Command::new("status:toggle", "Status")
                .enabled_when(|host: &TestHost| host.enabled.load(Ordering::SeqCst))
                .run_now(|_| {}),
        ]);
        let host = TestHost::new(true);

        assert!(stage_commands(&bus, &host, &Hotkeys::none())[0].enabled);
        host.enabled.store(false, Ordering::SeqCst);
        assert!(!stage_commands(&bus, &host, &Hotkeys::none())[0].enabled);
    }

    #[test]
    fn a_command_status_tells_why_it_cannot_run() {
        let bus = bus_with(vec![
            plain("hub:open", "Open"),
            Command::new("search:open", "Search")
                .needs_vault()
                .run_now(|_| {}),
            Command::new("status:toggle", "Status")
                .enabled_when(|host: &TestHost| host.enabled.load(Ordering::SeqCst))
                .run_now(|_| {}),
        ]);
        let host = TestHost::new(false);
        host.enabled.store(false, Ordering::SeqCst);

        assert_eq!(
            command_status(&bus, &host, "hub:open"),
            CommandStatus::Ready
        );
        assert_eq!(
            command_status(&bus, &host, "search:open"),
            CommandStatus::NeedsVault
        );
        assert_eq!(
            command_status(&bus, &host, "status:toggle"),
            CommandStatus::Disabled
        );
        assert_eq!(
            command_status(&bus, &host, "nope:nothing"),
            CommandStatus::Unknown
        );
    }

    #[test]
    fn a_command_without_a_vault_is_reported_as_needing_one_before_it_is_disabled() {
        let bus = bus_with(vec![
            Command::new("search:open", "Search")
                .needs_vault()
                .enabled_when(|_: &TestHost| false)
                .run_now(|_| {}),
        ]);

        assert_eq!(
            command_status(&bus, &TestHost::new(false), "search:open"),
            CommandStatus::NeedsVault
        );
        assert_eq!(
            command_status(&bus, &TestHost::new(true), "search:open"),
            CommandStatus::Disabled
        );
    }

    #[test]
    fn a_family_argument_runs_with_the_families_status() {
        let bus = family_with(Vec::new());

        assert_eq!(
            command_status(&bus, &TestHost::new(true), "context:set:work"),
            CommandStatus::Ready
        );
    }

    #[test]
    fn a_menu_key_is_the_commands_shortcut_in_canonical_notation() {
        for (command, shortcut) in [
            ("app:settings", "cmd+,"),
            ("palette:open", "cmd+k"),
            ("view:toggleInspector", "ctrl+cmd+i"),
            ("hub:open", "cmd+0"),
        ] {
            assert_eq!(
                shortcut_of(command, &Hotkeys::none()).as_deref(),
                Some(shortcut),
                "{command}"
            );
        }
    }

    #[test]
    fn the_menu_key_wins_over_a_global_shortcut() {
        let hotkeys = Hotkeys {
            palette: "ctrl+alt+cmd+p".into(),
            ..Hotkeys::none()
        };

        assert_eq!(
            shortcut_of("palette:open", &hotkeys).as_deref(),
            Some("cmd+k")
        );
    }

    #[test]
    fn a_command_without_a_menu_key_gets_its_set_global_shortcut_canonically() {
        let hotkeys = Hotkeys {
            toggle_status: "cmd+shift+s".into(),
            ..Hotkeys::none()
        };

        assert_eq!(
            shortcut_of("status:toggle", &hotkeys).as_deref(),
            Some("shift+cmd+s")
        );
        assert_eq!(shortcut_of("status:toggle", &Hotkeys::none()), None);
        assert_eq!(shortcut_of("nope:nothing", &hotkeys), None);
    }

    #[test]
    fn the_list_carries_each_commands_shortcut() {
        let hotkeys = Hotkeys {
            toggle_status: "cmd+f1".into(),
            ..Hotkeys::none()
        };
        let bus = bus_with(vec![
            plain("palette:open", "Palette"),
            plain("status:toggle", "Status"),
            plain("hub:toggle", "Hub"),
        ]);

        let shortcuts: Vec<Option<String>> = stage_commands(&bus, &TestHost::new(true), &hotkeys)
            .into_iter()
            .map(|c| c.shortcut)
            .collect();

        assert_eq!(
            shortcuts,
            [Some("cmd+k".to_string()), Some("cmd+f1".to_string()), None]
        );
    }

    #[test]
    fn a_family_lists_its_argument_and_choices_with_the_prefix() {
        let bus = family_with(vec![
            ArgChoice {
                word: "work".into(),
                title: Some("[W] Work".into()),
                argument: "work".into(),
            },
            ArgChoice {
                word: "deep".into(),
                title: None,
                argument: "deep-work".into(),
            },
        ]);

        assert_eq!(
            listed(&bus, &TestHost::new(true), &Hotkeys::none()),
            json!([{
                "id": "context:set:",
                "help": "Switches the context.",
                "enabled": true,
                "arg": "<context>",
                "choices": [
                    { "id": "context:set:work", "title": "[W] Work" },
                    { "id": "context:set:deep-work", "title": "deep" },
                ],
            }])
        );
    }

    #[test]
    fn a_family_lists_at_most_thirty_choices() {
        let many = (0..40).map(|i| ArgChoice::new(format!("c{i}"))).collect();
        let bus = family_with(many);

        let command = stage_commands(&bus, &TestHost::new(true), &Hotkeys::none()).remove(0);

        let choices = command.choices.unwrap();
        assert_eq!(choices.len(), 30);
        assert_eq!(choices[0].id, "context:set:c0");
        assert_eq!(choices[29].id, "context:set:c29");
    }

    #[test]
    fn a_family_without_choices_lists_none() {
        let mut bus = CommandBus::<TestHost>::default();
        bus.add_family(
            Command::new("settings:", "Settings")
                .family("<page>")
                .run(|_, _| async { Ok(()) }),
        );

        let command = stage_commands(&bus, &TestHost::new(true), &Hotkeys::none()).remove(0);

        assert_eq!(command.arg.as_deref(), Some("<page>"));
        assert_eq!(command.choices, Some(Vec::new()));
    }

    #[test]
    fn a_hidden_family_is_left_out() {
        let mut bus = CommandBus::<TestHost>::default();
        bus.add_family(
            Command::new("settings:open:", "Settings")
                .hidden()
                .family("<page>")
                .run(|_, _| async { Ok(()) }),
        );

        assert!(stage_commands(&bus, &TestHost::new(true), &Hotkeys::none()).is_empty());
    }

    #[test]
    fn only_the_global_shortcuts_that_are_set_are_listed_in_the_shortcuts_order() {
        let hotkeys = Hotkeys {
            journal: "ctrl+alt+cmd+j".into(),
            switch_activity: "cmd+f2".into(),
            quick_notes: "cmd+shift+m".into(),
            ..Hotkeys::none()
        };

        assert_eq!(
            stage_hotkeys(&hotkeys),
            [
                StageHotkey {
                    action: "switchActivity",
                    shortcut: "cmd+f2".into(),
                    command: "activity:switch",
                },
                StageHotkey {
                    action: "journal",
                    shortcut: "ctrl+alt+cmd+j".into(),
                    command: "journal:open",
                },
                StageHotkey {
                    action: "quickNotes",
                    shortcut: "shift+cmd+m".into(),
                    command: "notes:quick",
                },
            ]
        );
        assert!(stage_hotkeys(&Hotkeys::none()).is_empty());
    }

    #[test]
    fn a_hotkey_serializes_with_its_action_shortcut_and_command() {
        let hotkeys = Hotkeys {
            toggle_rail: "ctrl+alt+cmd+r".into(),
            ..Hotkeys::none()
        };

        assert_eq!(
            serde_json::to_value(stage_hotkeys(&hotkeys)).unwrap(),
            json!([{
                "action": "toggleRail",
                "shortcut": "ctrl+alt+cmd+r",
                "command": "rail:toggle",
            }])
        );
    }
}
