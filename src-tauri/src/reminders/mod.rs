//! The Daybook window's reminder: the first enabled rule of Settings → Reminders that applies now. Pure.

use chrono::{Datelike, Days, Duration, NaiveDate, NaiveDateTime};

use crate::calendar::MeetingLink;
use crate::calendar::events::{Event, event_meeting_link, running_event};
use crate::clock::is_within_window;
use crate::store::settings::{ReminderRule, TimeWindow, TodoSpan};

/// A routine's progress today.
pub struct ReminderRoutine {
    pub id: String,
    pub name: String,
    pub done: u32,
    pub total: u32,
    pub due: Option<TimeWindow>,
}

/// A journal group's progress today.
pub struct ReminderJournalGroup {
    pub title: String,
    pub answered: u32,
    pub total: u32,
    /// A free-text group was written.
    pub has_note: bool,
    pub due: Option<TimeWindow>,
}

pub struct ReminderInput<'a> {
    pub now: NaiveDateTime,
    pub rules: &'a [ReminderRule],
    /// Today's timed events from the current context's calendars.
    pub events: &'a [Event],
    /// The current context's routines (none without the Routines feature).
    pub routines: &'a [ReminderRoutine],
    pub journal: &'a [ReminderJournalGroup],
    /// The days of the open todos in the daily notes.
    pub todo_dates: &'a [NaiveDate],
}

#[derive(Clone, Debug, PartialEq)]
pub enum Reminder {
    Meeting {
        title: String,
        start: NaiveDateTime,
        end: NaiveDateTime,
        meeting: Option<MeetingLink>,
        running: bool,
    },
    Routine {
        routine: String,
        name: String,
        open: u32,
        started: bool,
    },
    Journal {
        group: String,
        open: u32,
        /// The group has questions; a free-text group has none.
        questions: bool,
    },
    Todos {
        count: usize,
        span: TodoSpan,
    },
}

/// Every day, from `from` until `to`.
fn in_window(window: &TimeWindow, now: NaiveDateTime) -> bool {
    const EVERY_DAY: &[u8] = &[];
    is_within_window(EVERY_DAY, &window.from, &window.to, now)
}

fn is_due(due: Option<&TimeWindow>, now: NaiveDateTime) -> bool {
    due.is_some_and(|due| in_window(due, now))
}

/// A meeting in progress wins (the latest started); the next one only without it.
fn meeting_reminder(minutes_before: u32, input: &ReminderInput) -> Option<Reminder> {
    let now = input.now;
    let soon = now + Duration::minutes(i64::from(minutes_before));
    let running = running_event(input.events, now);
    let next = || {
        input
            .events
            .iter()
            .filter(|event| now < event.start && event.start <= soon)
            .min_by_key(|event| event.start)
    };
    let shown = running.or_else(next)?;
    Some(Reminder::Meeting {
        title: shown.title.clone(),
        start: shown.start,
        end: shown.end,
        meeting: event_meeting_link(shown),
        running: running.is_some(),
    })
}

fn routine_reminder(id: &str, input: &ReminderInput) -> Option<Reminder> {
    let routine = input.routines.iter().find(|routine| routine.id == id)?;
    if routine.done >= routine.total || !is_due(routine.due.as_ref(), input.now) {
        return None;
    }
    Some(Reminder::Routine {
        routine: routine.id.clone(),
        name: routine.name.clone(),
        open: routine.total - routine.done,
        started: routine.done > 0,
    })
}

fn journal_open(group: &ReminderJournalGroup) -> u32 {
    if group.total > 0 {
        group.total.saturating_sub(group.answered)
    } else {
        u32::from(!group.has_note)
    }
}

fn journal_reminder(title: &str, input: &ReminderInput) -> Option<Reminder> {
    let group = input.journal.iter().find(|group| group.title == title)?;
    let open = journal_open(group);
    if open == 0 || !is_due(group.due.as_ref(), input.now) {
        return None;
    }
    Some(Reminder::Journal {
        group: group.title.clone(),
        open,
        questions: group.total > 0,
    })
}

/// The first day a span covers; weeks start on Monday. `None`: every day.
fn span_start(span: TodoSpan, today: NaiveDate) -> Option<NaiveDate> {
    match span {
        TodoSpan::Today => Some(today),
        TodoSpan::Week => {
            today.checked_sub_days(Days::new(today.weekday().num_days_from_monday().into()))
        }
        TodoSpan::Month => today.with_day(1),
        TodoSpan::All => None,
    }
}

