//! The running call, keyed by its app's bundle id, over a series of checks. Pure.

use chrono::{NaiveDateTime, TimeDelta};

use super::detect::{Found, RuleCheck};

/// How long a missing call keeps running; meanwhile another app in a call doesn't take over.
pub const END_GRACE: TimeDelta = TimeDelta::seconds(45);
/// Consecutive failed checks of the active call's detector before the call counts as ended (~1 minute).
pub const MAX_FAILURES: u32 = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct ActiveCall {
    /// The app trigger's name, e.g. Zoom.
    pub app: String,
    /// The app trigger's bundle id; it identifies the call.
    pub bundle_id: String,
    pub title: String,
    /// When the call was first seen.
    pub since: NaiveDateTime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    /// Its app is no longer in a call.
    Gone,
    /// Detection was switched off.
    Off,
    /// Its detector kept failing.
    Failing,
}

#[derive(Clone, Debug, PartialEq)]
pub enum CallEvent {
    Started(ActiveCall),
    /// The same app's call has a new title or its trigger a new name; it keeps its start.
    Changed(ActiveCall),
    /// The call moved to another app: `from` ended at `end`, `to` runs.
    Switched {
        from: ActiveCall,
        to: ActiveCall,
        end: NaiveDateTime,
    },
    /// `end`: when the call was last seen.
    Ended {
        call: ActiveCall,
        end: NaiveDateTime,
        reason: EndReason,
    },
}

/// What a check saw.
#[derive(Clone, Debug, PartialEq)]
pub enum Seen {
    /// Detection is off: a running call ends now.
    Off,
    /// Each enabled rule's answer, in rule order.
    Checked(Vec<RuleCheck>),
}

/// How a check came about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// The regular tick: a missing call ends after the grace.
    Regular,
    /// The owner asked: a missing call ends at once.
    Asked,
    /// The first check after a sleep: a missing call ends at once, at the latest when the Mac fell asleep.
    Woke { asleep_since: NaiveDateTime },
}

/// Another app in a call while the active one is missing, and since when.
#[derive(Clone, Debug, PartialEq)]
struct Candidate {
    bundle_id: String,
    since: NaiveDateTime,
}

#[derive(Debug, Default)]
pub struct Tracker {
    active: Option<ActiveCall>,
    /// When the active call was first missing; `None` while it is seen.
    missing_since: Option<NaiveDateTime>,
    /// When the active call's detector started failing, and how often in a row.
    failing_since: Option<NaiveDateTime>,
    failures: u32,
    candidate: Option<Candidate>,
}

fn first_call(checks: &[RuleCheck]) -> Option<(&RuleCheck, &str)> {
    checks.iter().find_map(|check| match &check.found {
        Found::Call(title) => Some((check, title.as_str())),
        _ => None,
    })
}

impl Tracker {
    pub fn active(&self) -> Option<&ActiveCall> {
        self.active.as_ref()
    }

    pub fn observe(&mut self, seen: Seen, now: NaiveDateTime, mode: Mode) -> Vec<CallEvent> {
        let checks = match seen {
            Seen::Off => {
                let end = self.missing_since.unwrap_or(now);
                return self.close(end, EndReason::Off).into_iter().collect();
            }
            Seen::Checked(checks) => checks,
        };
        let Some(active) = self.active.clone() else {
            return first_call(&checks)
                .map(|(check, title)| CallEvent::Started(self.start(check, title, now)))
                .into_iter()
                .collect();
        };
        let own = checks
            .iter()
            .find(|check| check.bundle_id == active.bundle_id);
        match own {
            Some(
                check @ RuleCheck {
                    found: Found::Call(title),
                    ..
                },
            ) => self.seen(check, title),
            Some(RuleCheck {
                found: Found::Unknown(_),
                ..
            }) => self.failed(now),
            _ => self.missing(active, &checks, now, mode),
        }
    }

    fn start(&mut self, check: &RuleCheck, title: &str, since: NaiveDateTime) -> ActiveCall {
        let call = ActiveCall {
            app: check.app.clone(),
            bundle_id: check.bundle_id.clone(),
            title: title.to_string(),
            since,
        };
        *self = Self {
            active: Some(call.clone()),
            ..Self::default()
        };
        call
    }

