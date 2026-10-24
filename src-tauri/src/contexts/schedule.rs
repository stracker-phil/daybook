//! Switching contexts by their work hours. Pure.

use chrono::{Duration, NaiveDateTime};

use crate::clock::{is_within_window, parse_clock};
use crate::store::settings::Context;

/// The first context that switches automatically and whose work hours contain `now`.
pub fn scheduled(contexts: &[Context], now: NaiveDateTime) -> Option<&Context> {
    contexts.iter().find(|context| {
        context.auto_switch
            && context
                .work_hours
                .as_ref()
                .is_some_and(|hours| is_within_window(&hours.days, &hours.from, &hours.to, now))
    })
}

/// When the work hours that contain `now` started: today at their start, or yesterday for hours that
/// run past midnight.
fn started_at(context: &Context, now: NaiveDateTime) -> NaiveDateTime {
    let from = context
        .work_hours
        .as_ref()
        .and_then(|hours| parse_clock(&hours.from))
        .unwrap_or(0);
    let start = now
        .date()
        .and_hms_opt(from / 60, from % 60, 0)
        .unwrap_or(now);
    if start > now {
        start - Duration::days(1)
    } else {
        start
    }
}

/// Remembers which context's work hours contained the last check, so a context becomes active when
/// its hours start and a manual switch holds until the next start.
#[derive(Debug, Default)]
pub struct AutoSwitch {
    /// `None` until the first check since launch.
    last: Option<Option<String>>,
}

