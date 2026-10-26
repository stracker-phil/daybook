//! When a routine's checklist or a journal group opens by itself (their `Timing`), and which one opens. Pure.

use chrono::NaiveDateTime;

use crate::clock::is_within_window;
use crate::store::settings::{OpenEvent, Timing};

const EVERY_DAY: &[u8] = &[];

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    Routine(String),
    Journal(String),
}

/// Why something might open now: its due time, or an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reason {
    Due,
    Event(OpenEvent),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    pub target: Target,
    pub timing: Timing,
}

const REASONS: [(Reason, &str); 6] = [
    (Reason::Due, "due"),
    (Reason::Event(OpenEvent::Start), "start"),
    (Reason::Event(OpenEvent::Context), "context"),
    (Reason::Event(OpenEvent::MeetingStart), "meeting-start"),
    (Reason::Event(OpenEvent::MeetingEnd), "meeting-end"),
    (Reason::Event(OpenEvent::Off), "off"),
];

impl Reason {
    pub fn name(self) -> &'static str {
        REASONS
            .iter()
            .find(|(reason, _)| *reason == self)
            .map_or("due", |(_, name)| name)
    }

    pub fn from_name(name: &str) -> Option<Self> {
        REASONS
            .iter()
            .find(|(_, known)| *known == name)
            .map(|(reason, _)| *reason)
    }
}

/// The key a once-a-day opening is remembered under in `routines.json`, e.g. `routine:morning@due`; the
/// prototype wrote the same keys.
pub fn fired_key(target: &Target, reason: Reason) -> String {
    let target = match target {
        Target::Routine(id) => format!("routine:{id}"),
        Target::Journal(group) => format!("journal:{group}"),
    };
    format!("{target}@{}", reason.name())
}

/// Whether `reason` opens `timing` now: a due time while its window is open (also after the Mac slept
/// through its start), or an event it lists.
pub fn opens_for(timing: &Timing, reason: Reason, now: NaiveDateTime) -> bool {
    match reason {
        Reason::Due => {
            timing.open_when_due
                && timing
                    .due
                    .as_ref()
                    .is_some_and(|due| is_within_window(EVERY_DAY, &due.from, &due.to, now))
        }
        Reason::Event(event) => timing.open_on.contains(&event),
    }
}

pub fn fires_once_per_day(reason: Reason) -> bool {
    matches!(
        reason,
        Reason::Due | Reason::Event(OpenEvent::Start | OpenEvent::Context)
    )
}

/// The first prompt that opens for `reason` and still has something to do; the others wait for their next
/// chance. `mark` remembers a once-a-day opening (`false`: it fired today already), `pending` tells
/// whether the target still has something open. A prompt counts as fired even when nothing was open.
pub fn first_to_open(
    prompts: &[Prompt],
    reason: Reason,
    now: NaiveDateTime,
    mut mark: impl FnMut(&str) -> bool,
    mut pending: impl FnMut(&Target) -> bool,
) -> Option<Target> {
    for prompt in prompts {
        if !opens_for(&prompt.timing, reason, now) {
            continue;
        }
        let claimed = !fires_once_per_day(reason) || mark(&fired_key(&prompt.target, reason));
        if claimed && pending(&prompt.target) {
            return Some(prompt.target.clone());
        }
    }
    None
}

/// The events that came in while the wizard couldn't open anything yet.
#[derive(Default, Debug)]
pub struct Waiting {
    events: Vec<OpenEvent>,
}

impl Waiting {
    pub fn push(&mut self, event: OpenEvent) {
        if !self.events.contains(&event) {
            self.events.push(event);
        }
    }

