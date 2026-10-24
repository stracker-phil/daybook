//! The board's events section: the day timeline as fractions of its width, and the event rows. Pure.

use chrono::{Datelike, Days, NaiveDate, NaiveDateTime, NaiveTime};
use serde::Serialize;
use ts_rs::TS;

use crate::calendar::MeetingLink;
use crate::calendar::events::{Event, event_meeting_link};
use crate::clock::{minute_of_day, parse_clock};
use crate::store::settings::{Timeline, WorkHours};

const DAY_MINUTES: u32 = 24 * 60;

/// From and to as fractions of the timeline's width, 0 to 1.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Span {
    pub from: f64,
    pub to: f64,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct TimelineHour {
    /// "09".
    pub label: String,
    pub at: f64,
    /// Every hour has a tick; labels skip hours so they never crowd.
    pub labeled: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct TimelineBlock {
    /// The event row's key.
    pub key: String,
    pub span: Span,
    /// "09:00–09:30 Standup".
    pub label: String,
    pub past: bool,
    /// Another event runs up to or into its start.
    pub follows: bool,
}

/// The day as a ruler: the hours, the context's work hours, one block per event and now.
#[derive(Serialize, Clone, Debug, PartialEq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct DayTimeline {
    pub hours: Vec<TimelineHour>,
    /// The current context's work hours on this day.
    pub work_hours: Option<Span>,
    /// Events that fill the whole day: a line along the bottom instead of blocks.
    pub full_day: Vec<String>,
    pub blocks: Vec<TimelineBlock>,
    /// Today only, while within the shown hours.
    pub now: Option<f64>,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct EventRow {
    pub key: String,
    /// "09:00–09:30" or "All day".
    pub time: String,
    pub title: String,
    pub calendar: String,
    /// The call it links to; the row then joins it.
    pub meeting: Option<MeetingLink>,
    /// Over at the time of the view.
    pub past: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct DayEvents {
    pub timeline: DayTimeline,
    pub rows: Vec<EventRow>,
}

/// The shown hours, in minutes.
struct Ruler {
    first: u32,
    last: u32,
}

impl Ruler {
    fn new(timeline: &Timeline) -> Self {
        Self {
            first: u32::from(timeline.first_hour) * 60,
            last: u32::from(timeline.last_hour) * 60,
        }
    }

    fn at(&self, minutes: u32) -> f64 {
        let span = f64::from(self.last - self.first);
        ((f64::from(minutes) - f64::from(self.first)) / span).clamp(0.0, 1.0)
    }

    fn span(&self, (from, to): (u32, u32)) -> Option<Span> {
        (to > self.first && from < self.last).then(|| Span {
            from: self.at(from),
            to: self.at(to),
        })
    }

    /// Labels every 1, 2 or 3 hours, so they never crowd.
    fn hour_ticks(&self) -> Vec<TimelineHour> {
        let (first, last) = (self.first / 60, self.last / 60);
        let step = match last - first {
            0..=8 => 1,
            9..=16 => 2,
            _ => 3,
        };
        (first..=last)
            .map(|hour| TimelineHour {
                label: format!("{:02}", hour % 24),
                at: self.at(hour * 60),
                labeled: (hour - first) % step == 0,
            })
            .collect()
    }
}

fn clock(time: NaiveDateTime) -> String {
    time.format("%H:%M").to_string()
}

/// The part of an event on `day`, in minutes: an end at midnight or later is the end of the day.
fn day_span(event: &Event, day: NaiveDate) -> (u32, u32) {
    let start = day.and_time(NaiveTime::MIN);
    let end = start + Days::new(1);
    let from = if event.start <= start {
        0
    } else {
        minute_of_day(event.start)
    };
    let to = if event.end >= end {
        DAY_MINUTES
    } else {
        minute_of_day(event.end)
    };
    (from, to)
}

/// An end at or before the start (e.g. bedtime after midnight) runs to the end of the day.
fn work_span(work_hours: &WorkHours, day: NaiveDate) -> Option<(u32, u32)> {
    let weekday = day.weekday().num_days_from_sunday() as u8;
    if !work_hours.days.is_empty() && !work_hours.days.contains(&weekday) {
        return None;
    }
    let from = parse_clock(&work_hours.from)?;
    let to = parse_clock(&work_hours.to)?;
    Some((from, if to > from { to } else { DAY_MINUTES }))
}

/// An event of the day with its row key and its minutes on that day.
struct Placed<'a> {
    key: String,
    event: &'a Event,
    span: (u32, u32),
}

impl Placed<'_> {
    fn is_full_day(&self) -> bool {
        self.span == (0, DAY_MINUTES)
    }
}

