//! Where the `timetracker` tool is, whether projects were linked by name once and whether the status
//! drives the timer (docs/timetracker.md).

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use super::read::*;

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct TimeTrackerSettings {
    /// The `timetracker` command; empty: `~/.local/bin/timetracker`.
    pub path: String,
    /// Projects were linked to TimeTracker's by name once; it doesn't happen again.
    pub linked: bool,
    /// Working continues the last timer, Break and Off stop it.
    pub status_timer: bool,
}

impl Default for TimeTrackerSettings {
    fn default() -> Self {
        Self {
            path: String::new(),
            linked: false,
            status_timer: true,
        }
    }
}

impl TimeTrackerSettings {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        Self {
            path: trimmed(field(source, "path")),
            linked: flag(field(source, "linked")).unwrap_or(false),
            status_timer: flag(field(source, "statusTimer")).unwrap_or(true),
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
                linked: true,
                status_timer: true,
            }
        );
    }

    #[test]
    fn the_status_timer_is_on_unless_turned_off() {
        for (raw, expected) in [
            (json!({}), true),
            (json!({ "path": "/bin/tt" }), true),
            (json!({ "statusTimer": true }), true),
            (json!({ "statusTimer": false }), false),
        ] {
            assert_eq!(
                TimeTrackerSettings::read(Some(&raw)).status_timer,
                expected,
                "{raw}"
            );
        }
    }

    #[test]
    fn a_turned_off_status_timer_round_trips() {
        let original = TimeTrackerSettings {
            status_timer: false,
            ..TimeTrackerSettings::default()
        };

        let value = serde_json::to_value(&original).unwrap();

        assert_eq!(value["statusTimer"], json!(false));
        assert_eq!(TimeTrackerSettings::read(Some(&value)), original);
    }

    #[test]
    fn the_section_round_trips_with_camel_case_fields() {
        let original = TimeTrackerSettings {
            path: "/bin/tt".into(),
            linked: true,
            status_timer: true,
        };

        let value = serde_json::to_value(&original).unwrap();

        assert_eq!(
            value,
            json!({ "path": "/bin/tt", "linked": true, "statusTimer": true })
        );
        assert_eq!(TimeTrackerSettings::read(Some(&value)), original);
    }
}
