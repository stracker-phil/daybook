//! The reminder as the Today card's one line: a label, the text and what a click (or ↵) does. Pure.

use serde::Serialize;
use ts_rs::TS;

use crate::reminders::Reminder;
use crate::store::settings::TodoSpan;

/// What a click or ↵ on the line does: a bus command, or joining an event's call.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum ReminderAction {
    Command { label: String, command: String },
    Join { label: String, url: String },
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct ReminderLine {
    /// "Now", "Next", "Routine", "Journal", "Todos".
    pub label: String,
    pub text: String,
    pub action: Option<ReminderAction>,
}

fn span_text(span: TodoSpan) -> &'static str {
    match span {
        TodoSpan::Today => "from today",
        TodoSpan::Week => "from this week",
        TodoSpan::Month => "from this month",
        TodoSpan::All => "in your daily notes",
    }
}

fn command(label: &str, command: String) -> Option<ReminderAction> {
    Some(ReminderAction::Command {
        label: label.into(),
        command,
    })
}

pub fn reminder_line(reminder: Reminder) -> ReminderLine {
    let line = |label: &str, text: String, action| ReminderLine {
        label: label.into(),
        text,
        action,
    };
    match reminder {
        Reminder::Meeting {
            title,
            start,
            end,
            meeting,
            running,
        } => {
            let join = meeting.map(|link| ReminderAction::Join {
                label: "Join".into(),
                url: link.url,
            });
            if running {
                line(
                    "Now",
                    format!("{title} until {}", end.format("%H:%M")),
                    join,
                )
            } else {
                let text = format!("{title} starting at {}", start.format("%H:%M"));
                line("Next", text, join)
            }
        }
        Reminder::Routine {
            routine,
            name,
            open,
            started,
        } => {
            let label = if started { "Continue" } else { "Start" };
            let action = command(label, format!("routine:{routine}"));
            line("Routine", format!("{name} · {open} open"), action)
        }
        Reminder::Journal {
            group,
            open,
            questions,
        } => {
            let text = if questions {
                format!("{group} · {open} open")
            } else {
                format!("{group} · not written yet")
            };
            let action = command("Open", format!("journal:open:{group}"));
            line("Journal", text, action)
        }
        Reminder::Todos { count, span } => {
            let todos = if count == 1 { "todo" } else { "todos" };
            let text = format!("{count} open {todos} {} to review", span_text(span));
            line("Todos", text, None)
        }
    }
}

#[cfg(test)]
mod tests {
    use chrono::{NaiveDate, NaiveDateTime};

    use super::*;
    use crate::calendar::MeetingLink;
    use crate::calendar::events::CallApp;

    fn at(hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 24)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    fn meeting(running: bool, link: Option<&str>) -> Reminder {
        Reminder::Meeting {
            title: "Sprint Review".into(),
            start: at(15, 0),
            end: at(15, 30),
            meeting: link.map(|url| MeetingLink {
                app: CallApp::Zoom,
                url: url.into(),
            }),
            running,
        }
    }

    fn command_action(label: &str, command: &str) -> Option<ReminderAction> {
        Some(ReminderAction::Command {
            label: label.into(),
            command: command.into(),
        })
    }

    #[test]
    fn a_running_meeting_shows_until_when_it_ends() {
        let line = reminder_line(meeting(true, None));
        assert_eq!(line.label, "Now");
        assert_eq!(line.text, "Sprint Review until 15:30");
        assert_eq!(line.action, None);
    }

    #[test]
    fn the_next_meeting_shows_when_it_starts() {
        let line = reminder_line(meeting(false, None));
        assert_eq!(line.label, "Next");
        assert_eq!(line.text, "Sprint Review starting at 15:00");
        assert_eq!(line.action, None);
    }

    #[test]
    fn a_meeting_with_a_link_offers_to_join_it() {
        let expected = Some(ReminderAction::Join {
            label: "Join".into(),
            url: "https://zoom.us/j/123".into(),
        });
        for running in [true, false] {
            let line = reminder_line(meeting(running, Some("https://zoom.us/j/123")));
            assert_eq!(line.action, expected, "running: {running}");
        }
    }

    #[test]
    fn a_routine_is_started_until_something_is_done() {
        for (started, label) in [(false, "Start"), (true, "Continue")] {
            let line = reminder_line(Reminder::Routine {
                routine: "evening".into(),
                name: "Evening".into(),
                open: 2,
                started,
            });
            assert_eq!(line.label, "Routine");
            assert_eq!(line.text, "Evening · 2 open");
            assert_eq!(
                line.action,
                command_action(label, "routine:evening"),
                "started: {started}"
            );
        }
    }

    #[test]
    fn a_journal_group_with_questions_shows_its_open_questions() {
        let line = reminder_line(Reminder::Journal {
            group: "Dusk".into(),
            open: 2,
            questions: true,
        });
        assert_eq!(line.label, "Journal");
        assert_eq!(line.text, "Dusk · 2 open");
        assert_eq!(line.action, command_action("Open", "journal:open:Dusk"));
    }

    #[test]
    fn a_free_text_journal_group_says_it_is_not_written_yet() {
        let line = reminder_line(Reminder::Journal {
            group: "Health".into(),
            open: 1,
            questions: false,
        });
        assert_eq!(line.text, "Health · not written yet");
        assert_eq!(line.action, command_action("Open", "journal:open:Health"));
    }

    #[test]
    fn todos_name_their_count_and_span_and_have_no_action() {
        let cases = [
            (1, TodoSpan::Today, "1 open todo from today to review"),
            (3, TodoSpan::Week, "3 open todos from this week to review"),
            (2, TodoSpan::Month, "2 open todos from this month to review"),
            (
                5,
                TodoSpan::All,
                "5 open todos in your daily notes to review",
            ),
        ];
        for (count, span, text) in cases {
            let line = reminder_line(Reminder::Todos { count, span });
            assert_eq!(line.label, "Todos", "{text}");
            assert_eq!(line.text, text);
            assert_eq!(line.action, None, "{text}");
        }
    }

    #[test]
    fn actions_serialize_with_a_kind_tag() {
        let join = ReminderAction::Join {
            label: "Join".into(),
            url: "https://zoom.us/j/123".into(),
        };
        assert_eq!(
            serde_json::to_value(&join).unwrap(),
            serde_json::json!({ "kind": "join", "label": "Join", "url": "https://zoom.us/j/123" })
        );
        assert_eq!(
            serde_json::to_value(command_action("Open", "journal:open:Dusk")).unwrap(),
            serde_json::json!({ "kind": "command", "label": "Open", "command": "journal:open:Dusk" })
        );
    }
}
