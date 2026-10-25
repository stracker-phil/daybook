//! Calendar events as plain data: which events count, the one running now and the call an event links
//! to. Pure.

use std::sync::LazyLock;

use chrono::NaiveDateTime;
use regex::Regex;
use serde::Serialize;
use ts_rs::TS;

/// A timed event of the shown day, in local time.
#[derive(Clone, Debug, PartialEq)]
pub struct Event {
    pub title: String,
    pub calendar: String,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
    pub url: Option<String>,
    pub location: Option<String>,
    pub notes: Option<String>,
}

/// An event as EventKit returns it.
#[derive(Clone, Debug, PartialEq)]
pub struct CalendarEvent {
    pub event: Event,
    pub all_day: bool,
    pub canceled: bool,
    /// Declined by the owner.
    pub declined: bool,
    /// Shows the owner as busy.
    pub busy: bool,
}

impl CalendarEvent {
    /// Neither canceled nor declined.
    pub fn takes_place(&self) -> bool {
        !self.canceled && !self.declined
    }
}

/// The events that count: timed, not canceled, not declined; by start.
pub fn timed_events(events: Vec<CalendarEvent>) -> Vec<Event> {
    let mut timed: Vec<Event> = events
        .into_iter()
        .filter(|e| !e.all_day && e.takes_place())
        .map(|e| e.event)
        .collect();
    timed.sort_by_key(|event| event.start);
    timed
}

/// The event running at `now`: the latest started one.
pub fn running_event(events: &[Event], now: NaiveDateTime) -> Option<&Event> {
    events
        .iter()
        .filter(|event| event.start <= now && now < event.end)
        .max_by_key(|event| event.start)
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub enum CallApp {
    Zoom,
    Teams,
    Meet,
}

impl CallApp {
    pub fn name(self) -> &'static str {
        match self {
            Self::Zoom => "Zoom",
            Self::Teams => "Teams",
            Self::Meet => "Meet",
        }
    }
}

/// The address that opens a call.
#[derive(Serialize, Clone, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct MeetingLink {
    pub app: CallApp,
    pub url: String,
}

/// Links that open the call itself, not its dial-in numbers, options or help pages.
static JOIN_LINKS: LazyLock<[(CallApp, Regex); 3]> = LazyLock::new(|| {
    const LINK_END: &str = r#"[^\s<>"')\]]"#;
    let pattern = |source: String| Regex::new(&format!("(?i){source}")).expect("join link pattern");
    [
        (
            CallApp::Zoom,
            pattern(format!(
                r"https://(?:[\w-]+\.)*zoom\.us/(?:j|my|w|s)/{LINK_END}+"
            )),
        ),
        (
            CallApp::Teams,
            pattern(format!(
                r"https://teams\.(?:microsoft\.com/(?:l/meetup-join|meet)|live\.com/meet)/{LINK_END}+"
            )),
        ),
        (
            CallApp::Meet,
            pattern(r"https://meet\.google\.com/(?:lookup/)?[a-z0-9][a-z0-9-]+".into()),
        ),
    ]
});

/// Google wraps links in notes in a redirect (`…&sa=D&source=calendar…`): the link without the
/// redirect's parameters, decoded.
fn unwrapped(url: &str) -> String {
    let Some(at) = url.find("&sa=D") else {
        return url.to_string();
    };
    let inner = &url[..at];
    percent_encoding::percent_decode_str(inner)
        .decode_utf8()
        .map_or_else(|_| inner.to_string(), |decoded| decoded.into_owned())
}

/// The call an event links to: the first join link in its link, location or notes (often HTML).
pub fn meeting_link(
    url: Option<&str>,
    location: Option<&str>,
    notes: Option<&str>,
) -> Option<MeetingLink> {
    let text = [url, location, notes]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .replace("&amp;", "&");
    JOIN_LINKS
        .iter()
        .filter_map(|(app, pattern)| {
            pattern
                .find(&text)
                .map(|found| (found.start(), *app, found.as_str()))
        })
        .min_by_key(|(at, ..)| *at)
        .map(|(_, app, url)| MeetingLink {
            app,
            url: unwrapped(url),
        })
}

pub fn event_meeting_link(event: &Event) -> Option<MeetingLink> {
    meeting_link(
        event.url.as_deref(),
        event.location.as_deref(),
        event.notes.as_deref(),
    )
}

