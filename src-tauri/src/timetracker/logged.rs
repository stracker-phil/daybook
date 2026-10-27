//! TimeTracker writes in the action log: each data-changing run of the tool is one line, a debug
//! build's dry run a skipped one.

use super::client::{RawReply, RunFailure, reply_result};
use super::reply::parse_recorded;
use crate::action_log::{self, ActionState, ActionTarget};

const DRY_RUN: &str = "Dry run: a debug build doesn't change TimeTracker's data";

/// The value of `--name=value` or `--name value` in `args`.
fn option<'a>(args: &[&'a str], name: &str) -> Option<&'a str> {
    let flag = format!("--{name}");
    let attached = format!("{flag}=");
    args.iter().enumerate().find_map(|(index, arg)| {
        arg.strip_prefix(attached.as_str()).or_else(|| {
            (*arg == flag)
                .then(|| args.get(index + 1).copied())
                .flatten()
        })
    })
}

/// `2026-10-27T10:00:00-2026-10-27T10:30:00` as `10:00–10:30`.
fn span_times(span: &str) -> Option<String> {
    let (start, end) = span.split_at_checked(19)?;
    let end = end.strip_prefix('-')?;
    Some(format!("{}–{}", start.get(11..16)?, end.get(11..16)?))
}

fn joined(parts: &[Option<String>]) -> String {
    parts
        .iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
        .join(" · ")
}

