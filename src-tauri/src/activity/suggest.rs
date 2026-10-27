//! The activity overlay's suggestions: the live call, today's meetings since the timer started, today's
//! Jira tickets and the timers used lately. Pure.

use std::collections::HashSet;

use chrono::NaiveDateTime;
use serde::Serialize;
use ts_rs::TS;

use crate::calendar::events::{Event, event_meeting_link};
use crate::projects::meeting_project;
use crate::store::settings::Project;
use crate::tools::RecentActivity;

const MAX_SUGGESTIONS: usize = 60;
const MAX_MEETINGS: usize = 3;

/// Meeting apps by a domain anywhere in an event's link, location or notes.
const MEETING_APPS: [(&str, &str); 3] = [
    ("zoom.us", "Zoom"),
    ("teams.microsoft.com", "Teams"),
    ("meet.google.com", "Meet"),
];

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum SuggestionGroup {
    Meetings,
    TicketsToday,
    RecentTimers,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(
    export,
    export_to = "src/api/types.gen.ts",
    rename = "ActivitySuggestion"
)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub group: SuggestionGroup,
    pub note: String,
    /// The Daybook project; `None` keeps the picked one.
    pub project: Option<String>,
    /// TimeTracker's project to start: a recent timer's, else the first linked to `project`.
    pub tt_project_id: Option<String>,
    /// The tag of a recent timer, carried over when it starts again.
    pub tag: Option<String>,
    /// `Oscorp · QRS-6805 review`
    pub display: String,
}

pub struct LiveCall<'a> {
    pub app: &'a str,
    pub title: &'a str,
}

pub struct SuggestionInput<'a> {
    pub live_call: Option<LiveCall<'a>>,
    /// Today's timed events.
    pub events: &'a [Event],
    /// When the running timer started, else now.
    pub since: NaiveDateTime,
    pub now: NaiveDateTime,
    /// Today's Jira tickets, newest first.
    pub tickets: &'a [String],
    pub recent: &'a [RecentActivity],
    /// The projects that count now.
    pub projects: &'a [&'a Project],
}

/// Events that began before `now` and end after `since`, latest start first.
pub fn events_since(events: &[Event], since: NaiveDateTime, now: NaiveDateTime) -> Vec<&Event> {
    let mut found: Vec<&Event> = events
        .iter()
        .filter(|event| event.start < now && event.end > since)
        .collect();
    found.sort_by_key(|event| std::cmp::Reverse(event.start));
    found
}

/// The call app an event links to, else `Meeting`.
pub fn meeting_app(event: &Event) -> String {
    if let Some(link) = event_meeting_link(event) {
        return link.app.name().to_string();
    }
    let haystack = [&event.url, &event.location, &event.notes]
        .into_iter()
        .flatten()
        .map(|part| part.to_lowercase())
        .collect::<Vec<_>>()
        .join(" ");
    MEETING_APPS
        .iter()
        .find(|(domain, _)| haystack.contains(domain))
        .map_or("Meeting", |(_, app)| app)
        .to_string()
}

