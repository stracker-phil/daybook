//! The vault, where the daily note's sections are, and what search covers.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::read::*;

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Vault {
    pub path: Option<String>,
    /// The name the Obsidian CLI knows the vault by.
    pub name: String,
    /// Overrides the Obsidian CLI lookup; empty: find it automatically.
    pub obsidian_cli_path: String,
}

impl Default for Vault {
    fn default() -> Self {
        Self {
            path: None,
            name: "Vault".into(),
            obsidian_cli_path: String::new(),
        }
    }
}

impl Vault {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        let name = trimmed(field(source, "name"));
        Self {
            path: string(field(source, "path")).filter(|path| !path.is_empty()),
            name: if name.is_empty() {
                Self::default().name
            } else {
                name
            },
            obsidian_cli_path: string(field(source, "obsidianCliPath")).unwrap_or_default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum HeadingLevel {
    #[serde(rename = "##")]
    H2,
    #[serde(rename = "###")]
    H3,
    #[serde(rename = "####")]
    H4,
}

/// A section the app writes to, found by level and text (case-insensitive).
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct SectionPlacement {
    pub level: HeadingLevel,
    pub text: String,
}

impl SectionPlacement {
    fn h2(text: &str) -> Self {
        Self {
            level: HeadingLevel::H2,
            text: text.into(),
        }
    }

    fn read(raw: Option<&Value>, fallback: Self) -> Self {
        let source = obj(raw);
        let text = string(field(source, "text")).map(|text| text.trim().to_string());
        Self {
            level: one_of(field(source, "level")).unwrap_or(fallback.level),
            text: text
                .filter(|text| !text.is_empty())
                .unwrap_or(fallback.text),
        }
    }
}

/// The daily note's sections the app knows, in their default order.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum DailySection {
    Journal,
    Focus,
    Plan,
    Notes,
}

const DAILY_SECTIONS: [DailySection; 4] = [
    DailySection::Journal,
    DailySection::Focus,
    DailySection::Plan,
    DailySection::Notes,
];

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct DailyNote {
    /// Used when Obsidian's daily-notes settings name no folder or format.
    pub fallback_folder: String,
    pub fallback_format: String,
    /// The notes heading, e.g. `## Notes`: its `###` topics are the day's notes.
    pub notes: String,
    pub journal: SectionPlacement,
    pub focus: SectionPlacement,
    pub plan: SectionPlacement,
    /// Every section once; a missing section is created at its place in this order.
    pub order: Vec<DailySection>,
}

impl Default for DailyNote {
    fn default() -> Self {
        Self {
            fallback_folder: "1-Journal".into(),
            fallback_format: "YYYY-MM-DD".into(),
            notes: "## Notes".into(),
            journal: SectionPlacement::h2("Journal"),
            focus: SectionPlacement::h2("Focus"),
            plan: SectionPlacement::h2("Plan for Today"),
            order: DAILY_SECTIONS.to_vec(),
        }
    }
}

impl DailyNote {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        let defaults = Self::default();
        let required = |key, fallback: String| {
            Some(trimmed(field(source, key)))
                .filter(|text| !text.is_empty())
                .unwrap_or(fallback)
        };
        Self {
            fallback_folder: required("fallbackFolder", defaults.fallback_folder),
            fallback_format: required("fallbackFormat", defaults.fallback_format),
            notes: required("notes", defaults.notes),
            journal: SectionPlacement::read(field(source, "journal"), defaults.journal),
            focus: SectionPlacement::read(field(source, "focus"), defaults.focus),
            plan: SectionPlacement::read(field(source, "plan"), defaults.plan),
            order: section_order(field(source, "order")),
        }
    }
}

/// Every section once, in the stored order; a section added later goes right after the one it
/// follows by default.
fn section_order(raw: Option<&Value>) -> Vec<DailySection> {
    let mut order: Vec<DailySection> = Vec::new();
    for section in items(raw).iter().filter_map(|item| one_of(Some(item))) {
        if !order.contains(&section) {
            order.push(section);
        }
    }
    for (index, section) in DAILY_SECTIONS.iter().enumerate() {
        if order.contains(section) {
            continue;
        }
        let at = if index == 0 {
            0
        } else {
            order
                .iter()
                .position(|s| *s == DAILY_SECTIONS[index - 1])
                .map_or(0, |i| i + 1)
        };
        order.insert(at, *section);
    }
    order
}

/// A named folder set for search. An entry ending in `/` is a folder, anything else another scope.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct SearchScope {
    pub name: String,
    pub entries: Vec<String>,
}

impl SearchScope {
    fn new(name: &str, entries: &[&str]) -> Self {
        Self {
            name: name.into(),
            entries: entries.iter().map(|e| e.to_string()).collect(),
        }
    }

    /// Names are lowercase words.
    fn read(raw: &Value) -> Option<Self> {
        let source = raw.as_object()?;
        let name = trimmed(source.get("name")).to_lowercase();
        is_word(&name).then(|| Self {
            name,
            entries: strings(source.get("entries")),
        })
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Search {
    pub scopes: Vec<SearchScope>,
    /// Folders never searched, e.g. `_/`.
    pub exclude: Vec<String>,
}

impl Default for Search {
    fn default() -> Self {
        Self {
            scopes: vec![
                SearchScope::new("journal", &["1-Journal/"]),
                SearchScope::new("work", &["2-Work/", "journal"]),
                SearchScope::new("private", &["3-Personal/", "journal"]),
                SearchScope::new("all", &["/"]),
            ],
            exclude: ["_/", ".obsidian/", ".trash/"].map(String::from).to_vec(),
        }
    }
}

impl Search {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        let defaults = Self::default();
        Self {
            scopes: list(field(source, "scopes"), SearchScope::read).unwrap_or(defaults.scopes),
            exclude: list_or(field(source, "exclude"), defaults.exclude),
        }
    }
}
