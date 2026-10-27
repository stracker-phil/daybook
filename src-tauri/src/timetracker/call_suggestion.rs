//! A finished call as a TimeTracker suggestion on its project, or why it isn't sent
//! (docs/timetracker.md#status-and-calls). Pure.

use chrono::{NaiveDateTime, TimeDelta};

use crate::calendar::Event;
use crate::calls::ActiveCall;
use crate::calls::checks::GENERIC_TITLE;
use crate::projects::meeting_project;
use crate::store::settings::{Project, Status};

/// Shorter calls aren't sent.
pub const MIN_CALL: TimeDelta = TimeDelta::minutes(3);
/// TimeTracker refuses longer notes.
const MAX_NOTE_CHARS: usize = 500;

/// `record --project <project_id> --span <start>-<end> --note=<note> --source Daybook`.
#[derive(Clone, Debug, PartialEq)]
pub struct CallSuggestion {
    /// TimeTracker's project id.
    pub project_id: String,
    /// The Daybook project.
    pub project: String,
    pub start: NaiveDateTime,
    /// At most the midnight after `start`: TimeTracker keeps a suggestion on one day.
    pub end: NaiveDateTime,
    /// `<App> - <title>`, one line.
    pub note: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Skip {
    NotWorking,
    NoStatusFeature,
    TooShort,
    NoProject,
    NotLinked,
}

impl Skip {
    pub fn reason(self) -> String {
        match self {
            Self::NotWorking => "Not Working".into(),
            Self::NoStatusFeature => "The context has no Status feature".into(),
            Self::TooShort => format!("Shorter than {} minutes", MIN_CALL.num_minutes()),
            Self::NoProject => "No project in the title".into(),
            Self::NotLinked => "Project not linked to TimeTracker".into(),
        }
    }
}

pub struct FinishedCall<'a> {
    pub call: &'a ActiveCall,
    pub end: NaiveDateTime,
    /// Today's status when the call ended.
    pub status: Option<Status>,
    /// The current context has the Status feature.
    pub status_feature: bool,
    /// The context calendars' events on the call's first day.
    pub events: &'a [Event],
    /// The current context's projects.
    pub offered: &'a [&'a Project],
}

/// The event that overlaps the call and started last.
fn overlapping(events: &[Event], start: NaiveDateTime, end: NaiveDateTime) -> Option<&Event> {
    events
        .iter()
        .filter(|event| event.start < end && event.end > start)
        .max_by_key(|event| event.start)
}

/// Whitespace collapsed to single spaces, at most `MAX_NOTE_CHARS`.
fn note(app: &str, title: &str) -> String {
    let line = format!("{app} - {title}")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    line.chars().take(MAX_NOTE_CHARS).collect()
}

/// The span ends at midnight at the latest.
fn same_day_end(start: NaiveDateTime, end: NaiveDateTime) -> NaiveDateTime {
    let midnight = start
        .date()
        .succ_opt()
        .and_then(|day| day.and_hms_opt(0, 0, 0))
        .unwrap_or(end);
    end.min(midnight)
}