fn todos_reminder(span: TodoSpan, window: &TimeWindow, input: &ReminderInput) -> Option<Reminder> {
    if !in_window(window, input.now) {
        return None;
    }
    let today = input.now.date();
    let from = span_start(span, today);
    let count = input
        .todo_dates
        .iter()
        .filter(|&&date| from.is_none_or(|start| date >= start) && date <= today)
        .count();
    (count > 0).then_some(Reminder::Todos { count, span })
}

fn reminder_for(rule: &ReminderRule, input: &ReminderInput) -> Option<Reminder> {
    match rule {
        ReminderRule::Meeting { minutes_before, .. } => meeting_reminder(*minutes_before, input),
        ReminderRule::Routine { routine, .. } => routine_reminder(routine, input),
        ReminderRule::Journal { group, .. } => journal_reminder(group, input),
        ReminderRule::Todos { span, from, to, .. } => {
            todos_reminder(*span, &TimeWindow::new(from, to), input)
        }
    }
}

fn enabled(rule: &ReminderRule) -> bool {
    match rule {
        ReminderRule::Meeting { enabled, .. }
        | ReminderRule::Routine { enabled, .. }
        | ReminderRule::Journal { enabled, .. }
        | ReminderRule::Todos { enabled, .. } => *enabled,
    }
}