/// The call of an address that is nothing but a join link: the only kind Daybook opens for an event.
pub fn join_link(url: &str) -> Option<MeetingLink> {
    meeting_link(Some(url), None, None).filter(|link| link.url == url)
}

pub fn is_meeting_url(url: &str) -> bool {
    join_link(url).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 12)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    fn event(title: &str, start: NaiveDateTime, end: NaiveDateTime) -> Event {
        Event {
            title: title.into(),
            calendar: "Work".into(),
            start,
            end,
            url: None,
            location: None,
            notes: None,
        }
    }

    fn calendar_event(title: &str, start: NaiveDateTime) -> CalendarEvent {
        CalendarEvent {
            event: event(title, start, start + chrono::Duration::hours(1)),
            all_day: false,
            canceled: false,
            declined: false,
            busy: false,
        }
    }

    #[test]
    fn an_event_takes_place_unless_it_is_canceled_or_declined() {
        for (canceled, declined, expected) in [
            (false, false, true),
            (true, false, false),
            (false, true, false),
            (true, true, false),
        ] {
            let event = CalendarEvent {
                canceled,
                declined,
                ..calendar_event("Standup", at(9, 0))
            };
            assert_eq!(event.takes_place(), expected, "{canceled} {declined}");
        }
    }

    fn titles(events: &[Event]) -> Vec<&str> {
        events.iter().map(|e| e.title.as_str()).collect()
    }

    fn notes_link(notes: &str) -> Option<MeetingLink> {
        meeting_link(None, None, Some(notes))
    }

    fn link(app: CallApp, url: &str) -> Option<MeetingLink> {
        Some(MeetingLink {
            app,
            url: url.into(),
        })
    }

    #[test]
    fn timed_events_drops_all_day_canceled_and_declined_events() {
        let all_day = CalendarEvent {
            all_day: true,
            ..calendar_event("Holiday", at(0, 0))
        };
        let canceled = CalendarEvent {
            canceled: true,
            ..calendar_event("Canceled", at(9, 0))
        };
        let declined = CalendarEvent {
            declined: true,
            ..calendar_event("Declined", at(10, 0))
        };
        let kept = calendar_event("Kept", at(11, 0));
        let result = timed_events(vec![all_day, canceled, declined, kept.clone()]);
        assert_eq!(result, vec![kept.event]);
    }

    #[test]
    fn timed_events_are_sorted_by_start_regardless_of_input_order() {
        let result = timed_events(vec![
            calendar_event("Late", at(14, 0)),
            calendar_event("Early", at(8, 0)),
            calendar_event("Middle", at(10, 30)),
        ]);
        assert_eq!(titles(&result), ["Early", "Middle", "Late"]);
    }

    #[test]
    fn the_running_event_is_the_latest_started_one() {
        let standup = event("Standup", at(9, 0), at(9, 30));
        let planning = event("Planning", at(10, 0), at(11, 0));
        let overlapping = event("Overlapping", at(10, 20), at(12, 0));

        let two = [standup.clone(), planning.clone()];
        assert_eq!(
            running_event(&two, at(10, 50)).map(|e| e.title.as_str()),
            Some("Planning")
        );

        let three = [standup, overlapping, planning];
        assert_eq!(
            running_event(&three, at(10, 50)).map(|e| e.title.as_str()),
            Some("Overlapping")
        );
        assert_eq!(running_event(&three, at(9, 50)), None);
    }

    #[test]
    fn an_event_runs_from_its_start_up_to_but_excluding_its_end() {
        let events = [event("Planning", at(10, 0), at(11, 0))];
        assert!(running_event(&events, at(10, 0)).is_some());
        assert!(running_event(&events, at(10, 59)).is_some());
        assert_eq!(running_event(&events, at(11, 0)), None);
        assert_eq!(running_event(&events, at(9, 59)), None);
    }

    #[test]
    fn the_join_link_is_found_in_a_google_redirect_with_html_entities() {
        let notes = "Join: <a href=\"https://www.google.com/url?q=https://us02web.zoom.us/j/12345678901?jst%3D2&amp;sa=D&amp;source=calendar&amp;usg=AOv\">https://us02web.zoom.us/j/12345678901?jst=2</a> \
                     <a href=\"https://support.zoom.us/hc/en-us/articles/201362193\">Help</a>";
        assert_eq!(
            notes_link(notes),
            link(CallApp::Zoom, "https://us02web.zoom.us/j/12345678901?jst=2")
        );
    }

    #[test]
    fn ampersand_entities_in_a_join_link_are_decoded() {
        assert_eq!(
            notes_link("https://zoom.us/j/1?pwd=abc&amp;omn=2 Passcode: 1"),
            link(CallApp::Zoom, "https://zoom.us/j/1?pwd=abc&omn=2")
        );
    }

    #[test]
    fn the_teams_join_link_wins_over_help_dial_in_and_options_links() {
        let notes = "Need help?<https://aka.ms/JoinTeamsMeeting?omkt=en-US> Join the meeting now<https://teams.microsoft.com/l/meetup-join/19%3ameeting_x%40thread.v2/0?context=%7b%7d> \
                     Dial in<https://dialin.teams.microsoft.com/813?id=4> Options<https://teams.microsoft.com/meetingOptions/?organizerId=1>";
        assert_eq!(
            meeting_link(None, Some("Microsoft Teams Meeting"), Some(notes)),
            link(
                CallApp::Teams,
                "https://teams.microsoft.com/l/meetup-join/19%3ameeting_x%40thread.v2/0?context=%7b%7d"
            )
        );
    }

    #[test]
    fn a_meet_link_in_an_anchor_is_found() {
        assert_eq!(
            notes_link("<a href=\"https://meet.google.com/xyz-abcd-efg\">Join</a>"),
            link(CallApp::Meet, "https://meet.google.com/xyz-abcd-efg")
        );
    }

    #[test]
    fn a_help_page_is_not_a_join_link() {
        assert_eq!(notes_link("<p>Agenda</p> https://support.zoom.us/hc"), None);
    }

    #[test]
    fn the_call_app_follows_from_the_text_the_link_is_in() {
        assert_eq!(
            meeting_link(Some("https://zoom.us/j/1"), None, None).map(|l| l.app),
            Some(CallApp::Zoom)
        );
        assert_eq!(
            meeting_link(
                None,
                Some("https://teams.microsoft.com/l/meetup-join/x"),
                None
            )
            .map(|l| l.app),
            Some(CallApp::Teams)
        );
        assert_eq!(
            notes_link("Join: https://meet.google.com/abc-defg-hij").map(|l| l.app),
            Some(CallApp::Meet)
        );
        assert_eq!(meeting_link(None, None, None), None);
    }

    #[test]
    fn the_earliest_link_in_the_combined_text_wins() {
        assert_eq!(
            meeting_link(
                Some("https://meet.google.com/abc-defg-hij"),
                None,
                Some("https://zoom.us/j/1")
            ),
            link(CallApp::Meet, "https://meet.google.com/abc-defg-hij")
        );
    }

    #[test]
    fn an_events_meeting_link_is_read_from_its_own_fields() {
        let with_link = Event {
            url: Some("https://zoom.us/j/42".into()),
            ..event("Sync", at(9, 0), at(10, 0))
        };
        assert_eq!(
            event_meeting_link(&with_link),
            link(CallApp::Zoom, "https://zoom.us/j/42")
        );
        assert_eq!(
            event_meeting_link(&event("Lunch", at(12, 0), at(13, 0))),
            None
        );
    }

    #[test]
    fn only_a_whole_join_link_counts_as_a_meeting_url() {
        for (input, expected) in [
            ("https://meet.google.com/xyz-abcd-efg", true),
            ("https://example.com/?next=https://zoom.us/j/1", false),
            ("file:///etc/passwd", false),
        ] {
            assert_eq!(is_meeting_url(input), expected, "{input}");
        }
    }

    #[test]
    fn a_bare_join_link_gives_its_call_and_the_same_url() {
        for (app, url) in [
            (CallApp::Zoom, "https://zoom.us/j/123456789?pwd=abc"),
            (CallApp::Meet, "https://meet.google.com/xyz-abcd-efg"),
            (
                CallApp::Teams,
                "https://teams.microsoft.com/l/meetup-join/19%3ameeting",
            ),
        ] {
            assert_eq!(join_link(url), link(app, url), "{url}");
        }
    }

    #[test]
    fn text_around_a_link_or_a_non_join_address_is_no_join_link() {
        for input in [
            "Join https://zoom.us/j/123456789",
            "https://zoom.us/j/123456789 and more",
            "https://support.zoom.us/hc/en-us/articles/201362193",
            "https://example.com/?next=https://zoom.us/j/1",
            "",
        ] {
            assert_eq!(join_link(input), None, "{input}");
        }
    }
}