    fn close(&mut self, end: NaiveDateTime, reason: EndReason) -> Option<CallEvent> {
        let call = std::mem::take(self).active?;
        Some(CallEvent::Ended { call, end, reason })
    }

    fn seen(&mut self, check: &RuleCheck, title: &str) -> Vec<CallEvent> {
        let Some(active) = std::mem::take(self).active else {
            return Vec::new();
        };
        let next = ActiveCall {
            app: check.app.clone(),
            title: title.to_string(),
            ..active.clone()
        };
        self.active = Some(next.clone());
        if next == active {
            Vec::new()
        } else {
            vec![CallEvent::Changed(next)]
        }
    }

    /// The call keeps running while its detector fails, up to `MAX_FAILURES` checks in a row; it ends
    /// when it was last known: its first miss or its first failure.
    fn failed(&mut self, now: NaiveDateTime) -> Vec<CallEvent> {
        self.failures += 1;
        let failing_since = *self.failing_since.get_or_insert(now);
        if self.failures < MAX_FAILURES {
            return Vec::new();
        }
        let end = self
            .missing_since
            .map_or(failing_since, |missing| missing.min(failing_since));
        self.close(end, EndReason::Failing).into_iter().collect()
    }

    fn missing(
        &mut self,
        active: ActiveCall,
        checks: &[RuleCheck],
        now: NaiveDateTime,
        mode: Mode,
    ) -> Vec<CallEvent> {
        self.failures = 0;
        self.failing_since = None;
        let missing_since = *self.missing_since.get_or_insert(now);
        let other = first_call(checks);
        let candidate = other.map(|(check, _)| candidate_for(self.candidate.as_ref(), check, now));
        self.candidate = candidate.clone();
        let Some(end) = end_time(mode, missing_since, now) else {
            return Vec::new();
        };
        match other.zip(candidate) {
            Some(((check, title), candidate)) => {
                let to = self.start(check, title, candidate.since);
                vec![CallEvent::Switched {
                    from: active,
                    to,
                    end,
                }]
            }
            None => self.close(end, EndReason::Gone).into_iter().collect(),
        }
    }
}

/// The app in a call while the active one is missing; the same app keeps its `since`.
fn candidate_for(previous: Option<&Candidate>, check: &RuleCheck, now: NaiveDateTime) -> Candidate {
    match previous {
        Some(known) if known.bundle_id == check.bundle_id => known.clone(),
        _ => Candidate {
            bundle_id: check.bundle_id.clone(),
            since: now,
        },
    }
}

