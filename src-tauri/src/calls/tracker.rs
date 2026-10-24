//! The running call over a series of checks. Pure.
//!
//! A call ends only after it was missing for `END_GRACE`, so one missed check never splits it; a check
//! the owner asked for ends a missing call at once.

use chrono::{NaiveDateTime, TimeDelta};

use super::detect::Detected;

pub const END_GRACE: TimeDelta = TimeDelta::seconds(45);

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveCall {
    /// The app trigger's name, e.g. Zoom.
    pub app: String,
    pub title: String,
    /// When the call was first seen.
    pub since: NaiveDateTime,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CallEvent {
    Started(ActiveCall),
    /// The same app's call has a new title.
    Changed(ActiveCall),
    /// `end`: when the call was last seen.
    Ended {
        call: ActiveCall,
        end: NaiveDateTime,
    },
}

/// What a check saw.
#[derive(Clone, Debug, PartialEq)]
pub enum Seen {
    /// Detection is off: a running call ends now.
    Off,
    Call(Option<Detected>),
}

#[derive(Debug, Default)]
pub struct Tracker {
    active: Option<ActiveCall>,
    /// When the active call was first missing; `None` while it is seen.
    missing_since: Option<NaiveDateTime>,
}

impl Tracker {
    pub fn active(&self) -> Option<&ActiveCall> {
        self.active.as_ref()
    }

    /// `asked`: the owner asked for the check, so a call that is gone ends without the grace.
    pub fn observe(&mut self, seen: Seen, now: NaiveDateTime, asked: bool) -> Vec<CallEvent> {
        let found = match seen {
            Seen::Off => None,
            Seen::Call(found) => {
                if found.is_none() && self.active.is_some() && !asked {
                    let since = *self.missing_since.get_or_insert(now);
                    if now - since < END_GRACE {
                        return Vec::new();
                    }
                }
                found
            }
        };
        let end = self.missing_since.take().unwrap_or(now);
        let next = found.map(|found| ActiveCall {
            since: self.started_at(&found.app, now),
            app: found.app,
            title: found.title,
        });
        if self.active == next {
            return Vec::new();
        }
        let previous = std::mem::replace(&mut self.active, next.clone());
        match (previous, next) {
            (Some(previous), Some(next)) if previous.app == next.app => {
                vec![CallEvent::Changed(next)]
            }
            (previous, next) => {
                let mut events = Vec::new();
                if let Some(call) = previous {
                    events.push(CallEvent::Ended { call, end });
                }
                if let Some(call) = next {
                    events.push(CallEvent::Started(call));
                }
                events
            }
        }
    }

    /// The same app's call keeps its start when only the title changes.
    fn started_at(&self, app: &str, now: NaiveDateTime) -> NaiveDateTime {
        match &self.active {
            Some(active) if active.app == app => active.since,
            _ => now,
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn at(secs: i64) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 25)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap()
            + TimeDelta::seconds(secs)
    }

    fn call(app: &str, title: &str) -> Seen {
        Seen::Call(Some(Detected {
            app: app.into(),
            title: title.into(),
        }))
    }

    fn none() -> Seen {
        Seen::Call(None)
    }

    fn active(app: &str, title: &str, since: i64) -> ActiveCall {
        ActiveCall {
            app: app.into(),
            title: title.into(),
            since: at(since),
        }
    }

    fn running(app: &str, title: &str) -> Tracker {
        let mut tracker = Tracker::default();
        tracker.observe(call(app, title), at(0), false);
        tracker
    }

    #[test]
    fn a_call_seen_starts_with_the_first_seen_time() {
        let mut tracker = Tracker::default();
        let events = tracker.observe(call("Zoom", "Standup"), at(0), false);
        assert_eq!(
            events,
            vec![CallEvent::Started(active("Zoom", "Standup", 0))]
        );
        assert_eq!(tracker.active(), Some(&active("Zoom", "Standup", 0)));
    }

    #[test]
    fn seeing_the_same_call_again_gives_nothing() {
        let mut tracker = running("Zoom", "Standup");
        assert_eq!(
            tracker.observe(call("Zoom", "Standup"), at(30), false),
            vec![]
        );
        assert_eq!(tracker.active(), Some(&active("Zoom", "Standup", 0)));
    }

    #[test]
    fn the_same_app_with_a_new_title_changes_and_keeps_the_start() {
        let mut tracker = running("Zoom", "Standup");
        let events = tracker.observe(call("Zoom", "Retro"), at(60), false);
        assert_eq!(events, vec![CallEvent::Changed(active("Zoom", "Retro", 0))]);
        assert_eq!(tracker.active(), Some(&active("Zoom", "Retro", 0)));
    }

    #[test]
    fn another_apps_call_ends_the_old_one_now_and_starts_the_new_one() {
        let mut tracker = running("Zoom", "Standup");
        let events = tracker.observe(call("Teams", "Sync"), at(60), false);
        assert_eq!(
            events,
            vec![
                CallEvent::Ended {
                    call: active("Zoom", "Standup", 0),
                    end: at(60),
                },
                CallEvent::Started(active("Teams", "Sync", 60)),
            ]
        );
        assert_eq!(tracker.active(), Some(&active("Teams", "Sync", 60)));
    }

    #[test]
    fn a_call_missing_less_than_the_grace_keeps_running() {
        let mut tracker = running("Zoom", "Standup");
        assert_eq!(tracker.observe(none(), at(10), false), vec![]);
        assert_eq!(tracker.observe(none(), at(54), false), vec![]);
        assert_eq!(tracker.active(), Some(&active("Zoom", "Standup", 0)));
    }

    #[test]
    fn a_call_missing_for_the_grace_ends_at_the_first_missed_time() {
        let mut tracker = running("Zoom", "Standup");
        tracker.observe(none(), at(10), false);
        let events = tracker.observe(none(), at(55), false);
        assert_eq!(
            events,
            vec![CallEvent::Ended {
                call: active("Zoom", "Standup", 0),
                end: at(10),
            }]
        );
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn a_call_missing_longer_than_the_grace_ends_at_the_first_missed_time() {
        let mut tracker = running("Zoom", "Standup");
        tracker.observe(none(), at(10), false);
        let events = tracker.observe(none(), at(300), false);
        assert_eq!(
            events,
            vec![CallEvent::Ended {
                call: active("Zoom", "Standup", 0),
                end: at(10),
            }]
        );
    }

    #[test]
    fn a_call_seen_again_within_the_grace_continues_and_a_later_miss_starts_a_fresh_grace() {
        let mut tracker = running("Zoom", "Standup");
        tracker.observe(none(), at(10), false);
        assert_eq!(
            tracker.observe(call("Zoom", "Standup"), at(40), false),
            vec![]
        );
        assert_eq!(tracker.observe(none(), at(60), false), vec![]);
        assert_eq!(tracker.observe(none(), at(100), false), vec![]);
        let events = tracker.observe(none(), at(105), false);
        assert_eq!(
            events,
            vec![CallEvent::Ended {
                call: active("Zoom", "Standup", 0),
                end: at(60),
            }]
        );
    }

    #[test]
    fn an_asked_check_ends_a_missing_call_at_once() {
        let mut tracker = running("Zoom", "Standup");
        let events = tracker.observe(none(), at(5), true);
        assert_eq!(
            events,
            vec![CallEvent::Ended {
                call: active("Zoom", "Standup", 0),
                end: at(5),
            }]
        );
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn an_asked_check_during_a_grace_ends_at_the_first_missed_time() {
        let mut tracker = running("Zoom", "Standup");
        tracker.observe(none(), at(10), false);
        let events = tracker.observe(none(), at(20), true);
        assert_eq!(
            events,
            vec![CallEvent::Ended {
                call: active("Zoom", "Standup", 0),
                end: at(10),
            }]
        );
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn switching_detection_off_ends_a_running_call_at_once() {
        let mut tracker = running("Zoom", "Standup");
        let events = tracker.observe(Seen::Off, at(5), false);
        assert_eq!(
            events,
            vec![CallEvent::Ended {
                call: active("Zoom", "Standup", 0),
                end: at(5),
            }]
        );
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn switching_detection_off_with_nothing_running_gives_nothing() {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.observe(Seen::Off, at(0), false), vec![]);
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn no_call_and_nothing_seen_gives_nothing() {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.observe(none(), at(0), false), vec![]);
        assert_eq!(tracker.observe(none(), at(100), true), vec![]);
        assert_eq!(tracker.active(), None);
    }
}
