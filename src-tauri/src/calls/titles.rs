//! The calendar title of a call its app calls just "Meeting", looked up once per event instead of every
//! check. Pure.

use chrono::{NaiveDateTime, TimeDelta};

/// Without a running event the calendar is asked again after this long.
const RECHECK_WITHOUT_EVENT: TimeDelta = TimeDelta::minutes(5);

#[derive(Debug)]
struct Cached {
    bundle_id: String,
    title: Option<String>,
    until: NaiveDateTime,
}

/// The event title last found for each app's call, until that event ends.
#[derive(Debug, Default)]
pub struct EventTitles {
    cached: Vec<Cached>,
}

impl EventTitles {
    /// The remembered event title (`Some(None)`: no event) while it holds; `None` when the calendar has
    /// to be asked.
    pub fn get(&self, bundle_id: &str, now: NaiveDateTime) -> Option<Option<String>> {
        self.cached
            .iter()
            .find(|cached| cached.bundle_id == bundle_id && now < cached.until)
            .map(|cached| cached.title.clone())
    }

    /// Remembers the event running now (its title and end) for the app's call.
    pub fn remember(
        &mut self,
        bundle_id: &str,
        event: Option<(&str, NaiveDateTime)>,
        now: NaiveDateTime,
    ) {
        let (title, until) = match event {
            Some((title, end)) => (Some(title.to_string()), end),
            None => (None, now + RECHECK_WITHOUT_EVENT),
        };
        self.cached
            .retain(|cached| cached.bundle_id != bundle_id && now < cached.until);
        self.cached.push(Cached {
            bundle_id: bundle_id.to_string(),
            title,
            until,
        });
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    const ZOOM: &str = "us.zoom.xos";
    const TEAMS: &str = "com.microsoft.teams2";

    fn at(minutes: i64) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 25)
            .unwrap()
            .and_hms_opt(10, 0, 0)
            .unwrap()
            + TimeDelta::minutes(minutes)
    }

    #[test]
    fn nothing_is_known_before_the_calendar_was_asked() {
        assert_eq!(EventTitles::default().get(ZOOM, at(0)), None);
    }

    #[test]
    fn an_event_title_holds_until_the_event_ends() {
        let mut titles = EventTitles::default();
        titles.remember(ZOOM, Some(("Planning", at(30))), at(0));
        assert_eq!(titles.get(ZOOM, at(0)), Some(Some("Planning".into())));
        assert_eq!(titles.get(ZOOM, at(29)), Some(Some("Planning".into())));
    }

    #[test]
    fn the_calendar_is_asked_again_once_the_event_has_ended() {
        let mut titles = EventTitles::default();
        titles.remember(ZOOM, Some(("Planning", at(30))), at(0));
        assert_eq!(titles.get(ZOOM, at(30)), None);
        assert_eq!(titles.get(ZOOM, at(45)), None);
    }

    #[test]
    fn no_event_is_remembered_for_five_minutes() {
        let mut titles = EventTitles::default();
        titles.remember(ZOOM, None, at(0));
        assert_eq!(titles.get(ZOOM, at(0)), Some(None));
        assert_eq!(titles.get(ZOOM, at(4)), Some(None));
        assert_eq!(titles.get(ZOOM, at(5)), None);
    }

    #[test]
    fn each_app_keeps_its_own_entry() {
        let mut titles = EventTitles::default();
        titles.remember(ZOOM, Some(("Planning", at(30))), at(0));
        titles.remember(TEAMS, None, at(1));
        assert_eq!(titles.get(ZOOM, at(2)), Some(Some("Planning".into())));
        assert_eq!(titles.get(TEAMS, at(2)), Some(None));
        assert_eq!(titles.get("com.example.other", at(2)), None);
    }

    #[test]
    fn remembering_again_replaces_the_apps_entry() {
        let mut titles = EventTitles::default();
        titles.remember(ZOOM, Some(("Planning", at(30))), at(0));
        titles.remember(ZOOM, Some(("Retro", at(90))), at(31));
        assert_eq!(titles.get(ZOOM, at(32)), Some(Some("Retro".into())));

        titles.remember(ZOOM, None, at(33));
        assert_eq!(titles.get(ZOOM, at(34)), Some(None));
    }
}