fn blocks(placed: &[Placed], ruler: &Ruler, now: NaiveDateTime) -> Vec<TimelineBlock> {
    let timed: Vec<&Placed> = placed.iter().filter(|p| !p.is_full_day()).collect();
    timed
        .iter()
        .filter_map(|p| {
            let from = p.span.0;
            let follows = timed
                .iter()
                .any(|other| other.key != p.key && other.span.0 < from && other.span.1 >= from);
            Some(TimelineBlock {
                key: p.key.clone(),
                span: ruler.span(p.span)?,
                label: format!(
                    "{}–{} {}",
                    clock(p.event.start),
                    clock(p.event.end),
                    p.event.title
                ),
                past: p.event.end <= now,
                follows,
            })
        })
        .collect()
}

fn row(p: &Placed, now: NaiveDateTime) -> EventRow {
    EventRow {
        key: p.key.clone(),
        time: if p.is_full_day() {
            "All day".into()
        } else {
            format!("{}–{}", clock(p.event.start), clock(p.event.end))
        },
        title: p.event.title.clone(),
        calendar: p.event.calendar.clone(),
        meeting: event_meeting_link(p.event),
        past: p.event.end <= now,
    }
}

pub struct DayInput<'a> {
    pub day: NaiveDate,
    pub now: NaiveDateTime,
    pub events: &'a [Event],
    pub timeline: &'a Timeline,
    pub work_hours: Option<&'a WorkHours>,
}