/// Checked in order: Working, the Status feature, at least `MIN_CALL` (of the part on its first day), a
/// project named in the call's or the overlapping event's title, its link to TimeTracker (the first).
pub fn call_suggestion(finished: &FinishedCall) -> Result<CallSuggestion, Skip> {
    let call = finished.call;
    if finished.status != Some(Status::Working) {
        return Err(Skip::NotWorking);
    }
    if !finished.status_feature {
        return Err(Skip::NoStatusFeature);
    }
    let end = same_day_end(call.since, finished.end);
    if end - call.since < MIN_CALL {
        return Err(Skip::TooShort);
    }
    let event = overlapping(finished.events, call.since, finished.end).map(|e| e.title.as_str());
    let title = match event {
        Some(event) if call.title == GENERIC_TITLE && !event.trim().is_empty() => event,
        _ => call.title.as_str(),
    };
    let text = format!("{title} {}", event.unwrap_or_default());
    let project = meeting_project(&text, finished.offered).ok_or(Skip::NoProject)?;
    let linked = project
        .timetracker_projects
        .first()
        .ok_or(Skip::NotLinked)?;
    Ok(CallSuggestion {
        project_id: linked.id.clone(),
        project: project.name.clone(),
        start: call.since,
        end,
        note: note(&call.app, title),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::LinkedProject;
    use chrono::NaiveDate;

    fn at(day: u32, hour: u32, minute: u32, second: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day)
            .unwrap()
            .and_hms_opt(hour, minute, second)
            .unwrap()
    }

    fn call(app: &str, title: &str, since: NaiveDateTime) -> ActiveCall {
        ActiveCall {
            app: app.into(),
            bundle_id: format!("com.example.{}", app.to_lowercase()),
            title: title.into(),
            since,
        }
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

    fn project(name: &str, tt_ids: &[&str]) -> Project {
        let mut project = Project::new(name);
        project.timetracker_projects = tt_ids
            .iter()
            .map(|id| LinkedProject {
                id: id.to_string(),
                name: format!("TT {id}"),
            })
            .collect();
        project
    }

    fn suggest(
        call: &ActiveCall,
        end: NaiveDateTime,
        events: &[Event],
        offered: &[&Project],
    ) -> Result<CallSuggestion, Skip> {
        call_suggestion(&FinishedCall {
            call,
            end,
            status: Some(Status::Working),
            status_feature: true,
            events,
            offered,
        })
    }

    #[test]
    fn a_call_naming_a_project_becomes_a_suggestion_on_its_linked_project() {
        let oscorp = project("Oscorp", &["tt-oscorp", "tt-other"]);
        let zoom = call("Zoom", "Oscorp Weekly", at(27, 10, 0, 0));

        assert_eq!(
            suggest(&zoom, at(27, 10, 45, 30), &[], &[&oscorp]),
            Ok(CallSuggestion {
                project_id: "tt-oscorp".into(),
                project: "Oscorp".into(),
                start: at(27, 10, 0, 0),
                end: at(27, 10, 45, 30),
                note: "Zoom - Oscorp Weekly".into(),
            })
        );
    }

    #[test]
    fn a_project_keyword_in_the_title_picks_the_project() {
        let mut oscorp = project("Oscorp", &["tt-oscorp"]);
        oscorp.keywords = vec!["QRS".into()];
        let zoom = call("Zoom", "qrs standup", at(27, 10, 0, 0));

        let suggestion = suggest(&zoom, at(27, 10, 30, 0), &[], &[&oscorp]).unwrap();

        assert_eq!(suggestion.project, "Oscorp");
        assert_eq!(suggestion.note, "Zoom - qrs standup");
    }

    #[test]
    fn the_overlapping_event_names_the_project_when_the_title_does_not() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let internal = project("Internal", &["tt-internal"]);
        let zoom = call("Zoom", "Weekly sync", at(27, 10, 0, 0));
        let events = [event("Oscorp Weekly", at(27, 9, 55, 0), at(27, 11, 0, 0))];

        let suggestion = suggest(&zoom, at(27, 10, 45, 0), &events, &[&internal, &oscorp]).unwrap();

        assert_eq!(suggestion.project_id, "tt-oscorp");
        assert_eq!(suggestion.note, "Zoom - Weekly sync");
    }

    #[test]
    fn a_generic_meeting_title_is_replaced_by_the_overlapping_events_in_the_note() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let teams = call("Teams", "Meeting", at(27, 10, 0, 0));
        let events = [event("Oscorp Weekly", at(27, 10, 0, 0), at(27, 11, 0, 0))];

        let suggestion = suggest(&teams, at(27, 10, 30, 0), &events, &[&oscorp]).unwrap();

        assert_eq!(suggestion.note, "Teams - Oscorp Weekly");
    }

    #[test]
    fn a_generic_meeting_title_stays_when_no_event_overlaps() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let teams = call("Teams", "Meeting", at(27, 10, 0, 0));

        assert_eq!(
            suggest(&teams, at(27, 10, 30, 0), &[], &[&oscorp]),
            Err(Skip::NoProject)
        );
    }

    #[test]
    fn a_blank_event_title_does_not_replace_a_generic_title() {
        let meeting = project("Meeting", &["tt-meeting"]);
        let teams = call("Teams", "Meeting", at(27, 10, 0, 0));
        let events = [event("  ", at(27, 10, 0, 0), at(27, 11, 0, 0))];

        let suggestion = suggest(&teams, at(27, 10, 30, 0), &events, &[&meeting]).unwrap();

        assert_eq!(suggestion.note, "Teams - Meeting");
    }

    #[test]
    fn the_event_that_started_last_wins() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let internal = project("Internal", &["tt-internal"]);
        let teams = call("Teams", "Meeting", at(27, 10, 0, 0));
        let events = [
            event("Oscorp Weekly", at(27, 9, 0, 0), at(27, 12, 0, 0)),
            event("Internal Sync", at(27, 10, 10, 0), at(27, 10, 50, 0)),
            event("Lunch", at(27, 12, 0, 0), at(27, 13, 0, 0)),
        ];

        let suggestion =
            suggest(&teams, at(27, 10, 45, 0), &events, &[&oscorp, &internal]).unwrap();

        assert_eq!(suggestion.project_id, "tt-internal");
        assert_eq!(suggestion.note, "Teams - Internal Sync");
    }

    #[test]
    fn an_event_that_ended_when_the_call_began_does_not_overlap() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let zoom = call("Zoom", "Weekly sync", at(27, 10, 0, 0));
        let events = [event("Oscorp Weekly", at(27, 9, 0, 0), at(27, 10, 0, 0))];

        assert_eq!(
            suggest(&zoom, at(27, 10, 30, 0), &events, &[&oscorp]),
            Err(Skip::NoProject)
        );
    }

    #[test]
    fn an_event_starting_when_the_call_ended_does_not_overlap() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let zoom = call("Zoom", "Weekly sync", at(27, 10, 0, 0));
        let events = [event("Oscorp Weekly", at(27, 10, 30, 0), at(27, 11, 0, 0))];

        assert_eq!(
            suggest(&zoom, at(27, 10, 30, 0), &events, &[&oscorp]),
            Err(Skip::NoProject)
        );
    }

    #[test]
    fn a_call_is_sent_from_three_minutes_on() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let zoom = call("Zoom", "Oscorp", at(27, 10, 0, 0));
        for (end, expected_ok) in [
            (at(27, 10, 0, 0), false),
            (at(27, 10, 2, 59), false),
            (at(27, 10, 3, 0), true),
            (at(27, 10, 3, 1), true),
        ] {
            assert_eq!(
                suggest(&zoom, end, &[], &[&oscorp]).is_ok(),
                expected_ok,
                "{end}"
            );
        }
        assert_eq!(
            suggest(&zoom, at(27, 10, 2, 59), &[], &[&oscorp]),
            Err(Skip::TooShort)
        );
    }

    #[test]
    fn only_working_sends_a_call() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let zoom = call("Zoom", "Oscorp", at(27, 10, 0, 0));
        for status in [None, Some(Status::Break), Some(Status::Off)] {
            let result = call_suggestion(&FinishedCall {
                call: &zoom,
                end: at(27, 10, 30, 0),
                status,
                status_feature: true,
                events: &[],
                offered: &[&oscorp],
            });
            assert_eq!(result, Err(Skip::NotWorking), "{status:?}");
        }
    }

    #[test]
    fn a_context_without_the_status_feature_sends_nothing() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let zoom = call("Zoom", "Oscorp", at(27, 10, 0, 0));
        let result = call_suggestion(&FinishedCall {
            call: &zoom,
            end: at(27, 10, 30, 0),
            status: Some(Status::Working),
            status_feature: false,
            events: &[],
            offered: &[&oscorp],
        });
        assert_eq!(result, Err(Skip::NoStatusFeature));
    }

    #[test]
    fn not_working_is_checked_before_the_status_feature_and_the_length() {
        let zoom = call("Zoom", "Oscorp", at(27, 10, 0, 0));
        let result = call_suggestion(&FinishedCall {
            call: &zoom,
            end: at(27, 10, 1, 0),
            status: Some(Status::Break),
            status_feature: false,
            events: &[],
            offered: &[],
        });
        assert_eq!(result, Err(Skip::NotWorking));
    }

    #[test]
    fn a_call_naming_no_offered_project_is_not_sent() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let zoom = call("Zoom", "Dentist", at(27, 10, 0, 0));

        assert_eq!(
            suggest(&zoom, at(27, 10, 30, 0), &[], &[&oscorp]),
            Err(Skip::NoProject)
        );
        assert_eq!(
            suggest(&zoom, at(27, 10, 30, 0), &[], &[]),
            Err(Skip::NoProject)
        );
    }

    #[test]
    fn a_project_without_a_timetracker_link_is_not_sent() {
        let oscorp = project("Oscorp", &[]);
        let zoom = call("Zoom", "Oscorp", at(27, 10, 0, 0));

        assert_eq!(
            suggest(&zoom, at(27, 10, 30, 0), &[], &[&oscorp]),
            Err(Skip::NotLinked)
        );
    }

    #[test]
    fn the_span_is_cut_at_the_midnight_after_the_start() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let zoom = call("Zoom", "Oscorp", at(27, 23, 30, 0));

        let suggestion = suggest(&zoom, at(28, 0, 45, 0), &[], &[&oscorp]).unwrap();

        assert_eq!(suggestion.start, at(27, 23, 30, 0));
        assert_eq!(suggestion.end, at(28, 0, 0, 0));
    }

    #[test]
    fn a_call_with_under_three_minutes_before_midnight_is_too_short() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let zoom = call("Zoom", "Oscorp", at(27, 23, 58, 0));

        assert_eq!(
            suggest(&zoom, at(28, 1, 0, 0), &[], &[&oscorp]),
            Err(Skip::TooShort)
        );
    }

    #[test]
    fn the_note_is_one_line() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let zoom = call("Zoom", "  Oscorp \n  Weekly\t sync ", at(27, 10, 0, 0));

        let suggestion = suggest(&zoom, at(27, 10, 30, 0), &[], &[&oscorp]).unwrap();

        assert_eq!(suggestion.note, "Zoom - Oscorp Weekly sync");
    }

    #[test]
    fn the_note_is_cut_to_500_characters() {
        let oscorp = project("Oscorp", &["tt-oscorp"]);
        let title = format!("Oscorp {}", "é".repeat(600));
        let zoom = call("Zoom", &title, at(27, 10, 0, 0));

        let suggestion = suggest(&zoom, at(27, 10, 30, 0), &[], &[&oscorp]).unwrap();

        assert_eq!(suggestion.note.chars().count(), 500);
        assert!(suggestion.note.starts_with("Zoom - Oscorp éé"));
    }

    #[test]
    fn every_skip_has_its_reason() {
        for (skip, reason) in [
            (Skip::NotWorking, "Not Working"),
            (Skip::NoStatusFeature, "The context has no Status feature"),
            (Skip::TooShort, "Shorter than 3 minutes"),
            (Skip::NoProject, "No project in the title"),
            (Skip::NotLinked, "Project not linked to TimeTracker"),
        ] {
            assert_eq!(skip.reason(), reason, "{skip:?}");
        }
    }
}
