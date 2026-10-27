//! The log as Settings → Status shows it: newest first, each event with its day and time. Pure.

use chrono::{DateTime, Local, NaiveDate};
use serde::Serialize;
use ts_rs::TS;

use super::model::{ActionState, ActionTarget, Event, Line, Log};
use crate::clock::iso_day;
use crate::locale::DateLocale;

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ActionLog {
    /// Newest first.
    pub events: Vec<ActionLogEvent>,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ActionLogEvent {
    pub id: String,
    /// `YYYY-MM-DD`
    pub day: String,
    /// "Today", "Yesterday" or the day in the Mac's locale ("Mon 26 Oct").
    pub day_label: String,
    /// `HH:MM`
    pub time: String,
    pub trigger: String,
    pub lines: Vec<ActionLogLine>,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ActionLogLine {
    /// `HH:MM` of the last state change.
    pub time: String,
    pub target: ActionTarget,
    pub state: ActionState,
    pub text: String,
    pub detail: Option<String>,
    /// Merged repeats of the event (autosaves), 1 otherwise.
    pub count: u32,
}

pub fn day_label(day: NaiveDate, today: NaiveDate, locale: DateLocale) -> String {
    if day == today {
        "Today".into()
    } else if today.pred_opt() == Some(day) {
        "Yesterday".into()
    } else {
        locale.short_day(day, today)
    }
}

fn hh_mm(at: DateTime<Local>) -> String {
    at.format("%H:%M").to_string()
}

fn line_view(line: &Line, count: u32) -> ActionLogLine {
    ActionLogLine {
        time: hh_mm(line.at),
        target: line.target,
        state: line.state,
        text: line.text.clone(),
        detail: line.detail.clone(),
        count,
    }
}

fn event_view(event: &Event, today: NaiveDate, locale: DateLocale) -> ActionLogEvent {
    let day = event.at.date_naive();
    ActionLogEvent {
        id: event.id.clone(),
        day: iso_day(day),
        day_label: day_label(day, today, locale),
        time: hh_mm(event.at),
        trigger: event.title.clone(),
        lines: event
            .lines
            .iter()
            .map(|line| line_view(line, event.count))
            .collect(),
    }
}

/// The visible events at `now`, newest first, their days in `locale`.
pub fn action_log(log: &Log, now: DateTime<Local>, locale: DateLocale) -> ActionLog {
    let today = now.date_naive();
    let mut events = log.visible(now);
    // Equal times: later-recorded first.
    events.reverse();
    // A merged repeat moves its event to its latest time.
    events.sort_by_key(|event| std::cmp::Reverse(event.at));
    ActionLog {
        events: events
            .into_iter()
            .map(|event| event_view(event, today, locale))
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn date(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn at(month: u32, day: u32, hour: u32, minute: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, month, day, hour, minute, 0)
            .single()
            .expect("a valid local time")
    }

    fn now() -> DateTime<Local> {
        at(10, 27, 12, 0)
    }

    fn line(text: &str, when: DateTime<Local>) -> Line {
        Line {
            target: ActionTarget::Obsidian,
            text: text.to_string(),
            state: ActionState::Done,
            detail: None,
            at: when,
        }
    }

    fn event(id: &str, when: DateTime<Local>, lines: Vec<Line>) -> Event {
        Event {
            id: id.to_string(),
            at: when,
            title: format!("Title {id}"),
            lines,
            count: 1,
            always: false,
        }
    }

    fn en_gb() -> DateLocale {
        DateLocale::parse("en_GB")
    }

    fn ids(view: &ActionLog) -> Vec<&str> {
        view.events.iter().map(|event| event.id.as_str()).collect()
    }

    #[test]
    fn a_day_reads_as_today_yesterday_or_its_date() {
        let today = date(2026, 10, 27);
        for (day, expected) in [
            (date(2026, 10, 27), "Today"),
            (date(2026, 10, 26), "Yesterday"),
            (date(2026, 10, 25), "Sun 25 Oct"),
            (date(2026, 10, 1), "Thu 1 Oct"),
            (date(2025, 12, 31), "Wed 31 Dec 2025"),
        ] {
            assert_eq!(day_label(day, today, en_gb()), expected, "{day}");
        }
    }

    #[test]
    fn a_day_label_is_written_in_the_locale() {
        let today = date(2026, 10, 27);
        for (day, expected) in [
            (date(2026, 10, 25), "So 25. Okt."),
            (date(2025, 12, 31), "Mi 31. Dez. 2025"),
        ] {
            assert_eq!(
                day_label(day, today, DateLocale::parse("de_DE")),
                expected,
                "{day}"
            );
        }
    }

    #[test]
    fn yesterday_is_found_across_a_year_boundary() {
        assert_eq!(
            day_label(date(2025, 12, 31), date(2026, 1, 1), en_gb()),
            "Yesterday"
        );
    }

    #[test]
    fn events_are_listed_newest_first() {
        let log = Log {
            events: vec![
                event("first", at(10, 27, 9, 0), vec![line("a", at(10, 27, 9, 0))]),
                event(
                    "second",
                    at(10, 27, 10, 0),
                    vec![line("b", at(10, 27, 10, 0))],
                ),
                event(
                    "third",
                    at(10, 27, 11, 0),
                    vec![line("c", at(10, 27, 11, 0))],
                ),
            ],
        };
        assert_eq!(
            ids(&action_log(&log, now(), en_gb())),
            ["third", "second", "first"]
        );
    }

    #[test]
    fn an_event_moved_to_a_later_time_by_a_merge_sorts_by_that_time() {
        let log = Log {
            events: vec![
                event(
                    "repeated",
                    at(10, 27, 11, 30),
                    vec![line("a", at(10, 27, 9, 0))],
                ),
                event(
                    "single",
                    at(10, 27, 10, 0),
                    vec![line("b", at(10, 27, 10, 0))],
                ),
            ],
        };
        assert_eq!(
            ids(&action_log(&log, now(), en_gb())),
            ["repeated", "single"]
        );
    }

    #[test]
    fn events_without_lines_are_left_out_unless_kept() {
        let mut kept = event("kept", at(10, 27, 10, 0), Vec::new());
        kept.always = true;
        let log = Log {
            events: vec![
                event("empty", at(10, 27, 9, 0), Vec::new()),
                kept,
                event(
                    "lines",
                    at(10, 27, 11, 0),
                    vec![line("a", at(10, 27, 11, 0))],
                ),
            ],
        };
        assert_eq!(ids(&action_log(&log, now(), en_gb())), ["lines", "kept"]);
    }

    #[test]
    fn an_event_view_carries_its_day_time_trigger_and_lines() {
        let mut slack = line("Away · Away", at(10, 27, 9, 17));
        slack.target = ActionTarget::SlackStatus;
        slack.state = ActionState::Failed;
        slack.detail = Some("Not connected".into());
        let mut saved = event(
            "e1",
            at(10, 26, 18, 5),
            vec![slack, line("Focus: 2 lines", at(10, 26, 18, 6))],
        );
        saved.title = "Focus saved".into();
        saved.count = 3;
        let view = action_log(
            &Log {
                events: vec![saved],
            },
            now(),
            en_gb(),
        );
        assert_eq!(
            serde_json::to_value(&view).unwrap(),
            json!({
                "events": [{
                    "id": "e1",
                    "day": "2026-10-26",
                    "dayLabel": "Yesterday",
                    "time": "18:05",
                    "trigger": "Focus saved",
                    "lines": [
                        {
                            "time": "09:17",
                            "target": "slackStatus",
                            "state": "failed",
                            "text": "Away · Away",
                            "detail": "Not connected",
                            "count": 3,
                        },
                        {
                            "time": "18:06",
                            "target": "obsidian",
                            "state": "done",
                            "text": "Focus: 2 lines",
                            "detail": null,
                            "count": 3,
                        },
                    ],
                }]
            })
        );
    }

    #[test]
    fn today_events_are_labelled_today() {
        let log = Log {
            events: vec![event(
                "e",
                at(10, 27, 8, 5),
                vec![line("a", at(10, 27, 8, 5))],
            )],
        };
        let view = action_log(&log, now(), en_gb());
        assert_eq!(view.events[0].day_label, "Today");
        assert_eq!(view.events[0].time, "08:05");
    }

    #[test]
    fn an_empty_log_has_no_events() {
        assert_eq!(
            action_log(&Log::default(), now(), en_gb()),
            ActionLog { events: Vec::new() }
        );
    }
}
