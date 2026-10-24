//! Contexts (which features are on, work hours, search scope) and projects.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::music::ContextMusic;
use super::read::*;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum Feature {
    /// The status and check-ins.
    Status,
    Slack,
    Jira,
    Routines,
}

/// Each context's hue: the current item in overlays, progress, the timeline's work hours.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum ContextColor {
    Blue,
    Purple,
    Pink,
    Red,
    Orange,
    Yellow,
    Green,
    Teal,
    Graphite,
}

const CONTEXT_COLORS: [ContextColor; 9] = [
    ContextColor::Blue,
    ContextColor::Purple,
    ContextColor::Pink,
    ContextColor::Red,
    ContextColor::Orange,
    ContextColor::Yellow,
    ContextColor::Green,
    ContextColor::Teal,
    ContextColor::Graphite,
];

/// On `days` (0 = Sunday; empty: every day) from `from` to `to` (`HH:MM`; a `to` before `from` runs
/// past midnight).
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct WorkHours {
    pub days: Vec<u8>,
    pub from: String,
    pub to: String,
}

impl WorkHours {
    fn read(raw: Option<&Value>) -> Option<Self> {
        let source = obj(raw);
        Some(Self {
            from: clock(field(source, "from"))?,
            to: clock(field(source, "to"))?,
            days: weekdays(field(source, "days")),
        })
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Context {
    pub id: String,
    /// The label in the menu bar, e.g. `[W]` or an emoji.
    pub label: String,
    pub name: String,
    pub features: Vec<Feature>,
    pub search_scope: String,
    pub color: ContextColor,
    /// Tinted in the day timeline, e.g. the workday; `None`: none.
    pub work_hours: Option<WorkHours>,
    /// Becomes active when its work hours start.
    pub auto_switch: bool,
    /// Titles of the calendars whose events count while it is active.
    pub calendars: Vec<String>,
    /// Minutes between check-ins while Working during the work hours; 0: none.
    pub check_in_minutes: u32,
    pub music: ContextMusic,
}

impl Context {
    fn read(raw: &Value, index: usize, seen: &mut HashSet<String>) -> Option<Self> {
        let source = raw.as_object()?;
        let name = trimmed(source.get("name"));
        if name.is_empty() {
            return None;
        }
        let work_hours = WorkHours::read(source.get("workHours"));
        Some(Self {
            id: unique_id(source, &name, seen, "context"),
            label: text(source.get("label")),
            features: strings(source.get("features"))
                .into_iter()
                .filter_map(|feature| one_of(Some(&Value::String(feature))))
                .collect(),
            search_scope: Some(trimmed(source.get("searchScope")))
                .filter(|scope| !scope.is_empty())
                .unwrap_or_else(|| "all".into()),
            color: palette_color(source.get("color"), index),
            auto_switch: work_hours.is_some() && is_true(source.get("autoSwitch")),
            work_hours,
            calendars: strings(source.get("calendars")),
            check_in_minutes: within(coerced_number(source.get("checkInMinutes")), 0.0, 240.0)
                .map_or(15, |minutes| round(minutes) as u32),
            music: ContextMusic::read(source.get("music")),
            name,
        })
    }

    /// Contexts need a name; ids are unique and a missing colour takes the next of the palette.
    pub fn read_all(raw: &[Value]) -> Vec<Self> {
        let mut seen = HashSet::new();
        raw.iter()
            .enumerate()
            .filter_map(|(index, item)| Self::read(item, index, &mut seen))
            .collect()
    }
}

fn palette_color(raw: Option<&Value>, index: usize) -> ContextColor {
    one_of(raw).unwrap_or(CONTEXT_COLORS[index % CONTEXT_COLORS.len()])
}

/// A Project Launcher project: its id opens it, the name is the one it had when linked.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct LinkedProject {
    pub id: String,
    pub name: String,
}

/// Picked for meetings, focus and check-ins; its name goes after the Slack status while Working
/// (`9:00 - 18:00 | Oscorp`), except for Internal.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub name: String,
    /// `:nexo:` starts the project's lines in the focus list; empty: none.
    pub slack_emoji: String,
    /// Off: never offered for new entries.
    pub active: bool,
    /// Ids of the contexts it's offered in; empty: every context.
    pub contexts: Vec<String>,
    /// Words in a meeting's title that mean this project (the name always counts).
    pub keywords: Vec<String>,
    /// Opened from the Projects tool.
    pub launcher_projects: Vec<LinkedProject>,
}

impl Project {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.into(),
            slack_emoji: String::new(),
            active: true,
            contexts: vec![],
            keywords: vec![],
            launcher_projects: vec![],
        }
    }

    pub fn read(raw: &Value) -> Option<Self> {
        let source = raw.as_object()?;
        let name = trimmed(source.get("name"));
        if name.is_empty() {
            return None;
        }
        Some(Self {
            slack_emoji: slack_emoji(source.get("slackEmoji")),
            active: on_unless_false(source.get("active")),
            contexts: distinct(strings(source.get("contexts"))),
            keywords: keywords(source.get("keywords")),
            launcher_projects: linked_projects(source.get("launcherProjects")),
            name,
        })
    }
}

/// `nexo` and `:nexo:` both become `:nexo:`.
fn slack_emoji(raw: Option<&Value>) -> String {
    let name: String = trimmed(raw)
        .chars()
        .filter(|c| *c != ':' && !c.is_whitespace())
        .collect();
    if name.is_empty() {
        name
    } else {
        format!(":{name}:")
    }
}

/// Each keyword once, whatever its case.
fn keywords(raw: Option<&Value>) -> Vec<String> {
    let mut seen = HashSet::new();
    strings(raw)
        .into_iter()
        .filter(|word| seen.insert(word.to_lowercase()))
        .collect()
}

/// A missing name shows the id.
fn linked_projects(raw: Option<&Value>) -> Vec<LinkedProject> {
    let mut seen = HashSet::new();
    items(raw)
        .iter()
        .filter_map(|entry| {
            let source = entry.as_object();
            let id = trimmed(field(source, "id"));
            if id.is_empty() || !seen.insert(id.clone()) {
                return None;
            }
            let name = Some(trimmed(field(source, "name")))
                .filter(|name| !name.is_empty())
                .unwrap_or_else(|| id.clone());
            Some(LinkedProject { id, name })
        })
        .collect()
}
