//! The Jira issues opened in a context with the Jira feature, by day, in `jira-tickets.json`
//! (`{ "YYYY-MM-DD": ["QRS-6805", …] }`, keys in the order first opened). Two weeks are kept.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{Days, NaiveDate};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::clock::iso_day;
use crate::error::AppResult;
use crate::store::json_store::JsonStore;

const FILE_NAME: &str = "jira-tickets.json";
/// Days before today that recording keeps.
const KEEP_DAYS: u64 = 14;
/// Days the overlay suggests from, today included.
pub const SUGGESTED_DAYS: u64 = 7;

/// Day → keys. A day that isn't a list and keys that aren't text are dropped on load.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(transparent)]
struct TicketDays(BTreeMap<String, Vec<String>>);

impl<'de> Deserialize<'de> for TicketDays {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Value::Object(days) = Value::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        let days = days
            .into_iter()
            .filter_map(|(day, keys)| {
                let keys = keys
                    .as_array()?
                    .iter()
                    .filter_map(|key| key.as_str().map(str::to_string))
                    .collect();
                Some((day, keys))
            })
            .collect();
        Ok(Self(days))
    }
}

impl TicketDays {
    fn on(&self, day: NaiveDate) -> Vec<String> {
        self.0.get(&iso_day(day)).cloned().unwrap_or_default()
    }

    /// Adds `key` to `day` once; days before `cutoff` go.
    fn add(&mut self, day: NaiveDate, key: &str, cutoff: NaiveDate) {
        let cutoff = iso_day(cutoff);
        self.0.retain(|date, _| *date >= cutoff);
        let keys = self.0.entry(iso_day(day)).or_default();
        if !keys.iter().any(|known| known == key) {
            keys.push(key.to_string());
        }
    }
}

pub struct JiraTickets {
    store: JsonStore<TicketDays>,
}

impl JiraTickets {
    pub fn open(config_dir: &Path) -> Self {
        Self {
            store: JsonStore::open(config_dir.join(FILE_NAME)),
        }
    }

    /// Adds `key` to `day`'s list once and forgets days more than two weeks before it.
    pub fn record(&self, key: &str, day: NaiveDate) -> AppResult<()> {
        let cutoff = day - Days::new(KEEP_DAYS);
        self.store.update(|days| days.add(day, key, cutoff))
    }

    /// `day`'s keys, newest first.
    pub fn on(&self, day: NaiveDate) -> Vec<String> {
        let mut keys = self.store.read(|days| days.on(day));
        keys.reverse();
        keys
    }

    /// The keys of the `days` days up to `today`: today first, newest first within a day, each once.
    pub fn recent(&self, today: NaiveDate, days: u64) -> Vec<String> {
        self.store.read(|stored| {
            let mut keys: Vec<String> = Vec::new();
            for back in 0..days {
                let Some(day) = today.checked_sub_days(Days::new(back)) else {
                    break;
                };
                for key in stored.on(day).into_iter().rev() {
                    if !keys.contains(&key) {
                        keys.push(key);
                    }
                }
            }
            keys
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_config_dir;

    fn day(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, day).unwrap()
    }

    fn tickets_in(contents: &str) -> (tempfile::TempDir, JiraTickets) {
        let dir = temp_config_dir();
        std::fs::write(dir.path().join(FILE_NAME), contents).unwrap();
        let tickets = JiraTickets::open(dir.path());
        (dir, tickets)
    }

    #[test]
    fn a_key_is_recorded_once_per_day() {
        let dir = temp_config_dir();
        let tickets = JiraTickets::open(dir.path());
        tickets.record("QRS-1", day(12)).unwrap();
        tickets.record("QRS-2", day(12)).unwrap();
        tickets.record("QRS-1", day(12)).unwrap();
        assert_eq!(tickets.on(day(12)), ["QRS-2", "QRS-1"]);
        assert!(tickets.on(day(13)).is_empty());
    }

    #[test]
    fn the_same_key_on_another_day_is_recorded_again() {
        let dir = temp_config_dir();
        let tickets = JiraTickets::open(dir.path());
        tickets.record("QRS-1", day(12)).unwrap();
        tickets.record("QRS-1", day(13)).unwrap();
        assert_eq!(tickets.on(day(12)), ["QRS-1"]);
        assert_eq!(tickets.on(day(13)), ["QRS-1"]);
    }

    #[test]
    fn recorded_keys_survive_reopening() {
        let dir = temp_config_dir();
        let tickets = JiraTickets::open(dir.path());
        tickets.record("QRS-1", day(12)).unwrap();
        tickets.record("QRS-2", day(12)).unwrap();
        drop(tickets);
        let reopened = JiraTickets::open(dir.path());
        assert_eq!(reopened.on(day(12)), ["QRS-2", "QRS-1"]);
    }

    #[test]
    fn days_older_than_fourteen_days_are_dropped_when_recording() {
        let dir = temp_config_dir();
        let tickets = JiraTickets::open(dir.path());
        tickets.record("OLD-1", day(1)).unwrap();
        tickets.record("QRS-1", day(15)).unwrap();
        assert_eq!(tickets.on(day(1)), ["OLD-1"]);
        tickets.record("QRS-2", day(16)).unwrap();
        assert!(tickets.on(day(1)).is_empty());
        assert_eq!(tickets.on(day(15)), ["QRS-1"]);
        assert_eq!(tickets.on(day(16)), ["QRS-2"]);
    }

    #[test]
    fn recent_keys_run_from_today_back_newest_first_without_repeats() {
        let dir = temp_config_dir();
        let tickets = JiraTickets::open(dir.path());
        tickets.record("OLD-1", day(13)).unwrap();
        tickets.record("WEEK-1", day(14)).unwrap();
        tickets.record("YEST-1", day(19)).unwrap();
        tickets.record("TODAY-1", day(19)).unwrap();
        tickets.record("TODAY-1", day(20)).unwrap();
        tickets.record("TODAY-2", day(20)).unwrap();
        assert_eq!(
            tickets.recent(day(20), SUGGESTED_DAYS),
            ["TODAY-2", "TODAY-1", "YEST-1", "WEEK-1"]
        );
    }

    #[test]
    fn recent_keys_are_empty_when_nothing_was_opened() {
        let dir = temp_config_dir();
        let tickets = JiraTickets::open(dir.path());
        assert!(tickets.recent(day(20), SUGGESTED_DAYS).is_empty());
    }

    #[test]
    fn a_file_that_is_not_an_object_reads_as_empty() {
        let (_dir, tickets) = tickets_in(r#"["QRS-1"]"#);
        assert!(tickets.on(day(12)).is_empty());
        assert!(tickets.recent(day(12), SUGGESTED_DAYS).is_empty());
    }

    #[test]
    fn days_that_are_not_lists_and_keys_that_are_not_text_are_dropped() {
        let (_dir, tickets) = tickets_in(
            r#"{
                "2026-10-12": ["QRS-1", 5, null, "QRS-2"],
                "2026-10-11": "QRS-3",
                "2026-10-10": {"key": "QRS-4"}
            }"#,
        );
        assert_eq!(tickets.on(day(12)), ["QRS-2", "QRS-1"]);
        assert!(tickets.on(day(11)).is_empty());
        assert!(tickets.on(day(10)).is_empty());
        assert_eq!(tickets.recent(day(12), SUGGESTED_DAYS), ["QRS-2", "QRS-1"]);
    }
}