/// When a missing call ended; `None` while it is within the grace.
fn end_time(mode: Mode, missing_since: NaiveDateTime, now: NaiveDateTime) -> Option<NaiveDateTime> {
    match mode {
        Mode::Regular if now - missing_since < END_GRACE => None,
        Mode::Regular | Mode::Asked => Some(missing_since),
        Mode::Woke { asleep_since } => Some(missing_since.min(asleep_since)),
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    const ZOOM: &str = "us.zoom.xos";
    const TEAMS: &str = "com.microsoft.teams2";
    const MEET: &str = "com.google.Chrome.app.meet";

    fn at(secs: i64) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 25)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap()
            + TimeDelta::seconds(secs)
    }

    fn rule_check(bundle_id: &str, app: &str, found: Found) -> RuleCheck {
        RuleCheck {
            bundle_id: bundle_id.into(),
            app: app.into(),
            found,
        }
    }

    fn zoom(found: Found) -> RuleCheck {
        rule_check(ZOOM, "Zoom", found)
    }

    fn teams(found: Found) -> RuleCheck {
        rule_check(TEAMS, "Teams", found)
    }

    fn meet(found: Found) -> RuleCheck {
        rule_check(MEET, "Meet", found)
    }

    fn in_call(title: &str) -> Found {
        Found::Call(title.into())
    }

    fn failing() -> Found {
        Found::Unknown("pgrep failed".into())
    }

    fn checked(checks: Vec<RuleCheck>) -> Seen {
        Seen::Checked(checks)
    }

    fn zoom_call(title: &str) -> Seen {
        checked(vec![zoom(in_call(title))])
    }

    fn zoom_gone() -> Seen {
        checked(vec![zoom(Found::NotInCall)])
    }

    fn zoom_failing() -> Seen {
        checked(vec![zoom(failing())])
    }

    fn zoom_gone_teams_call() -> Seen {
        checked(vec![zoom(Found::NotInCall), teams(in_call("Sync"))])
    }

    fn active(app: &str, bundle_id: &str, title: &str, since: i64) -> ActiveCall {
        ActiveCall {
            app: app.into(),
            bundle_id: bundle_id.into(),
            title: title.into(),
            since: at(since),
        }
    }

    fn zoom_active(title: &str, since: i64) -> ActiveCall {
        active("Zoom", ZOOM, title, since)
    }

    fn teams_active(title: &str, since: i64) -> ActiveCall {
        active("Teams", TEAMS, title, since)
    }

    fn ended(call: ActiveCall, end: i64, reason: EndReason) -> CallEvent {
        CallEvent::Ended {
            call,
            end: at(end),
            reason,
        }
    }

    fn running() -> Tracker {
        let mut tracker = Tracker::default();
        tracker.observe(zoom_call("Standup"), at(0), Mode::Regular);
        tracker
    }

    fn woke(asleep_since: i64) -> Mode {
        Mode::Woke {
            asleep_since: at(asleep_since),
        }
    }

    #[test]
    fn a_call_seen_starts_with_the_first_seen_time() {
        let mut tracker = Tracker::default();
        let events = tracker.observe(zoom_call("Standup"), at(0), Mode::Regular);
        assert_eq!(events, vec![CallEvent::Started(zoom_active("Standup", 0))]);
        assert_eq!(tracker.active(), Some(&zoom_active("Standup", 0)));
    }

    #[test]
    fn the_first_rule_in_a_call_in_rule_order_starts_with_its_bundle_id() {
        let mut tracker = Tracker::default();
        let seen = checked(vec![
            zoom(Found::NotInCall),
            teams(failing()),
            meet(in_call("Weekly")),
            teams(in_call("Sync")),
        ]);
        let events = tracker.observe(seen, at(7), Mode::Regular);
        assert_eq!(
            events,
            vec![CallEvent::Started(active("Meet", MEET, "Weekly", 7))]
        );
    }

    #[test]
    fn rules_that_are_not_in_a_call_or_failing_start_nothing() {
        let mut tracker = Tracker::default();
        let seen = checked(vec![zoom(Found::NotInCall), teams(failing())]);
        assert_eq!(tracker.observe(seen, at(0), Mode::Regular), vec![]);
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn seeing_the_same_call_again_gives_nothing() {
        let mut tracker = running();
        assert_eq!(
            tracker.observe(zoom_call("Standup"), at(30), Mode::Regular),
            vec![]
        );
        assert_eq!(tracker.active(), Some(&zoom_active("Standup", 0)));
    }

    #[test]
    fn the_same_app_with_a_new_title_changes_and_keeps_the_start() {
        let mut tracker = running();
        let events = tracker.observe(zoom_call("Retro"), at(60), Mode::Regular);
        assert_eq!(events, vec![CallEvent::Changed(zoom_active("Retro", 0))]);
        assert_eq!(tracker.active(), Some(&zoom_active("Retro", 0)));
    }

    #[test]
    fn a_renamed_rule_with_the_same_bundle_id_changes_and_keeps_the_start() {
        let mut tracker = running();
        let seen = checked(vec![rule_check(ZOOM, "Zoom Workplace", in_call("Standup"))]);
        let events = tracker.observe(seen, at(60), Mode::Regular);
        let renamed = active("Zoom Workplace", ZOOM, "Standup", 0);
        assert_eq!(events, vec![CallEvent::Changed(renamed.clone())]);
        assert_eq!(tracker.active(), Some(&renamed));
    }

    #[test]
    fn another_app_in_a_call_while_the_active_one_is_seen_changes_nothing() {
        let mut tracker = running();
        let seen = checked(vec![teams(in_call("Sync")), zoom(in_call("Standup"))]);
        assert_eq!(tracker.observe(seen, at(30), Mode::Regular), vec![]);
        assert_eq!(tracker.active(), Some(&zoom_active("Standup", 0)));
    }

    #[test]
    fn a_call_missing_less_than_the_grace_keeps_running() {
        let mut tracker = running();
        assert_eq!(tracker.observe(zoom_gone(), at(10), Mode::Regular), vec![]);
        assert_eq!(tracker.observe(zoom_gone(), at(54), Mode::Regular), vec![]);
        assert_eq!(tracker.active(), Some(&zoom_active("Standup", 0)));
    }

    #[test]
    fn a_call_missing_for_the_grace_ends_at_the_first_missed_time() {
        let mut tracker = running();
        tracker.observe(zoom_gone(), at(10), Mode::Regular);
        let events = tracker.observe(zoom_gone(), at(55), Mode::Regular);
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 10, EndReason::Gone)]
        );
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn a_call_missing_longer_than_the_grace_ends_at_the_first_missed_time() {
        let mut tracker = running();
        tracker.observe(zoom_gone(), at(10), Mode::Regular);
        let events = tracker.observe(zoom_gone(), at(300), Mode::Regular);
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 10, EndReason::Gone)]
        );
    }

    #[test]
    fn a_call_seen_again_within_the_grace_continues_and_a_later_miss_starts_a_fresh_grace() {
        let mut tracker = running();
        tracker.observe(zoom_gone(), at(10), Mode::Regular);
        assert_eq!(
            tracker.observe(zoom_call("Standup"), at(40), Mode::Regular),
            vec![]
        );
        assert_eq!(tracker.observe(zoom_gone(), at(60), Mode::Regular), vec![]);
        assert_eq!(tracker.observe(zoom_gone(), at(100), Mode::Regular), vec![]);
        let events = tracker.observe(zoom_gone(), at(105), Mode::Regular);
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 60, EndReason::Gone)]
        );
    }

    #[test]
    fn after_the_grace_another_app_in_a_call_takes_over_from_the_first_miss() {
        let mut tracker = running();
        tracker.observe(zoom_gone(), at(10), Mode::Regular);
        let events = tracker.observe(zoom_gone_teams_call(), at(55), Mode::Regular);
        assert_eq!(
            events,
            vec![CallEvent::Switched {
                from: zoom_active("Standup", 0),
                to: teams_active("Sync", 55),
                end: at(10),
            }]
        );
        assert_eq!(tracker.active(), Some(&teams_active("Sync", 55)));
    }

    #[test]
    fn the_new_call_starts_when_its_app_was_first_seen_during_the_grace() {
        let mut tracker = running();
        tracker.observe(zoom_gone(), at(10), Mode::Regular);
        tracker.observe(zoom_gone_teams_call(), at(20), Mode::Regular);
        let events = tracker.observe(zoom_gone_teams_call(), at(55), Mode::Regular);
        assert_eq!(
            events,
            vec![CallEvent::Switched {
                from: zoom_active("Standup", 0),
                to: teams_active("Sync", 20),
                end: at(10),
            }]
        );
    }

    #[test]
    fn a_different_app_showing_up_during_the_grace_restarts_the_new_calls_start() {
        let mut tracker = running();
        tracker.observe(zoom_gone(), at(10), Mode::Regular);
        let meet_call = checked(vec![zoom(Found::NotInCall), meet(in_call("Weekly"))]);
        tracker.observe(meet_call, at(20), Mode::Regular);
        tracker.observe(zoom_gone_teams_call(), at(30), Mode::Regular);
        let events = tracker.observe(zoom_gone_teams_call(), at(55), Mode::Regular);
        assert_eq!(
            events,
            vec![CallEvent::Switched {
                from: zoom_active("Standup", 0),
                to: teams_active("Sync", 30),
                end: at(10),
            }]
        );
    }

    #[test]
    fn another_app_in_a_call_during_the_grace_gives_no_events() {
        let mut tracker = running();
        assert_eq!(tracker.observe(zoom_gone(), at(10), Mode::Regular), vec![]);
        assert_eq!(
            tracker.observe(zoom_gone_teams_call(), at(30), Mode::Regular),
            vec![]
        );
        assert_eq!(tracker.active(), Some(&zoom_active("Standup", 0)));
    }

    #[test]
    fn the_active_call_seen_again_cancels_the_grace_and_the_other_app_as_candidate() {
        let mut tracker = running();
        tracker.observe(zoom_gone(), at(10), Mode::Regular);
        tracker.observe(zoom_gone_teams_call(), at(20), Mode::Regular);
        assert_eq!(
            tracker.observe(zoom_call("Standup"), at(40), Mode::Regular),
            vec![]
        );
        tracker.observe(zoom_gone_teams_call(), at(50), Mode::Regular);
        let events = tracker.observe(zoom_gone_teams_call(), at(95), Mode::Regular);
        assert_eq!(
            events,
            vec![CallEvent::Switched {
                from: zoom_active("Standup", 0),
                to: teams_active("Sync", 50),
                end: at(50),
            }]
        );
    }

    #[test]
    fn an_asked_check_switches_to_the_other_app_at_once() {
        let mut tracker = running();
        let events = tracker.observe(zoom_gone_teams_call(), at(5), Mode::Asked);
        assert_eq!(
            events,
            vec![CallEvent::Switched {
                from: zoom_active("Standup", 0),
                to: teams_active("Sync", 5),
                end: at(5),
            }]
        );
        assert_eq!(tracker.active(), Some(&teams_active("Sync", 5)));
    }

    #[test]
    fn an_asked_check_ends_a_missing_call_at_once() {
        let mut tracker = running();
        let events = tracker.observe(zoom_gone(), at(5), Mode::Asked);
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 5, EndReason::Gone)]
        );
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn an_asked_check_during_a_grace_ends_at_the_first_missed_time() {
        let mut tracker = running();
        tracker.observe(zoom_gone(), at(10), Mode::Regular);
        let events = tracker.observe(zoom_gone(), at(20), Mode::Asked);
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 10, EndReason::Gone)]
        );
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn an_asked_check_still_seeing_the_call_keeps_it() {
        let mut tracker = running();
        assert_eq!(
            tracker.observe(zoom_call("Standup"), at(5), Mode::Asked),
            vec![]
        );
        assert_eq!(tracker.active(), Some(&zoom_active("Standup", 0)));
    }

    #[test]
    fn a_check_after_a_sleep_ends_a_missing_call_when_the_mac_fell_asleep() {
        let mut tracker = running();
        let events = tracker.observe(zoom_gone(), at(600), woke(100));
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 100, EndReason::Gone)]
        );
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn a_check_after_a_sleep_ends_at_the_first_miss_when_that_was_before_the_sleep() {
        let mut tracker = running();
        tracker.observe(zoom_gone(), at(10), Mode::Regular);
        let events = tracker.observe(zoom_gone(), at(600), woke(100));
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 10, EndReason::Gone)]
        );
    }

    #[test]
    fn a_check_after_a_sleep_switches_to_the_other_app_at_once() {
        let mut tracker = running();
        let events = tracker.observe(zoom_gone_teams_call(), at(600), woke(100));
        assert_eq!(
            events,
            vec![CallEvent::Switched {
                from: zoom_active("Standup", 0),
                to: teams_active("Sync", 600),
                end: at(100),
            }]
        );
    }

    #[test]
    fn a_check_after_a_sleep_still_seeing_the_call_keeps_it() {
        let mut tracker = running();
        assert_eq!(
            tracker.observe(zoom_call("Standup"), at(600), woke(100)),
            vec![]
        );
        assert_eq!(tracker.active(), Some(&zoom_active("Standup", 0)));
    }

    #[test]
    fn a_detector_failing_fewer_than_four_checks_in_a_row_keeps_the_call() {
        let mut tracker = running();
        for secs in [10, 20, 30] {
            assert_eq!(
                tracker.observe(zoom_failing(), at(secs), Mode::Regular),
                vec![],
                "{secs}"
            );
        }
        assert_eq!(tracker.active(), Some(&zoom_active("Standup", 0)));
    }

    #[test]
    fn the_fourth_failed_check_in_a_row_ends_the_call_at_the_first_failure() {
        let mut tracker = running();
        for secs in [10, 20, 30] {
            tracker.observe(zoom_failing(), at(secs), Mode::Regular);
        }
        let events = tracker.observe(zoom_failing(), at(40), Mode::Regular);
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 10, EndReason::Failing)]
        );
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn a_failing_detector_after_a_miss_ends_the_call_at_the_first_miss() {
        let mut tracker = running();
        tracker.observe(zoom_gone(), at(5), Mode::Regular);
        for secs in [10, 20, 30] {
            tracker.observe(zoom_failing(), at(secs), Mode::Regular);
        }
        let events = tracker.observe(zoom_failing(), at(40), Mode::Regular);
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 5, EndReason::Failing)]
        );
    }

    #[test]
    fn a_call_seen_between_failures_resets_the_count() {
        let mut tracker = running();
        for secs in [10, 20, 30] {
            tracker.observe(zoom_failing(), at(secs), Mode::Regular);
        }
        assert_eq!(
            tracker.observe(zoom_call("Standup"), at(40), Mode::Regular),
            vec![]
        );
        for secs in [50, 60, 70] {
            assert_eq!(
                tracker.observe(zoom_failing(), at(secs), Mode::Regular),
                vec![],
                "{secs}"
            );
        }
        let events = tracker.observe(zoom_failing(), at(80), Mode::Regular);
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 50, EndReason::Failing)]
        );
    }

    #[test]
    fn a_miss_between_failures_resets_the_count() {
        let mut tracker = running();
        for secs in [10, 20, 30] {
            tracker.observe(zoom_failing(), at(secs), Mode::Regular);
        }
        tracker.observe(zoom_gone(), at(40), Mode::Regular);
        for secs in [50, 60, 70] {
            assert_eq!(
                tracker.observe(zoom_failing(), at(secs), Mode::Regular),
                vec![],
                "{secs}"
            );
        }
    }

    #[test]
    fn another_rules_failing_detector_never_affects_the_active_call() {
        let mut tracker = running();
        for secs in [10, 20, 30, 40, 50, 60] {
            let seen = checked(vec![zoom(in_call("Standup")), teams(failing())]);
            assert_eq!(
                tracker.observe(seen, at(secs), Mode::Regular),
                vec![],
                "{secs}"
            );
        }
        assert_eq!(tracker.active(), Some(&zoom_active("Standup", 0)));
    }

    #[test]
    fn switching_detection_off_ends_a_running_call_at_once() {
        let mut tracker = running();
        let events = tracker.observe(Seen::Off, at(5), Mode::Regular);
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 5, EndReason::Off)]
        );
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn switching_detection_off_ends_a_missing_call_at_its_first_miss() {
        let mut tracker = running();
        tracker.observe(zoom_gone(), at(10), Mode::Regular);
        let events = tracker.observe(Seen::Off, at(30), Mode::Regular);
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 10, EndReason::Off)]
        );
    }

    #[test]
    fn switching_detection_off_with_nothing_running_gives_nothing() {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.observe(Seen::Off, at(0), Mode::Regular), vec![]);
        assert_eq!(tracker.active(), None);
    }

    #[test]
    fn a_rule_removed_during_a_call_counts_as_missing() {
        let mut tracker = running();
        let without_zoom = || checked(vec![teams(Found::NotInCall)]);
        assert_eq!(
            tracker.observe(without_zoom(), at(10), Mode::Regular),
            vec![]
        );
        let events = tracker.observe(without_zoom(), at(55), Mode::Regular);
        assert_eq!(
            events,
            vec![ended(zoom_active("Standup", 0), 10, EndReason::Gone)]
        );
    }

    #[test]
    fn a_rule_disabled_during_a_call_lets_another_app_take_over_after_the_grace() {
        let mut tracker = running();
        let without_zoom = || checked(vec![teams(in_call("Sync"))]);
        assert_eq!(
            tracker.observe(without_zoom(), at(10), Mode::Regular),
            vec![]
        );
        let events = tracker.observe(without_zoom(), at(55), Mode::Regular);
        assert_eq!(
            events,
            vec![CallEvent::Switched {
                from: zoom_active("Standup", 0),
                to: teams_active("Sync", 10),
                end: at(10),
            }]
        );
    }

    #[test]
    fn no_call_and_nothing_seen_gives_nothing() {
        let mut tracker = Tracker::default();
        assert_eq!(tracker.observe(zoom_gone(), at(0), Mode::Regular), vec![]);
        assert_eq!(tracker.observe(zoom_gone(), at(100), Mode::Asked), vec![]);
        assert_eq!(tracker.observe(zoom_gone(), at(200), woke(150)), vec![]);
        assert_eq!(tracker.active(), None);
    }
}