/// The first enabled rule in the list that applies now.
pub fn pick_reminder(input: &ReminderInput) -> Option<Reminder> {
    input
        .rules
        .iter()
        .filter(|rule| enabled(rule))
        .find_map(|rule| reminder_for(rule, input))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::events::CallApp;

    fn at(hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 29)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    fn day(text: &str) -> NaiveDate {
        text.parse().unwrap()
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

    fn meeting_rule() -> ReminderRule {
        ReminderRule::Meeting {
            enabled: true,
            minutes_before: 60,
        }
    }

    fn routine_rule() -> ReminderRule {
        ReminderRule::Routine {
            enabled: true,
            routine: "evening".into(),
        }
    }

    fn journal_rule(group: &str) -> ReminderRule {
        ReminderRule::Journal {
            enabled: true,
            group: group.into(),
        }
    }

    fn disabled(rule: ReminderRule) -> ReminderRule {
        match rule {
            ReminderRule::Meeting { minutes_before, .. } => ReminderRule::Meeting {
                enabled: false,
                minutes_before,
            },
            ReminderRule::Routine { routine, .. } => ReminderRule::Routine {
                enabled: false,
                routine,
            },
            ReminderRule::Journal { group, .. } => ReminderRule::Journal {
                enabled: false,
                group,
            },
            ReminderRule::Todos { span, from, to, .. } => ReminderRule::Todos {
                enabled: false,
                span,
                from,
                to,
            },
        }
    }

    fn rules() -> Vec<ReminderRule> {
        vec![meeting_rule(), routine_rule(), journal_rule("Dusk")]
    }

    fn sprint_review() -> Event {
        event("Sprint Review", at(15, 0), at(15, 30))
    }

    fn evening() -> ReminderRoutine {
        ReminderRoutine {
            id: "evening".into(),
            name: "Evening".into(),
            done: 1,
            total: 3,
            due: Some(TimeWindow::new("17:30", "")),
        }
    }

    fn abend() -> ReminderJournalGroup {
        ReminderJournalGroup {
            title: "Dusk".into(),
            answered: 1,
            total: 3,
            has_note: false,
            due: Some(TimeWindow::new("14:00", "")),
        }
    }

    struct World {
        now: NaiveDateTime,
        rules: Vec<ReminderRule>,
        events: Vec<Event>,
        routines: Vec<ReminderRoutine>,
        journal: Vec<ReminderJournalGroup>,
        todo_dates: Vec<NaiveDate>,
    }

    impl World {
        fn new() -> Self {
            Self {
                now: at(14, 30),
                rules: rules(),
                events: vec![sprint_review()],
                routines: vec![evening()],
                journal: vec![abend()],
                todo_dates: vec![],
            }
        }

        fn pick(&self) -> Option<Reminder> {
            pick_reminder(&ReminderInput {
                now: self.now,
                rules: &self.rules,
                events: &self.events,
                routines: &self.routines,
                journal: &self.journal,
                todo_dates: &self.todo_dates,
            })
        }
    }

    fn shown_meeting(world: &World) -> Option<(String, bool)> {
        match world.pick() {
            Some(Reminder::Meeting { title, running, .. }) => Some((title, running)),
            _ => None,
        }
    }

    #[test]
    fn an_upcoming_meeting_wins() {
        assert_eq!(
            World::new().pick(),
            Some(Reminder::Meeting {
                title: "Sprint Review".into(),
                start: at(15, 0),
                end: at(15, 30),
                meeting: None,
                running: false,
            })
        );
    }

    #[test]
    fn a_meetings_reminder_carries_its_call() {
        let mut world = World::new();
        world.events[0].url = Some("https://zoom.us/j/1".into());
        let Some(Reminder::Meeting { meeting, .. }) = world.pick() else {
            panic!("expected a meeting reminder");
        };
        assert_eq!(
            meeting,
            Some(MeetingLink {
                app: CallApp::Zoom,
                url: "https://zoom.us/j/1".into(),
            })
        );
    }

    #[test]
    fn a_started_routine_only_shows_once_its_time_has_come() {
        let mut world = World::new();
        world.events.clear();
        assert!(matches!(world.pick(), Some(Reminder::Journal { .. })));
        world.now = at(18, 0);
        assert_eq!(
            world.pick(),
            Some(Reminder::Routine {
                routine: "evening".into(),
                name: "Evening".into(),
                open: 2,
                started: true,
            })
        );
    }

    #[test]
    fn done_items_and_rules_that_are_off_are_skipped() {
        let mut world = World::new();
        world.events.clear();
        world.journal[0].answered = 3;
        assert_eq!(world.pick(), None);

        let mut world = World::new();
        world.rules = vec![
            disabled(meeting_rule()),
            routine_rule(),
            journal_rule("Dusk"),
        ];
        assert_eq!(
            world.pick(),
            Some(Reminder::Journal {
                group: "Dusk".into(),
                open: 2,
                questions: true,
            })
        );
    }

    #[test]
    fn a_meeting_in_progress_wins_over_the_next_one_until_it_ends() {
        let mut world = World::new();
        world.now = at(15, 10);
        world.events.push(event("Planning", at(15, 30), at(16, 0)));
        assert_eq!(shown_meeting(&world), Some(("Sprint Review".into(), true)));
        world.now = at(15, 30);
        assert_eq!(shown_meeting(&world), Some(("Planning".into(), true)));
        world.now = at(16, 0);
        assert_eq!(shown_meeting(&world), None);
    }

    #[test]
    fn a_free_text_journal_group_counts_as_done_once_written() {
        let mut world = World::new();
        world.events.clear();
        world.rules = vec![journal_rule("Health")];
        world.journal = vec![ReminderJournalGroup {
            title: "Health".into(),
            answered: 0,
            total: 0,
            has_note: false,
            due: Some(TimeWindow::new("09:00", "")),
        }];
        assert_eq!(
            world.pick(),
            Some(Reminder::Journal {
                group: "Health".into(),
                open: 1,
                questions: false,
            })
        );
        world.journal[0].has_note = true;
        assert_eq!(world.pick(), None);
    }

    #[test]
    fn the_list_order_decides() {
        let mut world = World::new();
        world.rules = vec![journal_rule("Dusk"), meeting_rule()];
        assert!(matches!(world.pick(), Some(Reminder::Journal { .. })));
    }

    #[test]
    fn a_routine_without_items_today_is_not_reminded_of() {
        let mut world = World::new();
        world.now = at(18, 0);
        world.events.clear();
        world.rules = vec![routine_rule()];
        world.routines[0].done = 0;
        world.routines[0].total = 0;
        assert_eq!(world.pick(), None);
    }

    #[test]
    fn open_todos_are_counted_in_the_chosen_span_and_not_in_the_future() {
        let mut world = World::new();
        world.events.clear();
        world.rules = vec![ReminderRule::Todos {
            enabled: true,
            span: TodoSpan::Week,
            from: "09:00".into(),
            to: String::new(),
        }];
        world.todo_dates = ["2026-09-29", "2026-09-28", "2026-09-27", "2026-11-26"]
            .map(day)
            .to_vec();
        assert_eq!(
            world.pick(),
            Some(Reminder::Todos {
                count: 2,
                span: TodoSpan::Week,
            })
        );
    }

    #[test]
    fn a_span_starts_on_the_first_day_it_covers() {
        let today = day("2026-09-29");
        for (span, expected) in [
            (TodoSpan::Today, Some(day("2026-09-29"))),
            (TodoSpan::Week, Some(day("2026-09-28"))),
            (TodoSpan::Month, Some(day("2026-09-01"))),
            (TodoSpan::All, None),
        ] {
            assert_eq!(span_start(span, today), expected, "{span:?}");
        }
    }

    #[test]
    fn a_routine_or_group_without_a_due_time_is_never_reminded_of() {
        let mut world = World::new();
        world.now = at(18, 0);
        world.events.clear();
        world.routines[0].due = None;
        world.journal.clear();
        assert_eq!(world.pick(), None);
    }
}
