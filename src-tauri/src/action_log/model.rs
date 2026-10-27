//! The action log's events as `action-log.json` keeps them, oldest first, and the rules for adding to
//! them: lines join their trigger's event, a line with the same target and text is updated instead of
//! added twice, a merged trigger repeats the last event of its title, old and empty events go. Pure.

use chrono::{DateTime, Duration, Local};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use ts_rs::TS;

/// Days an event is kept.
pub const KEEP_DAYS: i64 = 14;
pub const MAX_EVENTS: usize = 2000;
/// Lines this soon after their trigger's start still belong to it (a Slack retry comes after 30 s).
pub const TRIGGER_OPEN: Duration = Duration::minutes(2);
/// Repeats of a merged trigger (an autosave) within this time become one event.
pub const MERGE_WITHIN: Duration = Duration::minutes(15);
/// The title of a line logged outside any trigger without a title of its own.
pub const OTHER: &str = "Other";

/// Where an action went.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum ActionTarget {
    Obsidian,
    SlackStatus,
    SlackMessage,
    Timetracker,
    Daybook,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum ActionState {
    Sending,
    Done,
    /// Kept until today's note exists or stops changing.
    Waiting,
    /// Not sent.
    Skipped,
    Failed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Line {
    pub target: ActionTarget,
    pub text: String,
    pub state: ActionState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// The last state change.
    pub at: DateTime<Local>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub id: String,
    /// The trigger's start; for merged repeats, the latest.
    pub at: DateTime<Local>,
    pub title: String,
    pub lines: Vec<Line>,
    /// 1 when never merged.
    #[serde(default = "one")]
    pub count: u32,
    /// Shown even without lines (a context switch).
    #[serde(default)]
    pub always: bool,
}

fn one() -> u32 {
    1
}

impl Event {
    pub fn shown(&self) -> bool {
        !self.lines.is_empty() || self.always
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LineRef {
    pub event: String,
    pub index: usize,
}

/// The events, oldest first.
#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(transparent)]
pub struct Log {
    pub events: Vec<Event>,
}

/// Events that don't read as one (an unknown target or state) are dropped on load.
impl<'de> Deserialize<'de> for Log {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let Value::Array(raw) = Value::deserialize(deserializer)? else {
            return Ok(Self::default());
        };
        let events = raw
            .into_iter()
            .filter_map(|event| serde_json::from_value(event).ok())
            .collect();
        Ok(Self { events })
    }
}

impl Log {
    fn event_mut(&mut self, id: &str) -> Option<&mut Event> {
        self.events.iter_mut().rev().find(|event| event.id == id)
    }

    /// Whether event `id` is there to edit.
    fn edit_event(&mut self, id: &str, edit: impl FnOnce(&mut Event)) -> bool {
        let Some(event) = self.event_mut(id) else {
            return false;
        };
        edit(event);
        true
    }

    pub fn clear(&mut self) {
        self.events.clear();
    }

    /// Starts an event titled `title`, or with `merge` counts one more repeat of the last event when it
    /// has the same title and is less than `MERGE_WITHIN` old. Returns its id.
    pub fn open(&mut self, title: &str, merge: bool, now: DateTime<Local>) -> String {
        if merge
            && let Some(last) = self.events.last_mut()
            && last.title == title
            && now - last.at < MERGE_WITHIN
        {
            last.at = now;
            last.count += 1;
            return last.id.clone();
        }
        let id = uuid::Uuid::new_v4().to_string();
        self.events.push(Event {
            id: id.clone(),
            at: now,
            title: title.to_string(),
            lines: Vec::new(),
            count: 1,
            always: false,
        });
        id
    }

    /// Adds a line to event `id`, or updates its line with the same target and text. `None` when the
    /// event is gone.
    pub fn add(
        &mut self,
        id: &str,
        target: ActionTarget,
        text: &str,
        state: ActionState,
        detail: Option<String>,
        now: DateTime<Local>,
    ) -> Option<LineRef> {
        let event = self.event_mut(id)?;
        let index = upsert_line(event, target, text, state, detail, now);
        Some(LineRef {
            event: id.to_string(),
            index,
        })
    }

    fn line_mut(&mut self, line: &LineRef) -> Option<&mut Line> {
        self.event_mut(&line.event)?.lines.get_mut(line.index)
    }

    /// Whether the line is still there.
    pub fn update(
        &mut self,
        line: &LineRef,
        state: ActionState,
        detail: Option<String>,
        now: DateTime<Local>,
    ) -> bool {
        let Some(line) = self.line_mut(line) else {
            return false;
        };
        line.state = state;
        line.detail = detail;
        line.at = now;
        true
    }

    pub fn rename_event(&mut self, id: &str, title: &str) -> bool {
        self.edit_event(id, |event| event.title = title.to_string())
    }

    pub fn rename_line(&mut self, line: &LineRef, text: &str) -> bool {
        let Some(line) = self.line_mut(line) else {
            return false;
        };
        line.text = text.to_string();
        true
    }

    /// The event shows even without lines.
    pub fn keep(&mut self, id: &str) -> bool {
        self.edit_event(id, |event| event.always = true)
    }

    /// Drops events older than `KEEP_DAYS`, empty ones once their trigger closed, and the oldest
    /// beyond `MAX_EVENTS`.
    pub fn prune(&mut self, now: DateTime<Local>) {
        let cutoff = cutoff(now);
        let closed = now - TRIGGER_OPEN;
        self.events
            .retain(|event| event.at >= cutoff && (event.shown() || event.at >= closed));
        let newest = newest(&self.events).len();
        self.events.drain(..self.events.len() - newest);
    }

    /// The events to show and keep: within `KEEP_DAYS`, with lines or kept, at most `MAX_EVENTS`.
    pub fn visible(&self, now: DateTime<Local>) -> Vec<&Event> {
        let cutoff = cutoff(now);
        let shown: Vec<&Event> = self
            .events
            .iter()
            .filter(|event| event.at >= cutoff && event.shown())
            .collect();
        newest(&shown).to_vec()
    }

    /// What the file keeps.
    pub fn saved(&self, now: DateTime<Local>) -> Log {
        Log {
            events: self.visible(now).into_iter().cloned().collect(),
        }
    }
}

/// Events older than this go.
fn cutoff(now: DateTime<Local>) -> DateTime<Local> {
    now - Duration::days(KEEP_DAYS)
}

/// The last `MAX_EVENTS` of `events`.
fn newest<T>(events: &[T]) -> &[T] {
    &events[events.len().saturating_sub(MAX_EVENTS)..]
}

/// Updates the line with the same target and text, else appends one. Returns its index.
fn upsert_line(
    event: &mut Event,
    target: ActionTarget,
    text: &str,
    state: ActionState,
    detail: Option<String>,
    now: DateTime<Local>,
) -> usize {
    let existing = event
        .lines
        .iter()
        .position(|line| line.target == target && line.text == text);
    match existing {
        Some(index) => {
            let line = &mut event.lines[index];
            line.state = state;
            line.detail = detail;
            line.at = now;
            index
        }
        None => {
            event.lines.push(Line {
                target,
                text: text.to_string(),
                state,
                detail,
                at: now,
            });
            event.lines.len() - 1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use serde_json::json;

    fn at(hour: u32, minute: u32) -> DateTime<Local> {
        Local
            .with_ymd_and_hms(2026, 10, 27, hour, minute, 0)
            .single()
            .expect("a valid local time")
    }

    fn now() -> DateTime<Local> {
        at(12, 0)
    }

    fn line(text: &str) -> Line {
        Line {
            target: ActionTarget::Obsidian,
            text: text.to_string(),
            state: ActionState::Done,
            detail: None,
            at: now(),
        }
    }

    fn event(id: &str, at: DateTime<Local>, lines: usize) -> Event {
        Event {
            id: id.to_string(),
            at,
            title: format!("Event {id}"),
            lines: (0..lines).map(|i| line(&format!("line {i}"))).collect(),
            count: 1,
            always: false,
        }
    }

    fn log_of(events: Vec<Event>) -> Log {
        Log { events }
    }

    fn ids(log: &Log) -> Vec<&str> {
        log.events.iter().map(|event| event.id.as_str()).collect()
    }

    #[test]
    fn a_new_title_opens_an_empty_event_at_the_given_time() {
        let mut log = Log::default();
        let id = log.open("Focus saved", false, now());
        assert_eq!(
            log.events,
            vec![Event {
                id,
                at: now(),
                title: "Focus saved".into(),
                lines: Vec::new(),
                count: 1,
                always: false,
            }]
        );
    }

    #[test]
    fn a_merged_open_repeats_the_last_event_of_the_same_title() {
        let mut log = Log::default();
        let first = log.open("Quick note: Ideas", true, at(12, 0));
        let second = log.open("Quick note: Ideas", true, at(12, 14));
        let third = log.open("Quick note: Ideas", true, at(12, 28));
        assert_eq!(first, second);
        assert_eq!(first, third);
        assert_eq!(log.events.len(), 1);
        assert_eq!(log.events[0].count, 3);
        assert_eq!(log.events[0].at, at(12, 28));
    }

    #[test]
    fn an_open_starts_a_new_event_when_merging_does_not_apply() {
        let cases = [
            ("another title", "Quick note: Other", true, at(12, 5)),
            (
                "older than the merge window",
                "Quick note: Ideas",
                true,
                at(12, 16),
            ),
            (
                "merging not asked for",
                "Quick note: Ideas",
                false,
                at(12, 5),
            ),
        ];
        for (name, title, merge, when) in cases {
            let mut log = Log::default();
            let first = log.open("Quick note: Ideas", true, at(12, 0));
            let second = log.open(title, merge, when);
            assert_ne!(first, second, "{name}");
            assert_eq!(log.events.len(), 2, "{name}");
            assert_eq!(log.events[0].count, 1, "{name}");
            assert_eq!(log.events[0].at, at(12, 0), "{name}");
        }
    }

    #[test]
    fn a_merge_only_looks_at_the_last_event() {
        let mut log = Log::default();
        let first = log.open("Quick note: Ideas", true, at(12, 0));
        log.open("Focus saved", false, at(12, 1));
        let again = log.open("Quick note: Ideas", true, at(12, 2));
        assert_ne!(first, again);
        assert_eq!(log.events.len(), 3);
    }

    #[test]
    fn adding_a_line_appends_it_and_returns_where_it_is() {
        let mut log = Log::default();
        let id = log.open("Focus saved", false, now());
        let first = log.add(
            &id,
            ActionTarget::Obsidian,
            "Focus: 2 lines",
            ActionState::Done,
            None,
            now(),
        );
        let second = log.add(
            &id,
            ActionTarget::SlackMessage,
            "#team: focus list",
            ActionState::Sending,
            Some("queued".into()),
            at(12, 1),
        );
        assert_eq!(
            first,
            Some(LineRef {
                event: id.clone(),
                index: 0
            })
        );
        assert_eq!(
            second,
            Some(LineRef {
                event: id.clone(),
                index: 1
            })
        );
        assert_eq!(
            log.events[0].lines[1],
            Line {
                target: ActionTarget::SlackMessage,
                text: "#team: focus list".into(),
                state: ActionState::Sending,
                detail: Some("queued".into()),
                at: at(12, 1),
            }
        );
    }

    #[test]
    fn a_line_with_the_same_target_and_text_is_updated_instead_of_duplicated() {
        let mut log = Log::default();
        let id = log.open("Focus saved", false, now());
        log.add(
            &id,
            ActionTarget::Obsidian,
            "Focus: 2 lines",
            ActionState::Sending,
            Some("old".into()),
            now(),
        );
        let again = log.add(
            &id,
            ActionTarget::Obsidian,
            "Focus: 2 lines",
            ActionState::Failed,
            None,
            at(12, 1),
        );
        assert_eq!(again.map(|line| line.index), Some(0));
        assert_eq!(
            log.events[0].lines,
            vec![Line {
                target: ActionTarget::Obsidian,
                text: "Focus: 2 lines".into(),
                state: ActionState::Failed,
                detail: None,
                at: at(12, 1),
            }]
        );
    }

    #[test]
    fn the_same_text_for_another_target_is_a_second_line() {
        let mut log = Log::default();
        let id = log.open("Status set: Off", false, now());
        for target in [ActionTarget::SlackStatus, ActionTarget::Timetracker] {
            log.add(&id, target, "Off", ActionState::Done, None, now());
        }
        assert_eq!(log.events[0].lines.len(), 2);
    }

    #[test]
    fn adding_to_an_event_that_is_gone_returns_none() {
        let mut log = Log::default();
        let added = log.add(
            "missing",
            ActionTarget::Obsidian,
            "x",
            ActionState::Done,
            None,
            now(),
        );
        assert_eq!(added, None);
        assert!(log.events.is_empty());
    }

    #[test]
    fn an_update_changes_state_detail_and_time_of_the_line() {
        let mut log = Log::default();
        let id = log.open("Focus saved", false, now());
        let line = log
            .add(
                &id,
                ActionTarget::SlackStatus,
                "Off",
                ActionState::Sending,
                None,
                now(),
            )
            .unwrap();
        let found = log.update(
            &line,
            ActionState::Failed,
            Some("Not connected".into()),
            at(12, 3),
        );
        assert!(found);
        let updated = &log.events[0].lines[0];
        assert_eq!(updated.state, ActionState::Failed);
        assert_eq!(updated.detail.as_deref(), Some("Not connected"));
        assert_eq!(updated.at, at(12, 3));
    }

    #[test]
    fn an_update_of_a_missing_line_changes_nothing() {
        let mut log = Log::default();
        let id = log.open("Focus saved", false, now());
        let before = log.clone();
        for line in [
            LineRef {
                event: "missing".into(),
                index: 0,
            },
            LineRef {
                event: id,
                index: 4,
            },
        ] {
            assert!(
                !log.update(&line, ActionState::Done, None, at(12, 1)),
                "{line:?}"
            );
        }
        assert_eq!(log, before);
    }

    #[test]
    fn renaming_changes_the_title_of_the_event() {
        let mut log = Log::default();
        let id = log.open("Context change", false, now());
        assert!(log.rename_event(&id, "Context: Work → Home"));
        assert_eq!(log.events[0].title, "Context: Work → Home");
        assert!(!log.rename_event("missing", "x"));
    }

    #[test]
    fn renaming_a_line_changes_its_text_and_keeps_its_place() {
        let mut log = Log::default();
        let id = log.open("Context change", false, now());
        log.add(
            &id,
            ActionTarget::Daybook,
            "a",
            ActionState::Done,
            None,
            now(),
        );
        let line = log
            .add(
                &id,
                ActionTarget::Daybook,
                "b",
                ActionState::Done,
                None,
                now(),
            )
            .unwrap();
        assert!(log.rename_line(&line, "c"));
        let texts: Vec<&str> = log.events[0]
            .lines
            .iter()
            .map(|l| l.text.as_str())
            .collect();
        assert_eq!(texts, ["a", "c"]);
        let gone = LineRef {
            event: id,
            index: 9,
        };
        assert!(!log.rename_line(&gone, "d"));
    }

    #[test]
    fn keeping_an_event_makes_it_show_without_lines() {
        let mut log = Log::default();
        let id = log.open("Context change", false, now());
        assert!(log.visible(now()).is_empty());
        assert!(log.keep(&id));
        assert_eq!(log.visible(now()).len(), 1);
        assert!(!log.keep("missing"));
    }

    #[test]
    fn prune_drops_events_older_than_fourteen_days() {
        let mut log = log_of(vec![
            event("old", now() - Duration::days(15), 1),
            event("recent", now() - Duration::days(13), 1),
        ]);
        log.prune(now());
        assert_eq!(ids(&log), ["recent"]);
    }

    #[test]
    fn prune_drops_empty_events_once_their_trigger_closed() {
        let mut kept = event("kept", at(11, 0), 0);
        kept.always = true;
        let mut log = log_of(vec![
            event("closed", at(11, 57), 0),
            event("open", at(11, 59), 0),
            kept,
            event("with-lines", at(11, 0), 1),
        ]);
        log.prune(now());
        assert_eq!(ids(&log), ["open", "kept", "with-lines"]);
    }

    #[test]
    fn prune_keeps_the_newest_two_thousand_events() {
        let start = now() - Duration::days(2);
        let mut log = log_of(
            (0..MAX_EVENTS + 5)
                .map(|i| event(&format!("e{i}"), start + Duration::minutes(i as i64), 1))
                .collect(),
        );
        log.prune(now());
        assert_eq!(log.events.len(), MAX_EVENTS);
        assert_eq!(log.events.first().map(|e| e.id.as_str()), Some("e5"));
        assert_eq!(log.events.last().map(|e| e.id.as_str()), Some("e2004"));
    }

    #[test]
    fn visible_leaves_out_empty_unkept_and_old_events() {
        let mut kept = event("kept", at(11, 0), 0);
        kept.always = true;
        let log = log_of(vec![
            event("old", now() - Duration::days(15), 1),
            event("empty", at(11, 0), 0),
            kept,
            event("lines", at(11, 30), 2),
        ]);
        let visible: Vec<&str> = log.visible(now()).iter().map(|e| e.id.as_str()).collect();
        assert_eq!(visible, ["kept", "lines"]);
    }

    #[test]
    fn visible_and_saved_cap_the_log_at_the_newest_two_thousand() {
        let start = now() - Duration::days(2);
        let log = log_of(
            (0..MAX_EVENTS + 3)
                .map(|i| event(&format!("e{i}"), start + Duration::minutes(i as i64), 1))
                .collect(),
        );
        let visible = log.visible(now());
        assert_eq!(visible.len(), MAX_EVENTS);
        assert_eq!(visible[0].id, "e3");
        let saved = log.saved(now());
        assert_eq!(saved.events.len(), MAX_EVENTS);
        assert_eq!(saved.events[0].id, "e3");
    }

    #[test]
    fn saved_holds_only_what_is_visible() {
        let log = log_of(vec![
            event("empty", at(11, 0), 0),
            event("lines", at(11, 30), 1),
        ]);
        assert_eq!(
            log.saved(now()),
            log_of(vec![event("lines", at(11, 30), 1)])
        );
    }

    #[test]
    fn a_file_that_is_not_an_array_loads_as_an_empty_log() {
        for raw in [json!({"events": []}), json!("text"), json!(null), json!(3)] {
            let log: Log = serde_json::from_value(raw.clone()).unwrap();
            assert_eq!(log, Log::default(), "{raw}");
        }
    }

    #[test]
    fn events_with_an_unknown_target_or_state_are_dropped_on_load() {
        let stamp = now().to_rfc3339();
        let line = |target: &str, state: &str| json!({"target": target, "text": "x", "state": state, "at": stamp});
        let event = |id: &str, line: serde_json::Value| json!({"id": id, "at": stamp, "title": "T", "lines": [line]});
        let raw = json!([
            event("good", line("obsidian", "done")),
            event("bad-target", line("email", "done")),
            event("bad-state", line("obsidian", "pending")),
            "not an event",
            event("also-good", line("slackStatus", "waiting")),
        ]);
        let log: Log = serde_json::from_value(raw).unwrap();
        assert_eq!(ids(&log), ["good", "also-good"]);
        assert_eq!(log.events[0].count, 1);
        assert!(!log.events[0].always);
        assert_eq!(log.events[0].lines[0].detail, None);
    }

    #[test]
    fn the_log_serializes_as_an_array_with_camel_case_names() {
        let mut shown = event("a", now(), 0);
        shown.lines.push(Line {
            target: ActionTarget::SlackStatus,
            text: "Off".into(),
            state: ActionState::Skipped,
            detail: None,
            at: now(),
        });
        let value = serde_json::to_value(log_of(vec![shown])).unwrap();
        assert_eq!(value[0]["id"], "a");
        assert_eq!(value[0]["title"], "Event a");
        assert_eq!(value[0]["count"], 1);
        assert_eq!(value[0]["always"], false);
        assert_eq!(value[0]["lines"][0]["target"], "slackStatus");
        assert_eq!(value[0]["lines"][0]["state"], "skipped");
        assert!(value[0]["lines"][0].get("detail").is_none());
    }
}
