//! Which links show now: every rule a link sets must match, and an event link's window must be open.
//! Project rules match nothing while no current project is known (TimeTracker's timer runs on none).
//! Pure.

use chrono::{Datelike, Duration, NaiveDateTime, Timelike};

use crate::calendar::Event;
use crate::search::posix::basename;
use crate::store::settings::{EventWindow, QuickLink};
use crate::tools::Link;

/// A calendar event of today, as the event rules read it.
#[derive(Clone, Debug, PartialEq)]
pub struct EventSpan {
    pub title: String,
    pub start: NaiveDateTime,
    pub end: NaiveDateTime,
}

impl From<&Event> for EventSpan {
    fn from(event: &Event) -> Self {
        Self {
            title: event.title.clone(),
            start: event.start,
            end: event.end,
        }
    }
}

/// The links that match now, then every other one, each in Settings order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuickAccess {
    pub now: Vec<Link>,
    pub more: Vec<Link>,
}

/// What the rules compare a link with.
struct Moment<'a> {
    now: NaiveDateTime,
    context: Option<&'a str>,
    project: Option<&'a str>,
    events: &'a [EventSpan],
}

fn has_no_rules(link: &QuickLink) -> bool {
    link.contexts.is_none()
        && link.projects.is_none()
        && link.days.is_none()
        && link.time.is_none()
        && link.event.is_none()
}

/// Every set rule but the event's; a link without any rule matches only when pinned.
fn rules_match(link: &QuickLink, at: &Moment) -> bool {
    if has_no_rules(link) {
        return link.pinned;
    }
    let contexts = link.contexts.as_ref().is_none_or(|ids| {
        at.context
            .is_some_and(|context| ids.iter().any(|id| id == context))
    });
    let projects = link.projects.as_ref().is_none_or(|names| {
        at.project.is_some_and(|project| {
            let project = project.to_lowercase();
            names.iter().any(|name| name.to_lowercase() == project)
        })
    });
    let weekday = at.now.weekday().num_days_from_sunday();
    let days = link
        .days
        .as_ref()
        .is_none_or(|days| days.iter().any(|day| u32::from(*day) == weekday));
    let minute = f64::from(at.now.hour() * 60 + at.now.minute());
    let time = link
        .time
        .as_ref()
        .is_none_or(|range| range.from <= minute && minute < range.to);
    contexts && projects && days && time
}

/// Whether a meeting whose title contains `word` makes the link show now.
fn event_window_open(link: &QuickLink, word: &str, at: &Moment) -> bool {
    let word = word.to_lowercase();
    let matching: Vec<&EventSpan> = at
        .events
        .iter()
        .filter(|event| event.title.to_lowercase().contains(&word))
        .collect();
    let now = at.now;
    match link.event_window {
        EventWindow::During => matching.iter().any(|e| e.start <= now && now < e.end),
        EventWindow::Before => {
            let lead = Duration::seconds((link.lead_minutes * 60.0).round() as i64);
            matching
                .iter()
                .any(|e| e.start - lead <= now && now < e.end)
        }
        EventWindow::UntilEnd => matching
            .iter()
            .map(|e| e.end)
            .max()
            .is_some_and(|end| now < end),
    }
}

/// The link's title, else one made from its target.
fn shown(link: &QuickLink) -> Link {
    let title = if link.title.is_empty() {
        link_title(&link.target)
    } else {
        link.title.clone()
    };
    Link {
        title,
        target: link.target.clone(),
    }
}

/// Splits `links` into Now (event links with an open window first, then the others that match) and More.
pub fn quick_access(
    now: NaiveDateTime,
    links: &[QuickLink],
    context: Option<&str>,
    events: &[EventSpan],
    project: Option<&str>,
) -> QuickAccess {
    let at = Moment {
        now,
        context,
        project,
        events,
    };
    let mut around_events = Vec::new();
    let mut matching = Vec::new();
    let mut more = Vec::new();
    for link in links {
        let fits = rules_match(link, &at);
        match &link.event {
            Some(word) if fits && event_window_open(link, word, &at) => {
                around_events.push(shown(link));
            }
            None if fits => matching.push(shown(link)),
            _ => more.push(shown(link)),
        }
    }
    around_events.extend(matching);
    QuickAccess {
        now: around_events,
        more,
    }
}

