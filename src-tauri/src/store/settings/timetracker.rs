//! Where the `timetracker` tool is and whether projects were linked by name once (docs/timetracker.md).

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use super::read::*;

#[derive(Serialize, Clone, Debug, PartialEq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct TimeTrackerSettings {
    /// The `timetracker` command; empty: `~/.local/bin/timetracker`.
    pub path: String,
    /// Projects were linked to TimeTracker's by name once; it doesn't happen again.
    pub linked: bool,
}

impl TimeTrackerSettings {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        Self {
            path: trimmed(field(source, "path")),
            linked: flag(field(source, "linked")).unwrap_or(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_missing_or_malformed_section_reads_as_the_defaults() {
        for raw in [None, Some(json!("x")), Some(json!(null)), Some(json!({}))] {
            assert_eq!(
                TimeTrackerSettings::read(raw.as_ref()),
                TimeTrackerSettings::default(),
                "{raw:?}"
            );
        }
    }

    #[test]
    fn the_path_is_trimmed_and_linked_is_kept() {
        let read =
            TimeTrackerSettings::read(Some(&json!({ "path": "  /bin/tt  ", "linked": true })));

        assert_eq!(
            read,
            TimeTrackerSettings {
                path: "/bin/tt".into(),
                linked: true
            }
        );
    }

    #[test]
    fn the_section_round_trips_with_camel_case_fields() {
        let original = TimeTrackerSettings {
            path: "/bin/tt".into(),
            linked: true,
        };

        let value = serde_json::to_value(&original).unwrap();

        assert_eq!(value, json!({ "path": "/bin/tt", "linked": true }));
        assert_eq!(TimeTrackerSettings::read(Some(&value)), original);
    }
}