pub fn day_events(input: &DayInput) -> DayEvents {
    let DayInput {
        day,
        now,
        events,
        timeline,
        work_hours,
    } = *input;
    let ruler = Ruler::new(timeline);
    let placed: Vec<Placed> = events
        .iter()
        .enumerate()
        .map(|(index, event)| Placed {
            key: format!("event-{index}"),
            event,
            span: day_span(event, day),
        })
        .collect();
    let now_minute = minute_of_day(now);
    let shows_now = day == now.date() && (ruler.first..=ruler.last).contains(&now_minute);
    DayEvents {
        timeline: DayTimeline {
            hours: ruler.hour_ticks(),
            work_hours: work_hours
                .and_then(|work_hours| work_span(work_hours, day))
                .and_then(|span| ruler.span(span)),
            full_day: placed
                .iter()
                .filter(|p| p.is_full_day())
                .map(|p| p.key.clone())
                .collect(),
            blocks: blocks(&placed, &ruler, now),
            now: shows_now.then(|| ruler.at(now_minute)),
        },
        rows: placed.iter().map(|p| row(p, now)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::events::CallApp;

    fn at(day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    fn saturday() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 24).unwrap()
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

    fn timeline(first_hour: u8, last_hour: u8) -> Timeline {
        Timeline {
            first_hour,
            last_hour,
        }
    }

    fn work_hours(days: &[u8], from: &str, to: &str) -> WorkHours {
        WorkHours {
            days: days.to_vec(),
            from: from.into(),
            to: to.into(),
        }
    }

    fn view(
        now: NaiveDateTime,
        events: &[Event],
        timeline: &Timeline,
        work_hours: Option<&WorkHours>,
    ) -> DayEvents {
        day_events(&DayInput {
            day: saturday(),
            now,
            events,
            timeline,
            work_hours,
        })
    }

    fn plain(events: &[Event]) -> DayEvents {
        view(at(24, 12, 0), events, &timeline(0, 24), None)
    }

    #[test]
    fn a_block_is_clipped_to_the_shown_hours() {
        let events = [event("Early", at(24, 7, 0), at(24, 9, 0))];
        let day = view(at(24, 12, 0), &events, &timeline(8, 18), None);
        assert_eq!(day.timeline.blocks.len(), 1);
        assert_eq!(day.timeline.blocks[0].span, Span { from: 0.0, to: 0.1 });
    }

    #[test]
    fn an_event_outside_the_shown_hours_has_a_row_but_no_block() {
        let events = [event("Late", at(24, 19, 0), at(24, 20, 0))];
        let day = view(at(24, 12, 0), &events, &timeline(8, 18), None);
        assert!(day.timeline.blocks.is_empty());
        assert_eq!(day.rows.len(), 1);
        assert_eq!(day.rows[0].time, "19:00–20:00");
    }

    #[test]
    fn work_hours_across_midnight_run_to_the_end_of_the_day() {
        let hours = work_hours(&[], "22:00", "02:00");
        let day = view(at(24, 12, 0), &[], &timeline(0, 24), Some(&hours));
        assert_eq!(
            day.timeline.work_hours,
            Some(Span {
                from: 1320.0 / 1440.0,
                to: 1.0
            })
        );
    }

    #[test]
    fn work_hours_ending_when_they_start_run_to_the_end_of_the_day() {
        let hours = work_hours(&[], "12:00", "12:00");
        let day = view(at(24, 12, 0), &[], &timeline(0, 24), Some(&hours));
        assert_eq!(day.timeline.work_hours, Some(Span { from: 0.5, to: 1.0 }));
    }

    #[test]
    fn work_hours_are_clipped_to_the_shown_hours() {
        let hours = work_hours(&[], "09:00", "17:00");
        let day = view(at(24, 12, 0), &[], &timeline(8, 18), Some(&hours));
        assert_eq!(
            day.timeline.work_hours,
            Some(Span {
                from: 60.0 / 600.0,
                to: 540.0 / 600.0
            })
        );
    }

    #[test]
    fn work_hours_show_only_on_their_weekdays() {
        let weekdays = work_hours(&[1, 2, 3, 4, 5], "09:00", "17:00");
        let with_saturday = work_hours(&[6], "09:00", "17:00");
        let every_day = work_hours(&[], "09:00", "17:00");
        let shown = |hours: &WorkHours| {
            view(at(24, 12, 0), &[], &timeline(0, 24), Some(hours))
                .timeline
                .work_hours
                .is_some()
        };
        assert!(!shown(&weekdays));
        assert!(shown(&with_saturday));
        assert!(shown(&every_day));
    }

    #[test]
    fn sunday_is_weekday_zero_for_work_hours() {
        let sunday = work_hours(&[0], "09:00", "17:00");
        let day = day_events(&DayInput {
            day: NaiveDate::from_ymd_opt(2026, 10, 25).unwrap(),
            now: at(25, 12, 0),
            events: &[],
            timeline: &timeline(0, 24),
            work_hours: Some(&sunday),
        });
        assert!(day.timeline.work_hours.is_some());
    }

    #[test]
    fn without_work_hours_nothing_is_tinted() {
        assert_eq!(plain(&[]).timeline.work_hours, None);
    }

    #[test]
    fn the_now_line_shows_on_today_within_the_shown_hours() {
        let day = view(at(24, 12, 0), &[], &timeline(8, 18), None);
        assert_eq!(day.timeline.now, Some(240.0 / 600.0));
    }

    #[test]
    fn the_now_line_is_hidden_outside_the_shown_hours() {
        let day = view(at(24, 6, 30), &[], &timeline(8, 18), None);
        assert_eq!(day.timeline.now, None);
    }

    #[test]
    fn the_now_line_is_hidden_on_another_day() {
        let day = view(at(25, 12, 0), &[], &timeline(0, 24), None);
        assert_eq!(day.timeline.now, None);
    }

    #[test]
    fn the_hours_run_from_the_first_to_the_last_inclusive() {
        let hours = view(at(24, 12, 0), &[], &timeline(9, 12), None)
            .timeline
            .hours;
        let shown: Vec<(&str, f64, bool)> = hours
            .iter()
            .map(|h| (h.label.as_str(), h.at, h.labeled))
            .collect();
        assert_eq!(
            shown,
            [
                ("09", 0.0, true),
                ("10", 1.0 / 3.0, true),
                ("11", 2.0 / 3.0, true),
                ("12", 1.0, true),
            ]
        );
    }

    #[test]
    fn hour_24_is_labeled_00() {
        let hours = plain(&[]).timeline.hours;
        assert_eq!(hours.len(), 25);
        assert_eq!(hours[24].label, "00");
        assert_eq!(hours[24].at, 1.0);
    }

    #[test]
    fn labels_skip_hours_as_more_hours_are_shown() {
        let labeled = |first: u8, last: u8| -> Vec<String> {
            view(at(24, 12, 0), &[], &timeline(first, last), None)
                .timeline
                .hours
                .into_iter()
                .filter(|hour| hour.labeled)
                .map(|hour| hour.label)
                .collect()
        };
        assert_eq!(labeled(8, 16).len(), 9, "8 hours: every hour");
        assert_eq!(
            labeled(8, 24),
            ["08", "10", "12", "14", "16", "18", "20", "22", "00"],
            "16 hours: every 2nd"
        );
        assert_eq!(
            labeled(0, 24),
            ["00", "03", "06", "09", "12", "15", "18", "21", "00"],
            "24 hours: every 3rd"
        );
        assert_eq!(
            labeled(7, 24).first().map(String::as_str),
            Some("07"),
            "17 hours: the first is labeled"
        );
    }

    #[test]
    fn an_event_filling_the_day_is_a_full_day_line_without_a_block() {
        let events = [event("Holiday", at(24, 0, 0), at(25, 0, 0))];
        let day = plain(&events);
        assert_eq!(day.timeline.full_day, ["event-0"]);
        assert!(day.timeline.blocks.is_empty());
        assert_eq!(day.rows[0].time, "All day");
    }

    #[test]
    fn an_event_spanning_past_both_ends_of_the_day_is_a_full_day_event() {
        let events = [event("Trip", at(23, 18, 0), at(26, 9, 0))];
        let day = plain(&events);
        assert_eq!(day.timeline.full_day, ["event-0"]);
        assert_eq!(day.rows[0].time, "All day");
    }

    #[test]
    fn an_event_ending_at_midnight_runs_to_the_end_of_the_day() {
        let events = [event("Dinner", at(24, 20, 30), at(25, 0, 0))];
        let day = plain(&events);
        assert_eq!(day.rows[0].time, "20:30–00:00");
        assert!(day.timeline.full_day.is_empty());
        assert_eq!(
            day.timeline.blocks[0].span,
            Span {
                from: 1230.0 / 1440.0,
                to: 1.0
            }
        );
    }

    #[test]
    fn an_event_starting_the_day_before_starts_the_block_at_the_left_edge() {
        let events = [event("Night", at(23, 22, 0), at(24, 2, 0))];
        let day = plain(&events);
        assert_eq!(
            day.timeline.blocks[0].span,
            Span {
                from: 0.0,
                to: 120.0 / 1440.0
            }
        );
    }

    #[test]
    fn a_block_following_another_event_is_marked() {
        let events = [
            event("First", at(24, 9, 0), at(24, 9, 30)),
            event("Second", at(24, 9, 30), at(24, 10, 0)),
        ];
        let blocks = plain(&events).timeline.blocks;
        assert!(!blocks[0].follows);
        assert!(blocks[1].follows);
    }

    #[test]
    fn a_block_starting_inside_another_event_is_marked() {
        let events = [
            event("Long", at(24, 9, 0), at(24, 10, 0)),
            event("Overlap", at(24, 9, 30), at(24, 10, 30)),
        ];
        let blocks = plain(&events).timeline.blocks;
        assert!(!blocks[0].follows);
        assert!(blocks[1].follows);
    }

    #[test]
    fn events_with_a_gap_between_them_do_not_follow() {
        let events = [
            event("First", at(24, 9, 0), at(24, 9, 30)),
            event("Second", at(24, 10, 0), at(24, 10, 30)),
        ];
        let blocks = plain(&events).timeline.blocks;
        assert!(blocks.iter().all(|block| !block.follows));
    }

    #[test]
    fn an_event_that_ended_by_now_is_past_in_row_and_block() {
        let events = [
            event("Over", at(24, 9, 0), at(24, 9, 30)),
            event("Running", at(24, 9, 15), at(24, 10, 0)),
        ];
        let day = view(at(24, 9, 30), &events, &timeline(0, 24), None);
        assert_eq!(
            day.rows.iter().map(|row| row.past).collect::<Vec<_>>(),
            [true, false]
        );
        assert_eq!(
            day.timeline
                .blocks
                .iter()
                .map(|block| block.past)
                .collect::<Vec<_>>(),
            [true, false]
        );
    }

    #[test]
    fn rows_keep_the_events_order_and_their_keys_match_the_blocks() {
        let events = [
            event("Standup", at(24, 9, 0), at(24, 9, 30)),
            event("Holiday", at(24, 0, 0), at(25, 0, 0)),
            event("Review", at(24, 14, 0), at(24, 15, 0)),
        ];
        let day = plain(&events);
        assert_eq!(
            day.rows
                .iter()
                .map(|row| (row.key.as_str(), row.title.as_str()))
                .collect::<Vec<_>>(),
            [
                ("event-0", "Standup"),
                ("event-1", "Holiday"),
                ("event-2", "Review")
            ]
        );
        assert_eq!(
            day.timeline
                .blocks
                .iter()
                .map(|block| block.key.as_str())
                .collect::<Vec<_>>(),
            ["event-0", "event-2"]
        );
        assert_eq!(day.timeline.full_day, ["event-1"]);
    }

    #[test]
    fn a_row_shows_its_time_calendar_and_a_block_its_label() {
        let events = [event("Standup", at(24, 9, 0), at(24, 9, 30))];
        let day = plain(&events);
        assert_eq!(day.rows[0].time, "09:00–09:30");
        assert_eq!(day.rows[0].calendar, "Work");
        assert_eq!(day.timeline.blocks[0].label, "09:00–09:30 Standup");
    }

    #[test]
    fn a_row_carries_the_join_link_of_its_event() {
        let mut call = event("Standup", at(24, 9, 0), at(24, 9, 30));
        call.url = Some("https://meet.google.com/abc-defg-hij".into());
        let events = [call, event("Lunch", at(24, 12, 0), at(24, 13, 0))];
        let rows = plain(&events).rows;
        assert_eq!(
            rows[0].meeting,
            Some(MeetingLink {
                app: CallApp::Meet,
                url: "https://meet.google.com/abc-defg-hij".into()
            })
        );
        assert_eq!(rows[1].meeting, None);
    }

    #[test]
    fn the_timeline_serializes_with_camel_case_fields() {
        let hours = work_hours(&[], "09:00", "10:00");
        let events = [event("Standup", at(24, 9, 0), at(24, 9, 30))];
        let day = view(at(24, 12, 0), &events, &timeline(8, 10), Some(&hours));
        assert_eq!(
            serde_json::to_value(&day.timeline).unwrap(),
            serde_json::json!({
                "hours": [
                    { "label": "08", "at": 0.0, "labeled": true },
                    { "label": "09", "at": 0.5, "labeled": true },
                    { "label": "10", "at": 1.0, "labeled": true },
                ],
                "workHours": { "from": 0.5, "to": 1.0 },
                "fullDay": [],
                "blocks": [{
                    "key": "event-0",
                    "span": { "from": 0.5, "to": 0.75 },
                    "label": "09:00–09:30 Standup",
                    "past": true,
                    "follows": false,
                }],
                "now": null,
            })
        );
    }
}