    /// The next event to try: none while away (they are dropped: nothing opens for what happened while
    /// away) or while the overlay shows something else (they wait).
    pub fn next(&mut self, away: bool, busy: bool) -> Option<OpenEvent> {
        if away {
            self.events.clear();
        }
        if away || busy || self.events.is_empty() {
            return None;
        }
        Some(self.events.remove(0))
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use chrono::NaiveDate;

    use super::*;

    fn at(hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 9, 28)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    fn on_events(open_on: Vec<OpenEvent>) -> Timing {
        Timing {
            open_on,
            ..Timing::default()
        }
    }

    fn routine(id: &str, timing: Timing) -> Prompt {
        Prompt {
            target: Target::Routine(id.into()),
            timing,
        }
    }

    fn journal(group: &str, timing: Timing) -> Prompt {
        Prompt {
            target: Target::Journal(group.into()),
            timing,
        }
    }

    fn open(
        prompts: &[Prompt],
        reason: Reason,
        now: NaiveDateTime,
        fired: &mut HashSet<String>,
        pending: bool,
    ) -> Option<Target> {
        first_to_open(
            prompts,
            reason,
            now,
            |key| fired.insert(key.to_string()),
            |_| pending,
        )
    }

    #[test]
    fn a_due_window_opens_inside_it_and_not_before_or_at_its_end() {
        let morning = Timing::new("09:45", "13:00", vec![]);
        for (now, expected) in [
            (at(9, 44), false),
            (at(9, 45), true),
            (at(11, 0), true),
            (at(13, 0), false),
        ] {
            assert_eq!(opens_for(&morning, Reason::Due, now), expected, "{now}");
        }
    }

    #[test]
    fn a_due_time_without_an_end_stays_open_until_the_end_of_the_day() {
        let evening = Timing::new("18:00", "", vec![]);
        assert!(opens_for(&evening, Reason::Due, at(23, 30)));
    }

    #[test]
    fn a_due_time_does_not_open_unless_asked_to_or_without_a_due_time() {
        let not_asked = Timing {
            open_when_due: false,
            ..Timing::new("09:00", "", vec![])
        };
        assert!(!opens_for(&not_asked, Reason::Due, at(10, 0)));

        let no_due = Timing {
            due: None,
            open_when_due: true,
            open_on: vec![],
        };
        assert!(!opens_for(&no_due, Reason::Due, at(10, 0)));
    }

    #[test]
    fn events_open_only_what_lists_them() {
        let evening = on_events(vec![OpenEvent::Off]);
        assert!(opens_for(
            &evening,
            Reason::Event(OpenEvent::Off),
            at(18, 0)
        ));
        assert!(!opens_for(
            &evening,
            Reason::Event(OpenEvent::Start),
            at(9, 0)
        ));
    }

    #[test]
    fn only_the_due_time_start_and_context_open_once_a_day() {
        for (reason, expected) in [
            (Reason::Due, true),
            (Reason::Event(OpenEvent::Start), true),
            (Reason::Event(OpenEvent::Context), true),
            (Reason::Event(OpenEvent::MeetingStart), false),
            (Reason::Event(OpenEvent::MeetingEnd), false),
            (Reason::Event(OpenEvent::Off), false),
        ] {
            assert_eq!(fires_once_per_day(reason), expected, "{reason:?}");
        }
    }

    #[test]
    fn fired_keys_follow_the_prototype_spelling() {
        for (target, reason, expected) in [
            (
                Target::Routine("morning".into()),
                Reason::Due,
                "routine:morning@due",
            ),
            (
                Target::Journal("Dusk".into()),
                Reason::Event(OpenEvent::Off),
                "journal:Dusk@off",
            ),
            (
                Target::Routine("standup".into()),
                Reason::Event(OpenEvent::MeetingEnd),
                "routine:standup@meeting-end",
            ),
            (
                Target::Routine("x".into()),
                Reason::Event(OpenEvent::MeetingStart),
                "routine:x@meeting-start",
            ),
        ] {
            assert_eq!(fired_key(&target, reason), expected);
        }
    }

    #[test]
    fn a_due_routine_opens_once_per_day() {
        let prompts = [routine("morning", Timing::new("09:45", "13:00", vec![]))];
        let mut fired = HashSet::new();

        assert_eq!(
            open(&prompts, Reason::Due, at(11, 0), &mut fired, true),
            Some(Target::Routine("morning".into()))
        );
        assert_eq!(
            open(&prompts, Reason::Due, at(11, 5), &mut fired, true),
            None
        );
        assert_eq!(fired, HashSet::from(["routine:morning@due".to_string()]));
    }

    #[test]
    fn events_that_repeat_open_every_time_and_remember_nothing() {
        let prompts = [journal(
            "Dusk",
            on_events(vec![OpenEvent::Off, OpenEvent::MeetingEnd]),
        )];
        for event in [OpenEvent::Off, OpenEvent::MeetingEnd] {
            let mut fired = HashSet::new();
            for _ in 0..2 {
                assert_eq!(
                    open(&prompts, Reason::Event(event), at(18, 0), &mut fired, true),
                    Some(Target::Journal("Dusk".into())),
                    "{event:?}"
                );
            }
            assert!(fired.is_empty(), "{event:?}");
        }
    }

    #[test]
    fn a_prompt_with_nothing_pending_counts_as_fired_for_the_day() {
        let prompts = [routine("morning", Timing::new("09:45", "13:00", vec![]))];
        let mut fired = HashSet::new();

        assert_eq!(
            open(&prompts, Reason::Due, at(10, 0), &mut fired, false),
            None
        );
        assert!(fired.contains("routine:morning@due"));
        assert_eq!(
            open(&prompts, Reason::Due, at(10, 30), &mut fired, true),
            None
        );
    }

    #[test]
    fn prompts_that_do_not_open_for_the_reason_are_skipped_and_not_marked() {
        let prompts = [
            routine("late", Timing::new("18:00", "", vec![])),
            routine("morning", Timing::new("09:45", "13:00", vec![])),
        ];
        let mut fired = HashSet::new();

        assert_eq!(
            open(&prompts, Reason::Due, at(11, 0), &mut fired, true),
            Some(Target::Routine("morning".into()))
        );
        assert_eq!(fired, HashSet::from(["routine:morning@due".to_string()]));
    }

    #[test]
    fn the_next_matching_prompt_opens_when_the_first_has_nothing_pending() {
        let prompts = [
            routine("morning", Timing::new("09:00", "13:00", vec![])),
            journal("Dusk", Timing::new("09:00", "13:00", vec![])),
        ];
        let mut fired = HashSet::new();

        let opened = first_to_open(
            &prompts,
            Reason::Due,
            at(10, 0),
            |key| fired.insert(key.to_string()),
            |target| matches!(target, Target::Journal(_)),
        );

        assert_eq!(opened, Some(Target::Journal("Dusk".into())));
        assert_eq!(
            fired,
            HashSet::from([
                "routine:morning@due".to_string(),
                "journal:Dusk@due".to_string()
            ])
        );
    }

    #[test]
    fn the_first_matching_prompt_in_order_wins() {
        let prompts = [
            routine("morning", Timing::new("09:00", "13:00", vec![])),
            journal("Dusk", Timing::new("09:00", "13:00", vec![])),
        ];
        let mut fired = HashSet::new();

        assert_eq!(
            open(&prompts, Reason::Due, at(10, 0), &mut fired, true),
            Some(Target::Routine("morning".into()))
        );
        assert_eq!(fired, HashSet::from(["routine:morning@due".to_string()]));
    }

    #[test]
    fn a_prompt_that_fired_already_is_skipped_for_the_next_one() {
        let prompts = [
            routine("morning", Timing::new("09:00", "13:00", vec![])),
            journal("Dusk", Timing::new("09:00", "13:00", vec![])),
        ];
        let mut fired = HashSet::from(["routine:morning@due".to_string()]);

        assert_eq!(
            open(&prompts, Reason::Due, at(10, 0), &mut fired, true),
            Some(Target::Journal("Dusk".into()))
        );
    }

    #[test]
    fn waiting_events_come_out_in_order() {
        let mut waiting = Waiting::default();
        waiting.push(OpenEvent::Start);
        waiting.push(OpenEvent::Context);

        assert_eq!(waiting.next(false, false), Some(OpenEvent::Start));
        assert_eq!(waiting.next(false, false), Some(OpenEvent::Context));
        assert_eq!(waiting.next(false, false), None);
    }

    #[test]
    fn pushing_the_same_event_twice_keeps_one() {
        let mut waiting = Waiting::default();
        waiting.push(OpenEvent::Start);
        waiting.push(OpenEvent::Start);

        assert_eq!(waiting.next(false, false), Some(OpenEvent::Start));
        assert_eq!(waiting.next(false, false), None);
    }

    #[test]
    fn waiting_events_are_kept_while_busy() {
        let mut waiting = Waiting::default();
        waiting.push(OpenEvent::Start);

        assert_eq!(waiting.next(false, true), None);
        assert_eq!(waiting.next(false, false), Some(OpenEvent::Start));
    }

    #[test]
    fn waiting_events_are_dropped_while_away() {
        let mut waiting = Waiting::default();
        waiting.push(OpenEvent::Start);

        assert_eq!(waiting.next(true, false), None);
        assert_eq!(waiting.next(false, false), None);
    }
}
