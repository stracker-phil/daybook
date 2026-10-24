//! Journal groups, routines and the reminders the Daybook window shows, with when each is due.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::read::*;

/// A daily window from `from` to `to` (`HH:MM`; empty: the end of the day; before `from`: past
/// midnight).
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct TimeWindow {
    pub from: String,
    pub to: String,
}

impl TimeWindow {
    pub fn new(from: &str, to: &str) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
        }
    }

    fn read(source: &Obj) -> Option<Self> {
        let from = parse_clock(&text(source.get("from")))?;
        let to = parse_clock(&text(source.get("to")));
        Some(Self {
            from: format_clock(from),
            to: to.map(format_clock).unwrap_or_default(),
        })
    }
}

/// Events that open a routine's checklist or a journal group by themselves.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "kebab-case")]
pub enum OpenEvent {
    /// The day's first Working.
    Start,
    /// A context switch, once a day.
    Context,
    MeetingStart,
    MeetingEnd,
    /// Done for today.
    Off,
}

/// When a routine or journal group is due, and when it opens by itself.
#[derive(Serialize, Clone, Debug, PartialEq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Timing {
    /// Reminded in the Daybook window while due and not done; `None`: never due.
    pub due: Option<TimeWindow>,
    /// Opens once a day when the due time starts (or later within it).
    pub open_when_due: bool,
    pub open_on: Vec<OpenEvent>,
}

impl Timing {
    pub fn new(from: &str, to: &str, open_on: Vec<OpenEvent>) -> Self {
        Self {
            due: Some(TimeWindow::new(from, to)),
            open_when_due: true,
            open_on,
        }
    }

    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        let due = obj(field(source, "due")).and_then(TimeWindow::read);
        let mut open_on: Vec<OpenEvent> = Vec::new();
        for event in strings(field(source, "openOn")) {
            if let Some(event) = one_of(Some(&Value::String(event)))
                && !open_on.contains(&event)
            {
                open_on.push(event);
            }
        }
        Self {
            open_when_due: due.is_some() && is_true(field(source, "openWhenDue")),
            due,
            open_on,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum AnswerKind {
    /// Any number of numbered answers.
    List,
    /// Optional tasks.
    Task,
    Text,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct JournalQuestion {
    pub question: String,
    pub hint: String,
    pub kind: AnswerKind,
}

impl JournalQuestion {
    pub fn new(question: &str, hint: &str, kind: AnswerKind) -> Self {
        Self {
            question: question.into(),
            hint: hint.into(),
            kind,
        }
    }

    fn read(raw: &Value) -> Option<Self> {
        let source = raw.as_object()?;
        let question = trimmed(source.get("question"));
        if question.is_empty() {
            return None;
        }
        Some(Self {
            question,
            hint: trimmed(source.get("hint")),
            kind: one_of(source.get("kind")).unwrap_or(AnswerKind::Text),
        })
    }
}

/// One callout in the journal section; a group without questions is free text.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct JournalGroup {
    pub title: String,
    /// Callout type, e.g. NOTE or HEART.
    pub callout: String,
    pub folded: bool,
    pub questions: Vec<JournalQuestion>,
    pub timing: Timing,
}

impl JournalGroup {
    pub fn read(raw: &Value) -> Option<Self> {
        let source = raw.as_object()?;
        let title = trimmed(source.get("title"));
        if title.is_empty() {
            return None;
        }
        let callout = str(source.get("callout")).unwrap_or_else(|| "NOTE".into());
        let callout = callout.trim();
        let callout = callout.strip_prefix("[!").unwrap_or(callout);
        let callout = callout.strip_suffix(']').unwrap_or(callout).to_uppercase();
        Some(Self {
            title,
            callout: if is_word(&callout) {
                callout
            } else {
                "NOTE".into()
            },
            folded: is_true(source.get("folded")),
            questions: items(source.get("questions"))
                .iter()
                .filter_map(JournalQuestion::read)
                .collect(),
            timing: Timing::read(source.get("timing")),
        })
    }
}

/// One item of a routine's checklist.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RoutineStep {
    pub title: String,
    /// A URL or vault path opened from the item.
    pub link: String,
    /// Weekdays the item is due (0 = Sunday); empty: every day.
    pub days: Vec<u8>,
}

impl RoutineStep {
    pub fn new(title: &str, link: &str) -> Self {
        Self {
            title: title.into(),
            link: link.into(),
            days: vec![],
        }
    }

    fn read(raw: &Value) -> Option<Self> {
        let source = raw.as_object()?;
        let title = trimmed(source.get("title"));
        if title.is_empty() {
            return None;
        }
        Some(Self {
            title,
            link: trimmed(source.get("link")),
            days: weekdays(source.get("days")),
        })
    }
}

/// A named checklist, opened by its timing or by hand.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct Routine {
    pub id: String,
    pub name: String,
    /// Context ids it runs in; empty: every context with the routines feature.
    pub contexts: Vec<String>,
    pub steps: Vec<RoutineStep>,
    pub timing: Timing,
}

