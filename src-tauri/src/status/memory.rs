//! Today's status in `status-memory.json`: which status, since when, and whether you were Working today.
//! The daily note is never read for it; a new day starts without a status.

use std::path::Path;

use chrono::{NaiveDate, NaiveDateTime};
use serde::{Deserialize, Serialize};

use crate::clock::{format_clock, iso_day, minute_of_day};
use crate::error::AppResult;
use crate::store::json_store::JsonStore;
use crate::store::settings::Status;

pub const FILE_NAME: &str = "status-memory.json";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct StatusMemory {
    /// `YYYY-MM-DD`
    pub date: String,
    pub status: Option<Status>,
    /// `HH:MM`
    pub since: Option<String>,
    pub worked_today: bool,
}

impl StatusMemory {
    /// What holds on `day`: another day's memory doesn't.
    pub fn on(&self, day: NaiveDate) -> Self {
        let date = iso_day(day);
        if self.date == date {
            self.clone()
        } else {
            Self {
                date,
                ..Self::default()
            }
        }
    }
}

/// The one decision behind every effect of a status; the day's first status always counts.
pub fn is_status_change(current: Option<Status>, next: Status) -> bool {
    current != Some(next)
}

#[derive(Clone, Debug, PartialEq)]
pub struct Change {
    /// `None`: the day's first status.
    pub previous: Option<Status>,
    /// The day's first Working.
    pub first_working: bool,
    pub since: String,
}

pub struct Statuses {
    store: JsonStore<StatusMemory>,
}

impl Statuses {
    pub fn open(config_dir: &Path) -> Self {
        Self {
            store: JsonStore::open(config_dir.join(FILE_NAME)),
        }
    }

    pub fn today(&self, now: NaiveDateTime) -> StatusMemory {
        self.store.read(|memory| memory.on(now.date()))
    }