fn starts_with_ignoring_case(text: &str, prefix: &str) -> bool {
    text.get(..prefix.len())
        .is_some_and(|start| start.eq_ignore_ascii_case(prefix))
}

/// A web link's host (with its port); the target itself when it doesn't parse. `None` for other targets.
fn web_title(target: &str) -> Option<String> {
    if !starts_with_ignoring_case(target, "http://")
        && !starts_with_ignoring_case(target, "https://")
    {
        return None;
    }
    let host = url::Url::parse(target).ok().and_then(|url| {
        let host = url.host_str()?.to_string();
        Some(match url.port() {
            Some(port) => format!("{host}:{port}"),
            None => host,
        })
    });
    Some(host.unwrap_or_else(|| target.to_string()))
}

/// The last path segment without `.md`.
fn note_name(target: &str) -> String {
    let name = basename(target);
    if let Some(cut) = name.len().checked_sub(3)
        && name
            .get(cut..)
            .is_some_and(|extension| extension.eq_ignore_ascii_case(".md"))
    {
        return name[..cut].to_string();
    }
    name.to_string()
}

/// A web link by its host, an Obsidian URI as "Obsidian link", a vault path by its note name.
pub fn link_title(target: &str) -> String {
    if let Some(title) = web_title(target) {
        return title;
    }
    if starts_with_ignoring_case(target, "obsidian://") {
        return "Obsidian link".into();
    }
    note_name(target)
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::store::settings::MinuteRange;

    fn at(day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    /// Monday 2026-10-12.
    fn monday(hour: u32, minute: u32) -> NaiveDateTime {
        at(12, hour, minute)
    }

    fn always() -> QuickLink {
        QuickLink {
            title: String::new(),
            target: "https://board.example.com/x".into(),
            contexts: None,
            projects: None,
            days: None,
            time: None,
            event: None,
            event_window: EventWindow::UntilEnd,
            lead_minutes: 15.0,
            pinned: true,
        }
    }

    fn named(title: &str) -> QuickLink {
        QuickLink {
            title: title.into(),
            ..always()
        }
    }

    fn event(title: &str, start: NaiveDateTime, end: NaiveDateTime) -> EventSpan {
        EventSpan {
            title: title.into(),
            start,
            end,
        }
    }

    fn weekly_planning() -> EventSpan {
        event("Weekly Planning", monday(11, 0), monday(12, 0))
    }

    fn titles(links: &[Link]) -> Vec<&str> {
        links.iter().map(|link| link.title.as_str()).collect()
    }

    fn build(
        now: NaiveDateTime,
        links: &[QuickLink],
        events: &[EventSpan],
        project: Option<&str>,
    ) -> QuickAccess {
        quick_access(now, links, Some("work"), events, project)
    }

    #[test]
    fn a_link_is_named_by_its_title_else_by_its_host_or_note_name() {
        let links = [
            always(),
            QuickLink {
                target: "2-Work/Plan.md".into(),
                ..always()
            },
            named("Board"),
        ];
        let shown = build(monday(10, 0), &links, &[], None);
        assert_eq!(titles(&shown.now), ["board.example.com", "Plan", "Board"]);
        assert_eq!(shown.now[0].target, "https://board.example.com/x");
        assert!(shown.more.is_empty());
    }

    #[test]
    fn links_whose_rules_dont_match_go_under_more() {
        let links = [
            QuickLink {
                contexts: Some(vec!["private".into()]),
                ..named("Private")
            },
            QuickLink {
                projects: Some(vec!["Oscorp".into()]),
                ..named("Oscorp")
            },
            QuickLink {
                days: Some(vec![5]),
                ..named("Friday")
            },
            QuickLink {
                time: Some(MinuteRange {
                    from: 780.0,
                    to: 1080.0,
                }),
                ..named("Afternoon")
            },
            QuickLink {
                time: Some(MinuteRange {
                    from: 540.0,
                    to: 720.0,
                }),
                ..named("Morning")
            },
        ];
        let shown = build(monday(10, 0), &links, &[], Some("oscorp"));
        assert_eq!(titles(&shown.now), ["Oscorp", "Morning"]);
        assert_eq!(titles(&shown.more), ["Private", "Friday", "Afternoon"]);
    }

    #[test]
    fn event_links_show_while_their_window_is_open_before_the_others() {
        let links = [
            named("Pinned"),
            QuickLink {
                event: Some("weekly".into()),
                ..named("Until end")
            },
            QuickLink {
                event: Some("weekly".into()),
                event_window: EventWindow::During,
                ..named("During")
            },
            QuickLink {
                event: Some("weekly".into()),
                event_window: EventWindow::Before,
                lead_minutes: 30.0,
                ..named("Before")
            },
            QuickLink {
                event: Some("standup".into()),
                ..named("Other")
            },
        ];
        let events = [weekly_planning()];
        let cases = [
            ((10, 0), vec!["Until end", "Pinned"]),
            ((10, 45), vec!["Until end", "Before", "Pinned"]),
            ((11, 30), vec!["Until end", "During", "Before", "Pinned"]),
            ((12, 30), vec!["Pinned"]),
        ];
        for ((hour, minute), expected) in cases {
            let shown = build(monday(hour, minute), &links, &events, None);
            assert_eq!(titles(&shown.now), expected, "{hour}:{minute:02}");
        }
        let after = build(monday(12, 30), &links, &events, None);
        assert_eq!(
            titles(&after.more),
            ["Until end", "During", "Before", "Other"]
        );
    }

    #[test]
    fn a_link_without_rules_shows_now_only_when_pinned() {
        let links = [
            named("Pinned"),
            QuickLink {
                pinned: false,
                ..named("Unpinned")
            },
        ];
        let shown = build(monday(10, 0), &links, &[], None);
        assert_eq!(titles(&shown.now), ["Pinned"]);
        assert_eq!(titles(&shown.more), ["Unpinned"]);
    }

    #[test]
    fn a_link_with_rules_ignores_pinned() {
        let monday_only = |pinned| QuickLink {
            days: Some(vec![1]),
            pinned,
            ..named("Monday")
        };
        for pinned in [true, false] {
            let shown = build(monday(10, 0), &[monday_only(pinned)], &[], None);
            assert_eq!(titles(&shown.now), ["Monday"], "pinned {pinned}");
        }
    }

    #[test]
    fn a_project_rule_never_matches_without_a_current_project() {
        let link = QuickLink {
            projects: Some(vec!["Oscorp".into()]),
            ..named("Oscorp")
        };
        let shown = build(monday(10, 0), &[link], &[], None);
        assert!(shown.now.is_empty());
        assert_eq!(titles(&shown.more), ["Oscorp"]);
    }

    #[test]
    fn a_project_rule_ignores_case() {
        let link = QuickLink {
            projects: Some(vec!["OSCORP".into(), "Other".into()]),
            ..named("Oscorp")
        };
        for project in ["oscorp", "Oscorp", "other"] {
            let shown = build(
                monday(10, 0),
                std::slice::from_ref(&link),
                &[],
                Some(project),
            );
            assert_eq!(titles(&shown.now), ["Oscorp"], "{project}");
        }
        let shown = build(monday(10, 0), &[link], &[], Some("Pay"));
        assert!(shown.now.is_empty());
    }

    #[test]
    fn a_contexts_rule_never_matches_without_a_current_context() {
        let link = QuickLink {
            contexts: Some(vec!["work".into()]),
            ..named("Work")
        };
        let shown = quick_access(monday(10, 0), &[link], None, &[], None);
        assert!(shown.now.is_empty());
        assert_eq!(titles(&shown.more), ["Work"]);
    }

    #[test]
    fn a_time_rule_includes_its_start_and_excludes_its_end() {
        let link = QuickLink {
            time: Some(MinuteRange {
                from: 600.0,
                to: 660.0,
            }),
            ..named("Window")
        };
        for ((hour, minute), shown_now) in [
            ((9, 59), false),
            ((10, 0), true),
            ((10, 59), true),
            ((11, 0), false),
        ] {
            let shown = build(monday(hour, minute), std::slice::from_ref(&link), &[], None);
            assert_eq!(!shown.now.is_empty(), shown_now, "{hour}:{minute:02}");
        }
    }

    #[test]
    fn a_days_rule_counts_sunday_as_zero() {
        let sunday = QuickLink {
            days: Some(vec![0]),
            ..named("Sunday")
        };
        let on_sunday = build(at(11, 10, 0), std::slice::from_ref(&sunday), &[], None);
        assert_eq!(titles(&on_sunday.now), ["Sunday"]);
        let on_monday = build(monday(10, 0), &[sunday], &[], None);
        assert_eq!(titles(&on_monday.more), ["Sunday"]);
    }

    #[test]
    fn an_until_end_link_is_open_from_the_start_of_the_day_until_the_last_matching_event_ends() {
        let link = QuickLink {
            event: Some("planning".into()),
            ..named("Planning")
        };
        let events = [
            weekly_planning(),
            event("Planning again", monday(15, 0), monday(16, 0)),
            event("Lunch", monday(12, 0), monday(13, 0)),
        ];
        for ((hour, minute), open) in [((0, 0), true), ((13, 30), true), ((16, 0), false)] {
            let shown = build(
                monday(hour, minute),
                std::slice::from_ref(&link),
                &events,
                None,
            );
            assert_eq!(!shown.now.is_empty(), open, "{hour}:{minute:02}");
        }
    }

    #[test]
    fn a_during_link_is_open_from_the_start_and_closed_at_the_end() {
        let link = QuickLink {
            event: Some("weekly".into()),
            event_window: EventWindow::During,
            ..named("During")
        };
        for ((hour, minute), open) in [((10, 59), false), ((11, 0), true), ((12, 0), false)] {
            let shown = build(
                monday(hour, minute),
                std::slice::from_ref(&link),
                &[weekly_planning()],
                None,
            );
            assert_eq!(!shown.now.is_empty(), open, "{hour}:{minute:02}");
        }
    }

    #[test]
    fn a_before_link_opens_its_lead_ahead_and_includes_the_meeting_itself() {
        let link = QuickLink {
            event: Some("weekly".into()),
            event_window: EventWindow::Before,
            lead_minutes: 30.0,
            ..named("Before")
        };
        for ((hour, minute), open) in [
            ((10, 29), false),
            ((10, 30), true),
            ((11, 30), true),
            ((12, 0), false),
        ] {
            let shown = build(
                monday(hour, minute),
                std::slice::from_ref(&link),
                &[weekly_planning()],
                None,
            );
            assert_eq!(!shown.now.is_empty(), open, "{hour}:{minute:02}");
        }
    }

    #[test]
    fn event_titles_match_as_case_insensitive_substrings() {
        let events = [weekly_planning()];
        for (word, open) in [
            ("WEEKLY", true),
            ("ly plan", true),
            ("Planning", true),
            ("monthly", false),
        ] {
            let link = QuickLink {
                event: Some(word.into()),
                ..named("Event")
            };
            let shown = build(monday(10, 0), &[link], &events, None);
            assert_eq!(!shown.now.is_empty(), open, "{word}");
        }
    }

    #[test]
    fn an_event_link_stays_under_more_when_another_rule_fails() {
        let link = QuickLink {
            event: Some("weekly".into()),
            days: Some(vec![5]),
            ..named("Friday meeting")
        };
        let shown = build(monday(10, 0), &[link], &[weekly_planning()], None);
        assert!(shown.now.is_empty());
        assert_eq!(titles(&shown.more), ["Friday meeting"]);
    }

    #[test]
    fn titles_are_made_from_the_target() {
        for (target, expected) in [
            ("https://board.example.com/x", "board.example.com"),
            ("http://localhost:3000/a", "localhost:3000"),
            ("HTTPS://Board.Example.com:8080/x", "board.example.com:8080"),
            ("https://", "https://"),
            ("obsidian://open?vault=Vault&file=Note", "Obsidian link"),
            ("OBSIDIAN://open", "Obsidian link"),
            ("2-Work/Plan.md", "Plan"),
            ("a/b/Note.MD", "Note"),
            ("folder/Note.md/", "Note"),
            ("2-Work/Plan", "Plan"),
            ("Plan.md", "Plan"),
        ] {
            assert_eq!(link_title(target), expected, "{target}");
        }
    }
}
