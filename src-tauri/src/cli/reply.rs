//! What a subcommand prints for the app's reply, one fact per line so scripts can grep it.
//! `--json` prints the reply itself instead.

use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::ipc::IpcMessage;
use crate::remote::StatusReport;

/// The text for `result`, the reply to `message`; `None` when the subcommand prints nothing.
pub fn render(message: &IpcMessage, result: &Value) -> Result<Option<String>, String> {
    Ok(match message {
        IpcMessage::Status => Some(status(&read::<StatusReport>(result)?)),
        _ => None,
    })
}

fn read<T: DeserializeOwned>(value: &Value) -> Result<T, String> {
    T::deserialize(value).map_err(|e| format!("The running Daybook sent an unexpected reply: {e}"))
}

fn status(report: &StatusReport) -> String {
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
