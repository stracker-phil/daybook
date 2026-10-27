//! "Pause Check-ins Today", in `checkins.json`: it survives a restart and ends with the day.

use std::path::Path;

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::error::AppResult;
use crate::store::json_store::JsonStore;

pub const FILE_NAME: &str = "checkins.json";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default, rename_all = "camelCase")]
struct Paused {
    paused_on: Option<NaiveDate>,
}

pub struct CheckInPause {
    store: JsonStore<Paused>,
}

impl CheckInPause {
    pub fn open(config_dir: &Path) -> Self {
        Self {
            store: JsonStore::open(config_dir.join(FILE_NAME)),
        }
    }

    pub fn is_paused(&self, today: NaiveDate) -> bool {
        self.store.read(|paused| paused.paused_on == Some(today))
    }

    /// Pauses today's check-ins or resumes them; whether they are paused now.
    pub fn toggle(&self, today: NaiveDate) -> AppResult<bool> {
        self.store.update(|paused| {
            let pause = paused.paused_on != Some(today);
            paused.paused_on = pause.then_some(today);
            pause
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, d).unwrap()
    }

    #[test]
    fn check_ins_are_not_paused_by_default() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!CheckInPause::open(dir.path()).is_paused(day(27)));
    }

    #[test]
    fn toggling_pauses_today_and_toggling_again_resumes() {
        let dir = tempfile::tempdir().unwrap();
        let pause = CheckInPause::open(dir.path());
        assert!(pause.toggle(day(27)).unwrap());
        assert!(pause.is_paused(day(27)));
        assert!(!pause.toggle(day(27)).unwrap());
        assert!(!pause.is_paused(day(27)));
    }

    #[test]
    fn the_pause_ends_with_the_day() {
        let dir = tempfile::tempdir().unwrap();
        let pause = CheckInPause::open(dir.path());
        pause.toggle(day(27)).unwrap();
        assert!(!pause.is_paused(day(28)));
    }

    #[test]
    fn toggling_the_next_day_pauses_that_day_after_an_earlier_pause() {
        let dir = tempfile::tempdir().unwrap();
        let pause = CheckInPause::open(dir.path());
        pause.toggle(day(27)).unwrap();
        assert!(pause.toggle(day(28)).unwrap());
        assert!(pause.is_paused(day(28)));
        assert!(!pause.is_paused(day(27)));
    }

    #[test]
    fn the_pause_survives_reopening() {
        let dir = tempfile::tempdir().unwrap();
        CheckInPause::open(dir.path()).toggle(day(27)).unwrap();
        let reopened = CheckInPause::open(dir.path());
        assert!(reopened.is_paused(day(27)));
        assert!(!reopened.is_paused(day(28)));
    }
}
