//! Quick access links with the rules that show them, and Jira.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::read::*;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "snake_case")]
pub enum EventWindow {
    UntilEnd,
    During,
    Before,
}

/// Minutes after midnight, `from` before `to`.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct MinuteRange {
    pub from: f64,
    pub to: f64,
}

impl MinuteRange {
    fn read(raw: Option<&Value>) -> Option<Self> {
        let source = obj(raw)?;
        let from = coerced_number(source.get("from"))?;
        let to = coerced_number(source.get("to"))?;
        (from < to).then_some(Self { from, to })
    }
}

/// A quick access link; every rule that is set must match.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct QuickLink {
    pub title: String,
    /// A vault path, `https://` URL or `obsidian://` URI.
    pub target: String,
    pub contexts: Option<Vec<String>>,
    /// Project names: shown while the current work is for one of them.
    pub projects: Option<Vec<String>>,
    /// 0 = Sunday … 6 = Saturday.
    pub days: Option<Vec<u8>>,
    pub time: Option<MinuteRange>,
    /// Shown around calendar events whose title contains this.
    pub event: Option<String>,
    pub event_window: EventWindow,
    pub lead_minutes: f64,
    pub pinned: bool,
}

impl QuickLink {
    /// Links need a target.
    pub fn read(raw: &Value) -> Option<Self> {
        let source = raw.as_object()?;
        let target = trimmed(source.get("target"));
        if target.is_empty() {
            return None;
        }
        let lead_minutes = coerced_number(source.get("leadMinutes")).filter(|m| *m > 0.0);
        Some(Self {
            title: trimmed(source.get("title")),
            target,
            contexts: non_empty(strings(source.get("contexts"))),
            projects: non_empty(strings(source.get("projects"))),
            days: non_empty(weekdays(source.get("days"))),
            time: MinuteRange::read(source.get("time")),
            event: Some(trimmed(source.get("event"))).filter(|event| !event.is_empty()),
            event_window: one_of(source.get("eventWindow")).unwrap_or(EventWindow::UntilEnd),
            lead_minutes: lead_minutes.unwrap_or(15.0),
            pinned: is_true(source.get("pinned")),
        })
    }
}

/// "Open Jira Issue" opens `base_url` + the key; a bare number gets `default_prefix`.
#[derive(Serialize, Clone, Debug, PartialEq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Jira {
    pub base_url: String,
    pub default_prefix: String,
}

impl Jira {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        Self {
            base_url: string(field(source, "baseUrl")).unwrap_or_default(),
            default_prefix: string(field(source, "defaultPrefix")).unwrap_or_default(),
        }
    }
}

/// An empty rule list means the rule isn't set.
fn non_empty<T>(list: Vec<T>) -> Option<Vec<T>> {
    Some(list).filter(|list| !list.is_empty())
}
