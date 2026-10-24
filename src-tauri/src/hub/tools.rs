//! The Daybook window's sidebar rows: the tools the window shows, each with its title, command and global
//! shortcut. Pure.

use serde::Serialize;
use ts_rs::TS;

use crate::bus::CommandStatus;
use crate::contexts::context_title;
use crate::store::settings::{Hotkeys, ToolId, ToolPlacement};
use crate::tools::{Surface, ToolState, tidy_separators, visible_tools};

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ToolRow {
    pub id: ToolId,
    pub title: String,
    /// What a click runs; `None` for a row that only tells (the call).
    pub command: Option<String>,
    /// The global shortcut, in Settings notation (`ctrl+alt+cmd+f`).
    pub shortcut: Option<String>,
    /// Shown in the context's colour.
    pub accent: bool,
    pub enabled: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HubTool {
    Tool(ToolRow),
    Separator,
}

pub struct HubInput<'a> {
    pub tools: &'a [ToolPlacement],
    pub state: &'a ToolState,
    /// The global shortcuts that work (`shortcuts::shown`).
    pub hotkeys: &'a Hotkeys,
    pub commands: &'a dyn CommandStatus,
}

enum Entry {
    Row {
        title: String,
        /// `None` for a row that only tells.
        command: Option<&'static str>,
    },
    Separator,
}

fn entry(id: ToolId, state: &ToolState) -> Entry {
    let row = |title: &str, command| Entry::Row {
        title: title.to_string(),
        command: Some(command),
    };
    match id {
        ToolId::Context => Entry::Row {
            title: state
                .current_context()
                .map(|context| context_title(&context.label, &context.name))
                .unwrap_or_else(|| "Context".into()),
            command: Some("context:switch"),
        },
        ToolId::Status => row("Status", "status:toggle"),
        ToolId::Activity => row("Log activity…", "activity:switch"),
        ToolId::Call => Entry::Row {
            title: "In a meeting".into(),
            command: None,
        },
        ToolId::Journal => row("Journal", "journal:open"),
        ToolId::Hub => row("Daybook", "hub:open"),
        ToolId::Search => row("Search vault", "search:open"),
        ToolId::Jira => row("Open Jira issue", "jira:open"),
        ToolId::QuickNotes => row("Quick notes", "notes:quick"),
        ToolId::Links => row("Links", "links:open"),
        ToolId::Projects => row("Projects", "projects:open"),
        ToolId::Music => row("Music", "music:open"),
        ToolId::Separator1 | ToolId::Separator2 => Entry::Separator,
    }
}

fn shortcut_for(id: ToolId, hotkeys: &Hotkeys) -> Option<&str> {
    let shortcut = match id {
        ToolId::Context => &hotkeys.switch_context,
        ToolId::Status => &hotkeys.toggle_status,
        ToolId::Activity => &hotkeys.switch_activity,
        ToolId::Journal => &hotkeys.journal,
        ToolId::Hub => &hotkeys.hub,
        ToolId::Search => &hotkeys.search,
        ToolId::Jira => &hotkeys.jira_open,
        ToolId::QuickNotes => &hotkeys.quick_notes,
        ToolId::Links => &hotkeys.links,
        ToolId::Projects => &hotkeys.projects,
        ToolId::Music => &hotkeys.music,
        ToolId::Call | ToolId::Separator1 | ToolId::Separator2 => return None,
    };
    (!shortcut.is_empty()).then_some(shortcut.as_str())
}