fn display(project: Option<&str>, note: &str) -> String {
    [project.unwrap_or_default(), note]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

#[derive(Default)]
struct Suggestions {
    seen: HashSet<(Option<String>, String)>,
    list: Vec<Suggestion>,
}

/// The first TimeTracker project linked to the Daybook project `name`.
fn first_linked(name: Option<&str>, projects: &[&Project]) -> Option<String> {
    let project = projects
        .iter()
        .find(|project| Some(project.name.as_str()) == name)?;
    project
        .timetracker_projects
        .first()
        .map(|linked| linked.id.clone())
}

impl Suggestions {
    fn push(&mut self, suggestion: Suggestion) {
        if self
            .seen
            .insert((suggestion.project.clone(), suggestion.note.clone()))
        {
            self.list.push(suggestion);
        }
    }

    fn meeting(&mut self, app: &str, title: &str, projects: &[&Project]) {
        let note = if title.is_empty() {
            app.to_string()
        } else {
            format!("{app} - {title}")
        };
        let project = meeting_project(title, projects);
        self.push(Suggestion {
            group: SuggestionGroup::Meetings,
            display: display(project.as_deref(), &note),
            tt_project_id: first_linked(project.as_deref(), projects),
            tag: None,
            note,
            project,
        });
    }
}

/// In the overlay's order, without repeats of the same project and note, at most 60.
pub fn activity_suggestions(input: &SuggestionInput) -> Vec<Suggestion> {
    let mut suggestions = Suggestions::default();
    if let Some(call) = &input.live_call {
        suggestions.meeting(call.app, call.title, input.projects);
    }
    for event in events_since(input.events, input.since, input.now)
        .into_iter()
        .take(MAX_MEETINGS)
    {
        suggestions.meeting(&meeting_app(event), &event.title, input.projects);
    }
    for key in input.tickets {
        suggestions.push(Suggestion {
            group: SuggestionGroup::TicketsToday,
            note: key.clone(),
            project: None,
            tt_project_id: None,
            tag: None,
            display: key.clone(),
        });
    }
    for recent in input.recent {
        suggestions.push(Suggestion {
            group: SuggestionGroup::RecentTimers,
            display: display(Some(&recent.project), &recent.note),
            note: recent.note.clone(),
            project: Some(recent.project.clone()),
            tt_project_id: Some(recent.tt_project_id.clone()),
            tag: recent.tag.clone(),
        });
    }
    suggestions.list.truncate(MAX_SUGGESTIONS);
    suggestions.list
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    use serde_json::json;

    fn at(hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 27)
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

    fn oscorp() -> Project {
        let mut project = Project::new("Oscorp");
        project.keywords = vec!["QRS".into()];
        project.timetracker_projects = ["sdk", "apollo"]
            .map(|id| crate::store::settings::LinkedProject {
                id: id.into(),
                name: id.into(),
            })
            .to_vec();
        project
    }

    fn titles(events: Vec<&Event>) -> Vec<&str> {
        events.iter().map(|event| event.title.as_str()).collect()
    }

    fn meetings() -> Vec<Event> {
        vec![
            event("Standup", at(9, 0), at(9, 30)),
            event("QRS planning", at(10, 0), at(11, 0)),
            event("Later", at(12, 0), at(13, 0)),
        ]
    }

    fn input<'a>(
        events: &'a [Event],
        projects: &'a [&'a Project],
        tickets: &'a [String],
        recent: &'a [RecentActivity],
    ) -> SuggestionInput<'a> {
        SuggestionInput {
            live_call: None,
            events,
            since: at(9, 20),
            now: at(10, 50),
            tickets,
            recent,
            projects,
        }
    }

    #[test]
    fn events_since_lists_the_events_that_overlap_the_span_latest_first() {
        let events = meetings();
        assert_eq!(
            titles(events_since(&events, at(9, 20), at(10, 50))),
            ["QRS planning", "Standup"]
        );
    }

    #[test]
    fn events_since_leaves_out_events_that_ended_before_and_those_not_yet_started() {
        let events = meetings();
        assert_eq!(
            titles(events_since(&events, at(9, 30), at(10, 50))),
            ["QRS planning"]
        );
    }

    #[test]
    fn the_meeting_app_follows_the_domain_in_the_link_location_or_notes() {
        let with = |edit: fn(&mut Event)| {
            let mut event = event("A", at(0, 0), at(1, 0));
            edit(&mut event);
            meeting_app(&event)
        };
        assert_eq!(with(|e| e.url = Some("https://zoom.us/j/1".into())), "Zoom");
        assert_eq!(
            with(|e| e.location = Some("https://teams.microsoft.com/l/x".into())),
            "Teams"
        );
        assert_eq!(
            with(|e| e.notes = Some("Join: https://meet.google.com/abc-defg-hij".into())),
            "Meet"
        );
        assert_eq!(with(|_| {}), "Meeting");
    }

    #[test]
    fn suggestions_come_as_call_meetings_tickets_and_recent_timers() {
        let project = oscorp();
        let projects = [&project];
        let events = meetings();
        let tickets = vec!["QRS-1".to_string()];
        let recent = vec![RecentActivity {
            project: "Oscorp".into(),
            note: "QRS-6805 review".into(),
            tt_project_id: "tt-1".into(),
            tag: Some("Dev".into()),
        }];
        let mut input = input(&events, &projects, &tickets, &recent);
        input.live_call = Some(LiveCall {
            app: "Zoom",
            title: "Sync with oscorp",
        });
        let suggestions = activity_suggestions(&input);
        assert_eq!(
            suggestions,
            vec![
                Suggestion {
                    group: SuggestionGroup::Meetings,
                    note: "Zoom - Sync with oscorp".into(),
                    project: Some("Oscorp".into()),
                    tt_project_id: Some("sdk".into()),
                    tag: None,
                    display: "Oscorp · Zoom - Sync with oscorp".into(),
                },
                Suggestion {
                    group: SuggestionGroup::Meetings,
                    note: "Meeting - QRS planning".into(),
                    project: Some("Oscorp".into()),
                    tt_project_id: Some("sdk".into()),
                    tag: None,
                    display: "Oscorp · Meeting - QRS planning".into(),
                },
                Suggestion {
                    group: SuggestionGroup::Meetings,
                    note: "Meeting - Standup".into(),
                    project: None,
                    tt_project_id: None,
                    tag: None,
                    display: "Meeting - Standup".into(),
                },
                Suggestion {
                    group: SuggestionGroup::TicketsToday,
                    note: "QRS-1".into(),
                    project: None,
                    tt_project_id: None,
                    tag: None,
                    display: "QRS-1".into(),
                },
                Suggestion {
                    group: SuggestionGroup::RecentTimers,
                    note: "QRS-6805 review".into(),
                    project: Some("Oscorp".into()),
                    tt_project_id: Some("tt-1".into()),
                    tag: Some("Dev".into()),
                    display: "Oscorp · QRS-6805 review".into(),
                },
            ]
        );
    }

    #[test]
    fn a_meeting_of_a_project_without_tracker_projects_has_no_tracker_project() {
        let mut hobby = Project::new("Hobby");
        hobby.keywords = vec!["garden".into()];
        let projects = [&hobby];
        let events = vec![event("Garden club", at(10, 0), at(11, 0))];
        let suggestions = activity_suggestions(&input(&events, &projects, &[], &[]));
        assert_eq!(suggestions[0].project.as_deref(), Some("Hobby"));
        assert_eq!(suggestions[0].tt_project_id, None);
    }

    #[test]
    fn a_live_call_without_a_title_is_just_the_app() {
        let mut input = input(&[], &[], &[], &[]);
        input.live_call = Some(LiveCall {
            app: "Zoom",
            title: "",
        });
        let suggestions = activity_suggestions(&input);
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].note, "Zoom");
        assert_eq!(suggestions[0].display, "Zoom");
    }

    #[test]
    fn at_most_three_meetings_since_the_timer_started_are_suggested() {
        let events: Vec<Event> = (8..13)
            .map(|hour| event(&format!("Meeting {hour}"), at(hour, 0), at(hour, 30)))
            .collect();
        let mut input = input(&events, &[], &[], &[]);
        input.since = at(8, 0);
        input.now = at(14, 0);
        let notes: Vec<_> = activity_suggestions(&input)
            .into_iter()
            .map(|suggestion| suggestion.note)
            .collect();
        assert_eq!(
            notes,
            [
                "Meeting - Meeting 12",
                "Meeting - Meeting 11",
                "Meeting - Meeting 10"
            ]
        );
    }

    #[test]
    fn a_repeat_of_the_same_project_and_note_is_dropped() {
        let events = vec![event("Planning", at(10, 0), at(11, 0))];
        let tickets = vec!["QRS-1".to_string(), "QRS-1".to_string()];
        let mut input = input(&events, &[], &tickets, &[]);
        input.live_call = Some(LiveCall {
            app: "Meeting",
            title: "Planning",
        });
        let notes: Vec<_> = activity_suggestions(&input)
            .into_iter()
            .map(|suggestion| suggestion.note)
            .collect();
        assert_eq!(notes, ["Meeting - Planning", "QRS-1"]);
    }

    #[test]
    fn the_same_note_on_another_project_is_kept() {
        let tickets = vec!["QRS-1".to_string()];
        let recent = vec![RecentActivity {
            project: "Oscorp".into(),
            note: "QRS-1".into(),
            tt_project_id: "tt-1".into(),
            tag: None,
        }];
        let displays: Vec<_> = activity_suggestions(&input(&[], &[], &tickets, &recent))
            .into_iter()
            .map(|suggestion| suggestion.display)
            .collect();
        assert_eq!(displays, ["QRS-1", "Oscorp · QRS-1"]);
    }

    #[test]
    fn at_most_sixty_suggestions_are_returned() {
        let tickets: Vec<String> = (0..70).map(|n| format!("QRS-{n}")).collect();
        let suggestions = activity_suggestions(&input(&[], &[], &tickets, &[]));
        assert_eq!(suggestions.len(), 60);
        assert_eq!(suggestions[59].note, "QRS-59");
    }

    #[test]
    fn a_suggestion_serializes_with_camel_case_fields() {
        let suggestion = Suggestion {
            group: SuggestionGroup::RecentTimers,
            note: "QRS-1".into(),
            project: Some("Oscorp".into()),
            tt_project_id: Some("tt-1".into()),
            tag: None,
            display: "Oscorp · QRS-1".into(),
        };
        assert_eq!(
            serde_json::to_value(&suggestion).unwrap(),
            json!({
                "group": "recentTimers",
                "note": "QRS-1",
                "project": "Oscorp",
                "ttProjectId": "tt-1",
                "tag": null,
                "display": "Oscorp · QRS-1",
            })
        );
    }
}
