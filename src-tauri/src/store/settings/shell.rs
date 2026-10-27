//! How the app is reached: the tools and where they show, global shortcuts, the HTTP API, and the
//! hours the day timeline shows.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::read::*;

/// The tools of the rail, the Daybook window's sidebar and the menu-bar menu.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum ToolId {
    Context,
    Status,
    Activity,
    Call,
    Separator1,
    Journal,
    Hub,
    Search,
    Jira,
    QuickNotes,
    Separator2,
    Links,
    Projects,
    Music,
}

/// In their default order.
const TOOL_IDS: [ToolId; 14] = [
    ToolId::Context,
    ToolId::Status,
    ToolId::Activity,
    ToolId::Call,
    ToolId::Separator1,
    ToolId::Journal,
    ToolId::Hub,
    ToolId::Search,
    ToolId::Jira,
    ToolId::QuickNotes,
    ToolId::Separator2,
    ToolId::Links,
    ToolId::Projects,
    ToolId::Music,
];

/// Where a tool shows; the menu-bar menu always shows every tool.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ToolPlacement {
    pub id: ToolId,
    pub rail: bool,
    /// The Daybook window's sidebar; never for the Daybook tool (`Hub`) itself.
    pub window: bool,
}

impl ToolPlacement {
    fn new(id: ToolId) -> Self {
        Self {
            id,
            rail: true,
            window: id != ToolId::Hub,
        }
    }
}

pub fn default_tools() -> Vec<ToolPlacement> {
    TOOL_IDS.map(ToolPlacement::new).to_vec()
}

/// Every tool once, in the stored order. A tool added later goes right after the tool it follows by
/// default, or first when that's the first tool.
pub fn read_tools(raw: Option<&Value>) -> Vec<ToolPlacement> {
    let mut tools: Vec<ToolPlacement> = Vec::new();
    for entry in items(raw) {
        let source = entry.as_object();
        let Some(id) = one_of::<ToolId>(field(source, "id")) else {
            continue;
        };
        if tools.iter().any(|tool| tool.id == id) {
            continue;
        }
        tools.push(ToolPlacement {
            id,
            rail: on_unless_false(field(source, "rail")),
            window: id != ToolId::Hub && on_unless_false(field(source, "window")),
        });
    }
    for (index, id) in TOOL_IDS.iter().enumerate() {
        if tools.iter().any(|tool| tool.id == *id) {
            continue;
        }
        let at = match index {
            0 => 0,
            _ => tools
                .iter()
                .position(|tool| tool.id == TOOL_IDS[index - 1])
                .map_or(0, |i| i + 1),
        };
        tools.insert(at, ToolPlacement::new(*id));
    }
    tools
}

/// Global shortcuts in lowercase `ctrl+alt+cmd+a` notation; empty: no shortcut.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Hotkeys {
    pub switch_activity: String,
    pub toggle_status: String,
    pub jira_open: String,
    pub journal: String,
    pub search: String,
    pub toggle_rail: String,
    pub hub: String,
    pub links: String,
    pub quick_notes: String,
    pub switch_context: String,
    pub focus: String,
    pub music: String,
    pub palette: String,
    pub projects: String,
}

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            switch_activity: "cmd+f2".into(),
            toggle_status: "cmd+f1".into(),
            jira_open: "ctrl+alt+cmd+j".into(),
            journal: "ctrl+alt+cmd+n".into(),
            search: "ctrl+alt+cmd+f".into(),
            toggle_rail: "ctrl+alt+cmd+r".into(),
            hub: String::new(),
            links: "ctrl+alt+cmd+l".into(),
            quick_notes: "ctrl+alt+cmd+m".into(),
            switch_context: String::new(),
            focus: String::new(),
            music: String::new(),
            palette: String::new(),
            projects: String::new(),
        }
    }
}

impl Hotkeys {
    pub fn none() -> Self {
        Self {
            switch_activity: String::new(),
            toggle_status: String::new(),
            jira_open: String::new(),
            journal: String::new(),
            search: String::new(),
            toggle_rail: String::new(),
            hub: String::new(),
            links: String::new(),
            quick_notes: String::new(),
            switch_context: String::new(),
            focus: String::new(),
            music: String::new(),
            palette: String::new(),
            projects: String::new(),
        }
    }

    /// A stored `""` means the shortcut was cleared; only shortcuts never stored get their default.
    pub fn read(raw: Option<&Value>) -> Self {
        let d = Self::default();
        let Some(source) = obj(raw) else {
            return d;
        };
        let key = |name, fallback| string(source.get(name)).unwrap_or(fallback);
        Self {
            switch_activity: key("switchActivity", d.switch_activity),
            toggle_status: key("toggleStatus", d.toggle_status),
            jira_open: key("jiraOpen", d.jira_open),
            journal: key("journal", d.journal),
            search: key("search", d.search),
            toggle_rail: key("toggleRail", d.toggle_rail),
            hub: key("hub", d.hub),
            links: key("links", d.links),
            quick_notes: key("quickNotes", d.quick_notes),
            switch_context: key("switchContext", d.switch_context),
            focus: key("focus", d.focus),
            music: key("music", d.music),
            palette: key("palette", d.palette),
            projects: key("projects", d.projects),
        }
    }
}

/// The local HTTP API for automation and Stage.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Api {
    pub enabled: bool,
    pub port: u16,
    /// Only a token imported from the prototype, until it moves to the Keychain on the next start
    /// (`http_api::token`); never shown to the page.
    #[serde(skip_serializing_if = "String::is_empty")]
    #[ts(skip)]
    pub token: String,
    /// The Stage hook and its discovery file.
    pub presentations: bool,
}

impl Default for Api {
    fn default() -> Self {
        Self {
            enabled: false,
            port: 27180,
            token: String::new(),
            presentations: true,
        }
    }
}

impl Api {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        let d = Self::default();
        let port = number(field(source, "port"))
            .filter(|port| port.fract() == 0.0 && (1024.0..=65535.0).contains(port));
        Self {
            enabled: flag(field(source, "enabled")).unwrap_or(d.enabled),
            port: port.map_or(d.port, |port| port as u16),
            token: string(field(source, "token")).unwrap_or_default(),
            presentations: flag(field(source, "presentations")).unwrap_or(d.presentations),
        }
    }
}

/// The hours the day timeline shows, `first_hour` before `last_hour`.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Timeline {
    pub first_hour: u8,
    pub last_hour: u8,
}

impl Default for Timeline {
    fn default() -> Self {
        Self {
            first_hour: 0,
            last_hour: 24,
        }
    }
}

impl Timeline {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        let d = Self::default();
        let first = within(coerced_number(field(source, "firstHour")), 0.0, 23.0)
            .map_or(d.first_hour, |hour| round(hour) as u8);
        let last = within(
            coerced_number(field(source, "lastHour")),
            f64::from(first) + 1.0,
            24.0,
        )
        .map_or(d.last_hour.max(first + 1), |hour| round(hour) as u8);
        Self {
            first_hour: first,
            last_hour: last,
        }
    }
}