impl Routine {
    /// The items due on `weekday` (0 = Sunday), with their position in the routine.
    pub fn steps_on(&self, weekday: u8) -> Vec<(usize, &RoutineStep)> {
        self.steps
            .iter()
            .enumerate()
            .filter(|(_, step)| step.days.is_empty() || step.days.contains(&weekday))
            .collect()
    }

    /// Routines need a name; ids are unique.
    pub fn read_all(raw: &[Value]) -> Vec<Self> {
        let mut seen = HashSet::new();
        raw.iter()
            .filter_map(|item| {
                let source = item.as_object()?;
                let name = trimmed(source.get("name"));
                if name.is_empty() {
                    return None;
                }
                Some(Self {
                    id: unique_id(source, &name, &mut seen, "routine"),
                    name,
                    contexts: strings(source.get("contexts")),
                    steps: items(source.get("steps"))
                        .iter()
                        .filter_map(RoutineStep::read)
                        .collect(),
                    timing: Timing::read(source.get("timing")),
                })
            })
            .collect()
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum TodoSpan {
    Today,
    Week,
    Month,
    All,
}

/// What the Daybook window reminds of; the first enabled rule in the list that applies wins.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ReminderRule {
    /// The next meeting from `minutes_before` until it starts, and a running meeting until it ends.
    Meeting {
        enabled: bool,
        #[serde(rename = "minutesBefore")]
        minutes_before: u32,
    },
    /// While the routine (by id) is due.
    Routine { enabled: bool, routine: String },
    /// While the journal group (by title) is due.
    Journal { enabled: bool, group: String },
    Todos {
        enabled: bool,
        span: TodoSpan,
        from: String,
        to: String,
    },
}

impl ReminderRule {
    pub fn read(raw: &Value) -> Option<Self> {
        let source = raw.as_object()?;
        let enabled = on_unless_false(source.get("enabled"));
        let kind = source.get("kind").and_then(Value::as_str);
        let routine = trimmed(source.get("routine"));
        let group = trimmed(source.get("group"));
        match kind {
            Some("meeting") => Some(Self::Meeting {
                enabled,
                minutes_before: within(coerced_number(source.get("minutesBefore")), 0.0, 240.0)
                    .map_or(60, |minutes| round(minutes) as u32),
            }),
            Some("routine") if !routine.is_empty() => Some(Self::Routine { enabled, routine }),
            Some("journal") if !group.is_empty() => Some(Self::Journal { enabled, group }),
            Some("todos") => {
                let window = TimeWindow::read(source)?;
                Some(Self::Todos {
                    enabled,
                    span: one_of(source.get("span")).unwrap_or(TodoSpan::Today),
                    from: window.from,
                    to: window.to,
                })
            }
            _ => None,
        }
    }

    /// The routine or journal group it belongs to, as `routine:<id>` or `journal:<title>`.
    fn item(&self) -> Option<String> {
        match self {
            Self::Routine { routine, .. } => Some(routine_key(routine)),
            Self::Journal { group, .. } => Some(journal_key(group)),
            _ => None,
        }
    }
}

/// Every routine and journal group has exactly one reminder rule: new ones are added at the end,
/// rules of removed ones are dropped, so the list order sets their place among the reminders.
pub fn with_item_reminders(
    rules: Vec<ReminderRule>,
    routines: &[Routine],
    journal: &[JournalGroup],
) -> Vec<ReminderRule> {
    let routine_items = routines.iter().map(|r| routine_key(&r.id));
    let journal_items = journal.iter().map(|g| journal_key(&g.title));
    let known: HashSet<String> = routine_items.chain(journal_items).collect();
    let mut seen = HashSet::new();
    let mut kept: Vec<ReminderRule> = rules
        .into_iter()
        .filter(|rule| match rule.item() {
            None => true,
            Some(item) => known.contains(&item) && seen.insert(item),
        })
        .collect();
    for routine in routines {
        if !seen.contains(&routine_key(&routine.id)) {
            kept.push(ReminderRule::Routine {
                enabled: true,
                routine: routine.id.clone(),
            });
        }
    }
    for group in journal {
        if !seen.contains(&journal_key(&group.title)) {
            kept.push(ReminderRule::Journal {
                enabled: true,
                group: group.title.clone(),
            });
        }
    }
    kept
}

fn routine_key(id: &str) -> String {
    format!("routine:{id}")
}

fn journal_key(title: &str) -> String {
    format!("journal:{title}")
}