/// The window's tools in order; a tool whose command no feature registered yet is left out, one that
/// can't run now is greyed.
pub fn hub_tools(input: HubInput) -> Vec<HubTool> {
    let ids = visible_tools(input.tools, input.state, Surface::Window);
    let kept: Vec<(ToolId, Entry)> = ids
        .into_iter()
        .map(|id| (id, entry(id, input.state)))
        .filter(|(_, entry)| match entry {
            Entry::Row {
                command: Some(command),
                ..
            } => input.commands.knows(command),
            _ => true,
        })
        .collect();
    let order = tidy_separators(kept.iter().map(|(id, _)| *id).collect());
    let mut kept = kept.into_iter();
    order
        .into_iter()
        .filter_map(|id| kept.find(|(kept_id, _)| *kept_id == id))
        .map(|(id, entry)| match entry {
            Entry::Separator => HubTool::Separator,
            Entry::Row { title, command } => HubTool::Tool(ToolRow {
                id,
                title,
                command: command.map(str::to_string),
                shortcut: shortcut_for(id, input.hotkeys).map(str::to_string),
                accent: id == ToolId::Context,
                enabled: command.is_none_or(|command| input.commands.is_enabled(command)),
            }),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::model::CommandsStub;
    use crate::store::settings::{Feature, default_tools};
    use crate::tools::bare_state;

    fn hotkeys() -> Hotkeys {
        Hotkeys {
            search: "ctrl+alt+cmd+f".into(),
            switch_context: "ctrl+alt+cmd+c".into(),
            ..Hotkeys::none()
        }
    }

    fn place(ids: &[ToolId]) -> Vec<ToolPlacement> {
        ids.iter()
            .map(|id| ToolPlacement {
                id: *id,
                rail: true,
                window: true,
            })
            .collect()
    }

    fn build(tools: &[ToolPlacement], state: &ToolState, commands: &CommandsStub) -> Vec<HubTool> {
        hub_tools(HubInput {
            tools,
            state,
            hotkeys: &hotkeys(),
            commands,
        })
    }

    /// The tool ids in order; `None` marks a separator.
    fn ids(tools: &[HubTool]) -> Vec<Option<ToolId>> {
        tools
            .iter()
            .map(|tool| match tool {
                HubTool::Tool(row) => Some(row.id),
                HubTool::Separator => None,
            })
            .collect()
    }

    fn row(tools: &[HubTool], id: ToolId) -> Option<&ToolRow> {
        tools.iter().find_map(|tool| match tool {
            HubTool::Tool(row) if row.id == id => Some(row),
            _ => None,
        })
    }

    #[test]
    fn the_default_tools_come_in_settings_order_without_what_has_nothing_to_offer() {
        let tools = build(&default_tools(), &bare_state(), &CommandsStub::default());
        assert_eq!(
            ids(&tools),
            [
                Some(ToolId::Context),
                Some(ToolId::Status),
                None,
                Some(ToolId::Journal),
                Some(ToolId::Search),
                Some(ToolId::Jira),
                Some(ToolId::QuickNotes),
            ]
        );
    }

    #[test]
    fn a_tool_switched_off_for_the_window_is_hidden() {
        let mut placements = place(&[ToolId::Status, ToolId::Journal, ToolId::Search]);
        placements[1].window = false;
        let tools = build(&placements, &bare_state(), &CommandsStub::default());
        assert_eq!(ids(&tools), [Some(ToolId::Status), Some(ToolId::Search)]);
    }

    #[test]
    fn the_hub_tool_never_shows() {
        let tools = build(&default_tools(), &bare_state(), &CommandsStub::default());
        assert_eq!(row(&tools, ToolId::Hub), None);
    }

    #[test]
    fn a_tool_whose_command_is_unknown_is_left_out_and_separators_are_tidied() {
        let placements = place(&[
            ToolId::Status,
            ToolId::Separator1,
            ToolId::Journal,
            ToolId::Separator2,
            ToolId::Search,
        ]);
        let state = bare_state();

        let no_journal = CommandsStub {
            unknown: vec!["journal:open"],
            ..CommandsStub::default()
        };
        assert_eq!(
            ids(&build(&placements, &state, &no_journal)),
            [Some(ToolId::Status), None, Some(ToolId::Search)]
        );

        let only_status = CommandsStub {
            unknown: vec!["journal:open", "search:open"],
            ..CommandsStub::default()
        };
        assert_eq!(
            ids(&build(&placements, &state, &only_status)),
            [Some(ToolId::Status)]
        );
    }

    #[test]
    fn a_known_but_disabled_command_gives_a_greyed_row() {
        let commands = CommandsStub {
            disabled: vec!["status:toggle"],
            ..CommandsStub::default()
        };
        let tools = build(
            &place(&[ToolId::Status, ToolId::Journal]),
            &bare_state(),
            &commands,
        );
        assert_eq!(row(&tools, ToolId::Status).map(|r| r.enabled), Some(false));
        assert_eq!(row(&tools, ToolId::Journal).map(|r| r.enabled), Some(true));
    }

    #[test]
    fn the_context_row_is_titled_by_the_current_context_and_accented() {
        let tools = build(
            &place(&[ToolId::Context]),
            &bare_state(),
            &CommandsStub::default(),
        );
        assert_eq!(
            tools,
            [HubTool::Tool(ToolRow {
                id: ToolId::Context,
                title: "[W] Work".into(),
                command: Some("context:switch".into()),
                shortcut: Some("ctrl+alt+cmd+c".into()),
                accent: true,
                enabled: true,
            })]
        );
    }

    #[test]
    fn a_row_carries_its_shortcut_and_none_when_it_has_none() {
        let tools = build(
            &place(&[ToolId::Search, ToolId::Journal]),
            &bare_state(),
            &CommandsStub::default(),
        );
        assert_eq!(
            row(&tools, ToolId::Search),
            Some(&ToolRow {
                id: ToolId::Search,
                title: "Search vault".into(),
                command: Some("search:open".into()),
                shortcut: Some("ctrl+alt+cmd+f".into()),
                accent: false,
                enabled: true,
            })
        );
        assert_eq!(
            row(&tools, ToolId::Journal).map(|r| r.shortcut.clone()),
            Some(None)
        );
    }

    #[test]
    fn the_call_row_only_tells() {
        let state = ToolState {
            meeting_since: Some("10:15".into()),
            ..bare_state()
        };
        let tools = build(&place(&[ToolId::Call]), &state, &CommandsStub::default());
        assert_eq!(
            tools,
            [HubTool::Tool(ToolRow {
                id: ToolId::Call,
                title: "In a meeting".into(),
                command: None,
                shortcut: None,
                accent: false,
                enabled: true,
            })]
        );
    }

    #[test]
    fn jira_stays_hidden_without_the_jira_feature() {
        let state = ToolState {
            features: vec![Feature::Status],
            ..bare_state()
        };
        let tools = build(
            &place(&[ToolId::Status, ToolId::Jira]),
            &state,
            &CommandsStub::default(),
        );
        assert_eq!(ids(&tools), [Some(ToolId::Status)]);
    }

    #[test]
    fn rows_serialize_with_a_kind_tag_and_camel_case_fields() {
        let tools = build(
            &place(&[ToolId::Search, ToolId::Separator1, ToolId::Journal]),
            &bare_state(),
            &CommandsStub::default(),
        );
        assert_eq!(
            serde_json::to_value(&tools).unwrap(),
            serde_json::json!([
                {
                    "kind": "tool",
                    "id": "search",
                    "title": "Search vault",
                    "command": "search:open",
                    "shortcut": "ctrl+alt+cmd+f",
                    "accent": false,
                    "enabled": true
                },
                { "kind": "separator" },
                {
                    "kind": "tool",
                    "id": "journal",
                    "title": "Journal",
                    "command": "journal:open",
                    "shortcut": null,
                    "accent": false,
                    "enabled": true
                }
            ])
        );
    }
}
