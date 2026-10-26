//! The triggers that already opened something today, in `routines.json`; a new day starts with none.

use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::store::json_store::JsonStore;

pub const FILE_NAME: &str = "routines.json";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
struct FiredDay {
    /// `YYYY-MM-DD`
    date: String,
    fired: Vec<String>,
}

pub struct Fired {
    store: JsonStore<FiredDay>,
}

impl Fired {
    pub fn open(config_dir: &Path) -> Self {
        Self {
            store: JsonStore::open(config_dir.join(FILE_NAME)),
        }
    }

    /// Remembers `key` as fired on `day`; `false` when it already fired that day.
    pub fn mark(&self, key: &str, day: NaiveDate) -> AppResult<bool> {
        let date = day.format("%Y-%m-%d").to_string();
        self.store.update(|current| {
            if current.date != date {
                *current = FiredDay {
                    date,
                    fired: Vec::new(),
                };
            }
            if current.fired.iter().any(|fired| fired == key) {
                return false;
            }
            current.fired.push(key.to_string());
            true
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_config_dir;

    fn day(text: &str) -> NaiveDate {
        text.parse().unwrap()
    }

    #[test]
    fn a_key_fires_once_per_day() {
        let dir = temp_config_dir();
        let fired = Fired::open(dir.path());
        let monday = day("2026-10-26");

        assert!(fired.mark("routine:morning@due", monday).unwrap());
        assert!(!fired.mark("routine:morning@due", monday).unwrap());
        assert!(fired.mark("routine:evening@due", monday).unwrap());
    }

    #[test]
    fn a_new_day_starts_with_nothing_fired() {
        let dir = temp_config_dir();
        let fired = Fired::open(dir.path());
        fired.mark("k", day("2026-10-26")).unwrap();

        assert!(fired.mark("k", day("2026-10-27")).unwrap());
        assert!(!fired.mark("k", day("2026-10-27")).unwrap());
    }

    #[test]
    fn marks_survive_reopening_the_store() {
        let dir = temp_config_dir();
        Fired::open(dir.path())
            .mark("k", day("2026-10-26"))
            .unwrap();

        assert!(
            !Fired::open(dir.path())
                .mark("k", day("2026-10-26"))
                .unwrap()
        );
    }

    #[test]
    fn the_file_written_by_the_prototype_import_is_read() {
        let dir = temp_config_dir();
        std::fs::write(
            dir.path().join(FILE_NAME),
            r#"{"date":"2026-10-26","fired":["routine:morning@due"]}"#,
        )
        .unwrap();
        let fired = Fired::open(dir.path());

        assert!(
            !fired
                .mark("routine:morning@due", day("2026-10-26"))
                .unwrap()
        );
    }
}
