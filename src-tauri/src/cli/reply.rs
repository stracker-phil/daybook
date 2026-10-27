//! What the command prints for the app's reply, one fact per line so scripts can grep it.
//! `--json` prints the reply itself instead.

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::automation::{Action, AutomationError, Outcome};
use crate::error::ErrorKind;
use crate::ipc::IpcMessage;
use crate::remote::StatusReport;

/// The running app's answer to the command's words: what to print where, and the exit code.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct CliReply {
    pub code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl CliReply {
    pub fn print(text: impl Into<String>) -> Self {
        Self {
            stdout: text.into(),
            ..Self::default()
        }
    }

    pub fn json(value: &Value) -> Self {
        Self::print(value.to_string())
    }

    /// Words the command can't use: exit code 2.
    pub fn usage(message: &str, json: bool) -> Self {
        Self {
            code: 2,
            ..Self::error(ErrorKind::InvalidInput, message, json)
        }
    }

    /// The app answered with an error: exit code 1.
    pub fn failed(error: &AutomationError, json: bool) -> Self {
        Self::error(error.kind, &error.message, json)
    }

    fn error(kind: ErrorKind, message: &str, json: bool) -> Self {
        let (stdout, stderr) = if json {
            (
                json!({ "kind": kind, "message": message }).to_string(),
                String::new(),
            )
        } else {
            (String::new(), message.to_string())
        };
        Self {
            code: 1,
            stdout,
            stderr,
        }
    }
}

/// What an action prints: `state` the state, an unchanged status says so, the rest nothing; with
/// `--json` the whole outcome (`state` only the state).
pub fn action_reply(action: Action, outcome: &Outcome, json: bool) -> CliReply {
    let state = outcome.get("state").cloned().unwrap_or_default();
    if action == Action::State {
        return CliReply::print(if json {
            state.to_string()
        } else {
            serde_json::to_string_pretty(&state).unwrap_or_default()
        });
    }
    if json {
        return CliReply::json(&Value::Object(outcome.clone()));
    }
    match (
        outcome.get("result").and_then(Value::as_str),
        state.get("status"),
    ) {
        (Some("unchanged"), Some(Value::String(status))) => {
            CliReply::print(format!("The status already is {status}."))
        }
        _ => CliReply::default(),
    }
}

/// The text for `result`, the reply to `message`; `None` when the subcommand prints nothing.
pub fn render(message: &IpcMessage, result: &Value) -> Result<Option<String>, String> {
    Ok(match message {
        IpcMessage::Status => Some(report_text(&read::<StatusReport>(result)?)),
        _ => None,
    })
}

fn read<T: DeserializeOwned>(value: &Value) -> Result<T, String> {
    T::deserialize(value).map_err(|e| format!("The running Daybook sent an unexpected reply: {e}"))
}

pub fn report_text(report: &StatusReport) -> String {
    let window = if report.window_visible {
        "visible"
    } else {
        "hidden"
    };
    format!("Daybook {}\nwindow: {window}", report.version)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn outcome(value: Value) -> Outcome {
        value.as_object().cloned().unwrap()
    }

    fn stdout_json(reply: &CliReply) -> Value {
        serde_json::from_str(&reply.stdout).unwrap()
    }

    #[test]
    fn state_prints_the_state_as_pretty_json() {
        let outcome = outcome(json!({ "state": { "status": "break" } }));
        assert_eq!(
            action_reply(Action::State, &outcome, false),
            CliReply::print("{\n  \"status\": \"break\"\n}")
        );
    }

    #[test]
    fn state_prints_compact_json_with_the_json_flag() {
        let outcome = outcome(json!({ "state": { "status": "break" } }));
        assert_eq!(
            action_reply(Action::State, &outcome, true),
            CliReply::print(r#"{"status":"break"}"#)
        );
    }

    #[test]
    fn an_unchanged_status_says_so() {
        let outcome = outcome(json!({ "result": "unchanged", "state": { "status": "break" } }));
        assert_eq!(
            action_reply(Action::Status, &outcome, false),
            CliReply::print("The status already is break.")
        );
    }

    #[test]
    fn other_results_print_nothing() {
        for (action, value) in [
            (
                Action::Status,
                json!({ "result": "set", "state": { "status": "break" } }),
            ),
            (
                Action::Status,
                json!({ "result": "unchanged", "state": { "status": null } }),
            ),
            (Action::Slack, json!({ "result": "set", "state": {} })),
            (Action::Command, json!({ "state": {} })),
            (Action::Show, json!({})),
        ] {
            assert_eq!(
                action_reply(action, &outcome(value.clone()), false),
                CliReply::default(),
                "{value}"
            );
        }
    }

    #[test]
    fn the_json_flag_prints_the_whole_outcome() {
        let value = json!({ "result": "unchanged", "state": { "status": "break" } });
        let reply = action_reply(Action::Status, &outcome(value.clone()), true);
        assert_eq!(reply.code, 0);
        assert_eq!(stdout_json(&reply), value);
        assert_eq!(reply.stderr, "");
    }

    #[test]
    fn usage_exits_with_two_and_the_message_on_stderr() {
        assert_eq!(
            CliReply::usage("daybook show takes no words.", false),
            CliReply {
                code: 2,
                stdout: String::new(),
                stderr: "daybook show takes no words.".into()
            }
        );
    }

    #[test]
    fn usage_with_json_puts_an_invalid_input_error_on_stdout() {
        let reply = CliReply::usage("0 to 100", true);
        assert_eq!(reply.code, 2);
        assert_eq!(reply.stderr, "");
        assert_eq!(
            stdout_json(&reply),
            json!({ "kind": "invalid_input", "message": "0 to 100" })
        );
    }

    #[test]
    fn a_failed_action_exits_with_one() {
        let error = AutomationError::new(404, "Unknown command “x”.");
        assert_eq!(
            CliReply::failed(&error, false),
            CliReply {
                code: 1,
                stdout: String::new(),
                stderr: "Unknown command “x”.".into()
            }
        );
        let reply = CliReply::failed(&error, true);
        assert_eq!(reply.code, 1);
        assert_eq!(
            stdout_json(&reply),
            json!({ "kind": "not_found", "message": "Unknown command “x”." })
        );
    }

    #[test]
    fn print_and_json_replies_exit_with_zero() {
        assert_eq!(
            CliReply::print("hi"),
            CliReply {
                code: 0,
                stdout: "hi".into(),
                stderr: String::new()
            }
        );
        assert_eq!(CliReply::json(&json!([1])).stdout, "[1]");
    }

    #[test]
    fn status_prints_the_version_and_the_window() {
        for (visible, window) in [(true, "visible"), (false, "hidden")] {
            let result = json!({ "version": "1.2.3", "windowVisible": visible });
            assert_eq!(
                render(&IpcMessage::Status, &result),
                Ok(Some(format!("Daybook 1.2.3\nwindow: {window}")))
            );
        }
    }

    #[test]
    fn commands_without_output_print_nothing() {
        for message in [
            IpcMessage::Hide,
            IpcMessage::Quit,
            IpcMessage::Show { focus: false },
        ] {
            assert_eq!(render(&message, &Value::Null), Ok(None), "{message:?}");
        }
    }

    #[test]
    fn a_status_reply_of_the_wrong_shape_is_an_error() {
        let err = render(&IpcMessage::Status, &json!({ "version": 3 })).unwrap_err();
        assert!(
            err.starts_with("The running Daybook sent an unexpected reply"),
            "{err}"
        );
    }
}