    /// Remembers `next` from `now`; `None` when it is today's status already.
    pub fn set(&self, next: Status, now: NaiveDateTime) -> AppResult<Option<Change>> {
        let today = self.today(now);
        if !is_status_change(today.status, next) {
            return Ok(None);
        }
        let since = format_clock(minute_of_day(now));
        let change = Change {
            previous: today.status,
            first_working: next == Status::Working && !today.worked_today,
            since: since.clone(),
        };
        self.store.update(|memory| {
            *memory = StatusMemory {
                date: today.date,
                status: Some(next),
                since: Some(since),
                worked_today: today.worked_today || next == Status::Working,
            }
        })?;
        Ok(Some(change))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::test_support::temp_config_dir;

    fn at(day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    #[test]
    fn picking_the_status_you_have_changes_nothing() {
        for status in [Status::Working, Status::Break, Status::Off] {
            assert!(!is_status_change(Some(status), status), "{status:?}");
        }
    }

    #[test]
    fn the_first_status_of_a_day_always_counts() {
        for status in [Status::Working, Status::Break, Status::Off] {
            assert!(is_status_change(None, status), "{status:?}");
        }
    }

    #[test]
    fn another_status_is_a_change() {
        for (current, next) in [
            (Status::Working, Status::Break),
            (Status::Working, Status::Off),
            (Status::Break, Status::Working),
            (Status::Off, Status::Working),
        ] {
            assert!(
                is_status_change(Some(current), next),
                "{current:?} → {next:?}"
            );
        }
    }

    #[test]
    fn a_new_day_starts_without_a_status() {
        let dir = temp_config_dir();
        let statuses = Statuses::open(dir.path());
        statuses.set(Status::Working, at(23, 17, 45)).unwrap();

        assert_eq!(
            statuses.today(at(24, 8, 0)),
            StatusMemory {
                date: "2026-10-24".into(),
                status: None,
                since: None,
                worked_today: false,
            }
        );
    }

    #[test]
    fn setting_a_status_returns_the_change_and_shows_it_today() {
        let dir = temp_config_dir();
        let statuses = Statuses::open(dir.path());

        let change = statuses.set(Status::Working, at(24, 9, 5)).unwrap();

        assert_eq!(
            change,
            Some(Change {
                previous: None,
                first_working: true,
                since: "09:05".into(),
            })
        );
        assert_eq!(
            statuses.today(at(24, 9, 30)),
            StatusMemory {
                date: "2026-10-24".into(),
                status: Some(Status::Working),
                since: Some("09:05".into()),
                worked_today: true,
            }
        );
    }

    #[test]
    fn a_change_names_the_status_it_replaces() {
        let dir = temp_config_dir();
        let statuses = Statuses::open(dir.path());
        statuses.set(Status::Working, at(24, 9, 0)).unwrap();

        let change = statuses.set(Status::Break, at(24, 12, 0)).unwrap();

        assert_eq!(
            change,
            Some(Change {
                previous: Some(Status::Working),
                first_working: false,
                since: "12:00".into(),
            })
        );
    }

    #[test]
    fn setting_the_same_status_again_changes_nothing_and_keeps_since() {
        let dir = temp_config_dir();
        let statuses = Statuses::open(dir.path());
        statuses.set(Status::Break, at(24, 10, 0)).unwrap();

        assert_eq!(statuses.set(Status::Break, at(24, 10, 20)).unwrap(), None);
        assert_eq!(
            statuses.today(at(24, 10, 30)).since,
            Some("10:00".to_string())
        );
    }

    #[test]
    fn the_status_and_since_survive_a_restart() {
        let dir = temp_config_dir();
        Statuses::open(dir.path())
            .set(Status::Break, at(24, 10, 0))
            .unwrap();

        let reopened = Statuses::open(dir.path());

        assert_eq!(
            reopened.today(at(24, 10, 15)),
            StatusMemory {
                date: "2026-10-24".into(),
                status: Some(Status::Break),
                since: Some("10:00".into()),
                worked_today: false,
            }
        );
    }

    #[test]
    fn only_the_first_working_of_a_day_is_the_first() {
        let dir = temp_config_dir();
        let statuses = Statuses::open(dir.path());
        let mut firsts = Vec::new();
        for (status, hour) in [
            (Status::Break, 8),
            (Status::Working, 9),
            (Status::Break, 12),
            (Status::Working, 13),
        ] {
            let change = statuses.set(status, at(24, hour, 0)).unwrap().unwrap();
            firsts.push(change.first_working);
        }
        assert_eq!(firsts, [false, true, false, false]);
    }

    #[test]
    fn working_again_after_going_off_is_not_the_first() {
        let dir = temp_config_dir();
        let statuses = Statuses::open(dir.path());
        statuses.set(Status::Working, at(24, 9, 0)).unwrap();
        statuses.set(Status::Off, at(24, 17, 0)).unwrap();

        let change = statuses
            .set(Status::Working, at(24, 20, 0))
            .unwrap()
            .unwrap();

        assert!(!change.first_working);
    }

    #[test]
    fn a_new_day_makes_the_next_working_the_first_again() {
        let dir = temp_config_dir();
        let statuses = Statuses::open(dir.path());
        statuses.set(Status::Working, at(23, 9, 0)).unwrap();
        assert!(statuses.today(at(23, 10, 0)).worked_today);

        assert!(!statuses.today(at(24, 8, 0)).worked_today);
        let change = statuses
            .set(Status::Working, at(24, 9, 0))
            .unwrap()
            .unwrap();

        assert_eq!(change.previous, None);
        assert!(change.first_working);
    }

    #[test]
    fn the_file_holds_the_date_status_since_and_worked_today() {
        let dir = temp_config_dir();
        Statuses::open(dir.path())
            .set(Status::Working, at(24, 9, 5))
            .unwrap();

        let written = std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap();

        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&written).unwrap(),
            json!({
                "date": "2026-10-24",
                "status": "working",
                "since": "09:05",
                "workedToday": true
            })
        );
    }

    #[test]
    fn a_file_in_the_prototypes_shape_is_read() {
        let dir = temp_config_dir();
        std::fs::write(
            dir.path().join(FILE_NAME),
            r#"{"date":"2026-10-24","status":"break","since":"10:00","workedToday":true}"#,
        )
        .unwrap();

        let statuses = Statuses::open(dir.path());

        assert_eq!(
            statuses.today(at(24, 10, 30)),
            StatusMemory {
                date: "2026-10-24".into(),
                status: Some(Status::Break),
                since: Some("10:00".into()),
                worked_today: true,
            }
        );
        let change = statuses
            .set(Status::Working, at(24, 11, 0))
            .unwrap()
            .unwrap();
        assert!(!change.first_working);
    }
}