/// "Apollo 10:00–10:30": the suggestion's project and span, as far as given.
fn suggestion_head(args: &[&str], project: impl Fn(&str) -> String) -> String {
    [
        option(args, "project").map(project),
        option(args, "span").and_then(span_times),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ")
}

/// The action log's text for a write: "Timer started: Apollo · Review · #deep", "Timer stopped",
/// "Suggestion: Apollo 10:00–10:30 · Zoom - Weekly"; others as the command line. `name_of` names
/// TimeTracker's project ids.
pub fn describe_write(args: &[&str], name_of: impl Fn(&str) -> Option<String>) -> String {
    let project = |id: &str| name_of(id).unwrap_or_else(|| id.to_string());
    let note = || {
        option(args, "note")
            .filter(|note| !note.trim().is_empty())
            .map(String::from)
    };
    match args {
        ["timer", "start", id, ..] => format!(
            "Timer started: {}",
            joined(&[
                Some(project(id)),
                note(),
                option(args, "tag").map(|tag| format!("#{tag}")),
            ])
        ),
        ["timer", "stop", ..] => "Timer stopped".into(),
        ["record", ..] => format!(
            "Suggestion: {}",
            joined(&[Some(suggestion_head(args, project)), note()])
        ),
        _ => format!("timetracker {}", args.join(" ")),
    }
}

/// The line's state and detail after a run: a recorded suggestion names its id.
pub fn outcome(
    args: &[&str],
    reply: &Result<RawReply, RunFailure>,
) -> (ActionState, Option<String>) {
    match reply_result(reply) {
        Ok(stdout) => {
            let id = (args.first() == Some(&"record"))
                .then(|| parse_recorded(&stdout).ok().flatten())
                .flatten();
            (ActionState::Done, id.map(|id| format!("Suggestion {id}")))
        }
        Err(e) => (ActionState::Failed, Some(e.to_string())),
    }
}

/// Logs a write that ran (`reply`) or, in a dry run, was skipped.
pub fn log_write(
    args: &[&str],
    name_of: impl Fn(&str) -> Option<String>,
    reply: Option<&Result<RawReply, RunFailure>>,
) {
    let text = describe_write(args, name_of);
    let (state, detail) = match reply {
        Some(reply) => outcome(args, reply),
        None => (ActionState::Skipped, Some(DRY_RUN.into())),
    };
    action_log::log(ActionTarget::Timetracker, &text, state, detail);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(id: &str) -> Option<String> {
        (id == "p1").then(|| "Apollo".to_string())
    }

    fn ok(stdout: &str) -> Result<RawReply, RunFailure> {
        Ok(RawReply {
            code: Some(0),
            stdout: stdout.into(),
            stderr: String::new(),
        })
    }

    #[test]
    fn a_started_timer_names_its_project_note_and_tag() {
        assert_eq!(
            describe_write(
                &["timer", "start", "p1", "--note", "Review", "--tag=deep"],
                names
            ),
            "Timer started: Apollo · Review · #deep"
        );
    }

    #[test]
    fn a_started_timer_leaves_out_what_it_was_not_given() {
        for (args, expected) in [
            (vec!["timer", "start", "p1"], "Timer started: Apollo"),
            (
                vec!["timer", "start", "p1", "--note", "  "],
                "Timer started: Apollo",
            ),
            (
                vec!["timer", "start", "p1", "--note=Review"],
                "Timer started: Apollo · Review",
            ),
            (
                vec!["timer", "start", "p1", "--tag", "deep"],
                "Timer started: Apollo · #deep",
            ),
        ] {
            assert_eq!(describe_write(&args, names), expected, "{args:?}");
        }
    }

    #[test]
    fn a_project_without_a_name_falls_back_to_its_id() {
        assert_eq!(
            describe_write(&["timer", "start", "p9", "--note", "x"], names),
            "Timer started: p9 · x"
        );
        assert_eq!(
            describe_write(
                &[
                    "record",
                    "--project",
                    "p9",
                    "--span",
                    "2026-10-27T10:00:00-2026-10-27T10:30:00"
                ],
                names
            ),
            "Suggestion: p9 10:00–10:30"
        );
    }

    #[test]
    fn a_stopped_timer_is_described_plainly() {
        assert_eq!(describe_write(&["timer", "stop"], names), "Timer stopped");
    }

    #[test]
    fn a_suggestion_names_its_project_span_and_note() {
        assert_eq!(
            describe_write(
                &[
                    "record",
                    "--project=p1",
                    "--span",
                    "2026-10-27T10:00:00-2026-10-27T10:30:00",
                    "--note",
                    "Zoom - Weekly",
                ],
                names
            ),
            "Suggestion: Apollo 10:00–10:30 · Zoom - Weekly"
        );
    }

    #[test]
    fn a_suggestion_with_a_span_that_does_not_read_shows_the_project_only() {
        assert_eq!(
            describe_write(&["record", "--project", "p1", "--span", "later"], names),
            "Suggestion: Apollo"
        );
    }

    #[test]
    fn other_writes_are_described_as_the_command_line() {
        assert_eq!(
            describe_write(&["project", "rename", "p1", "New"], names),
            "timetracker project rename p1 New"
        );
    }

    #[test]
    fn a_run_that_worked_is_done() {
        assert_eq!(
            outcome(&["timer", "stop"], &ok("")),
            (ActionState::Done, None)
        );
    }

    #[test]
    fn a_recorded_suggestion_names_its_id() {
        assert_eq!(
            outcome(&["record", "--project", "p1"], &ok(r#"{"id":"s-42"}"#)),
            (ActionState::Done, Some("Suggestion s-42".into()))
        );
    }

    #[test]
    fn a_record_without_a_readable_reply_is_done_without_a_detail() {
        assert_eq!(outcome(&["record"], &ok("")), (ActionState::Done, None));
    }

    #[test]
    fn a_run_that_failed_carries_the_error() {
        let refused: Result<RawReply, RunFailure> = Ok(RawReply {
            code: Some(1),
            stdout: r#"{"message":"No such project"}"#.into(),
            stderr: String::new(),
        });
        assert_eq!(
            outcome(&["timer", "start", "p9"], &refused),
            (ActionState::Failed, Some("No such project".into()))
        );
        let missing = outcome(&["timer", "stop"], &Err(RunFailure::Missing));
        assert_eq!(missing.0, ActionState::Failed);
        assert!(missing.1.is_some());
    }
}