impl AutoSwitch {
    /// The context to switch to now, if any. `active` is the current context, `set_at` when it was
    /// set. On the first check, hours that started after the last switch (while the app wasn't
    /// running) still win.
    pub fn check(
        &mut self,
        contexts: &[Context],
        active: Option<&str>,
        set_at: Option<NaiveDateTime>,
        now: NaiveDateTime,
    ) -> Option<String> {
        let scheduled = scheduled(contexts, now);
        let id = scheduled.map(|context| context.id.clone());
        let Some(last) = self.last.replace(id.clone()) else {
            let context = scheduled?;
            let started_since_set = set_at.is_none_or(|set_at| started_at(context, now) > set_at);
            return (active != Some(context.id.as_str()) && started_since_set)
                .then(|| context.id.clone());
        };
        if last == id {
            return None;
        }
        id.filter(|id| active != Some(id.as_str()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use serde_json::json;

    /// 12 Oct 2026 is a Monday.
    fn at(day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    fn context(id: &str, from: &str, to: &str, days: &[u8], auto_switch: bool) -> Context {
        let raw = json!({
            "name": id,
            "id": id,
            "workHours": { "days": days, "from": from, "to": to },
            "autoSwitch": auto_switch,
        });
        Context::read_all(&[raw]).remove(0)
    }

    fn plain(id: &str) -> Context {
        Context::read_all(&[json!({ "name": id, "id": id })]).remove(0)
    }

    fn work() -> Context {
        context("work", "09:00", "17:00", &[], true)
    }

    fn night() -> Context {
        context("night", "22:00", "06:00", &[], true)
    }

    fn scheduled_id(contexts: &[Context], now: NaiveDateTime) -> Option<&str> {
        scheduled(contexts, now).map(|context| context.id.as_str())
    }

    #[test]
    fn the_context_whose_hours_contain_now_is_scheduled() {
        let contexts = [work()];
        assert_eq!(scheduled_id(&contexts, at(12, 10, 0)), Some("work"));
        assert_eq!(scheduled_id(&contexts, at(12, 8, 59)), None);
        assert_eq!(scheduled_id(&contexts, at(12, 17, 0)), None);
    }

    #[test]
    fn hours_running_past_midnight_count_before_and_after_midnight() {
        let contexts = [night()];
        assert_eq!(scheduled_id(&contexts, at(12, 23, 0)), Some("night"));
        assert_eq!(scheduled_id(&contexts, at(13, 2, 0)), Some("night"));
        assert_eq!(scheduled_id(&contexts, at(13, 6, 0)), None);
        assert_eq!(scheduled_id(&contexts, at(12, 12, 0)), None);
    }

    #[test]
    fn only_contexts_with_auto_switch_and_work_hours_count() {
        let contexts = [
            plain("plain"),
            context("manual", "00:00", "23:59", &[], false),
            work(),
        ];
        assert_eq!(scheduled_id(&contexts, at(12, 10, 0)), Some("work"));
    }

    #[test]
    fn the_first_matching_context_wins() {
        let contexts = [context("first", "08:00", "18:00", &[], true), work()];
        assert_eq!(scheduled_id(&contexts, at(12, 10, 0)), Some("first"));
    }

    #[test]
    fn work_days_are_respected() {
        let contexts = [context(
            "weekdays",
            "09:00",
            "17:00",
            &[1, 2, 3, 4, 5],
            true,
        )];
        assert_eq!(scheduled_id(&contexts, at(12, 10, 0)), Some("weekdays"));
        assert_eq!(scheduled_id(&contexts, at(17, 10, 0)), None);
    }

    #[test]
    fn the_first_check_switches_when_the_hours_started_after_the_last_switch() {
        let contexts = [work()];
        let mut auto = AutoSwitch::default();
        let switched = auto.check(&contexts, Some("other"), Some(at(12, 8, 0)), at(12, 10, 0));
        assert_eq!(switched.as_deref(), Some("work"));
    }

    #[test]
    fn the_first_check_keeps_a_switch_made_after_the_hours_started() {
        let contexts = [work()];
        let mut auto = AutoSwitch::default();
        let switched = auto.check(&contexts, Some("other"), Some(at(12, 9, 30)), at(12, 10, 0));
        assert_eq!(switched, None);
    }

    #[test]
    fn the_first_check_switches_when_nothing_was_ever_set() {
        let contexts = [work()];
        let mut auto = AutoSwitch::default();
        let switched = auto.check(&contexts, Some("other"), None, at(12, 10, 0));
        assert_eq!(switched.as_deref(), Some("work"));
    }

    #[test]
    fn the_first_check_does_not_switch_to_the_active_context() {
        let contexts = [work()];
        let mut auto = AutoSwitch::default();
        let switched = auto.check(&contexts, Some("work"), None, at(12, 10, 0));
        assert_eq!(switched, None);
    }

    #[test]
    fn the_first_check_outside_all_hours_switches_nothing() {
        let contexts = [work()];
        let mut auto = AutoSwitch::default();
        assert_eq!(
            auto.check(&contexts, Some("other"), None, at(12, 20, 0)),
            None
        );
    }

    #[test]
    fn hours_past_midnight_are_measured_from_the_evening_they_started() {
        let contexts = [night()];
        let switched = AutoSwitch::default().check(
            &contexts,
            Some("other"),
            Some(at(12, 21, 0)),
            at(13, 2, 0),
        );
        assert_eq!(switched.as_deref(), Some("night"));
        let held = AutoSwitch::default().check(
            &contexts,
            Some("other"),
            Some(at(12, 23, 0)),
            at(13, 2, 0),
        );
        assert_eq!(held, None);
    }

    #[test]
    fn later_checks_switch_when_the_scheduled_context_changed() {
        let contexts = [work()];
        let mut auto = AutoSwitch::default();
        assert_eq!(
            auto.check(&contexts, Some("other"), None, at(12, 8, 0)),
            None
        );
        let switched = auto.check(&contexts, Some("other"), None, at(12, 9, 0));
        assert_eq!(switched.as_deref(), Some("work"));
    }

    #[test]
    fn a_manual_switch_holds_until_the_next_start() {
        let contexts = [work()];
        let mut auto = AutoSwitch::default();
        auto.check(&contexts, Some("work"), None, at(12, 9, 0));
        assert_eq!(
            auto.check(&contexts, Some("other"), None, at(12, 9, 30)),
            None
        );
        assert_eq!(
            auto.check(&contexts, Some("other"), None, at(12, 16, 59)),
            None
        );
        assert_eq!(
            auto.check(&contexts, Some("other"), None, at(12, 17, 0)),
            None
        );
        let next_day = auto.check(&contexts, Some("other"), None, at(13, 9, 0));
        assert_eq!(next_day.as_deref(), Some("work"));
    }

    #[test]
    fn nothing_switches_when_hours_end() {
        let contexts = [work()];
        let mut auto = AutoSwitch::default();
        auto.check(&contexts, Some("work"), None, at(12, 16, 59));
        assert_eq!(
            auto.check(&contexts, Some("work"), None, at(12, 17, 0)),
            None
        );
    }

    #[test]
    fn a_later_check_does_not_switch_to_the_context_already_active() {
        let contexts = [work()];
        let mut auto = AutoSwitch::default();
        auto.check(&contexts, Some("work"), None, at(12, 8, 0));
        assert_eq!(
            auto.check(&contexts, Some("work"), None, at(12, 9, 0)),
            None
        );
    }
}
