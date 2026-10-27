//! Requests from the CLI: `handle` maps each `IpcMessage` to the module that does it; the command's
//! words go to `automation::cli`. Requests only the page can answer go out as `REMOTE_EVENT` with an
//! id and come back through `remote_reply`.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use ts_rs::TS;

use crate::error::{AppError, ErrorKind};
use crate::events::REMOTE_EVENT;
use crate::ipc::{HandlerResult, IpcError, IpcMessage};
use crate::launch;
use crate::pending::{NoReply, PendingReplies};
use crate::window;

/// Below the IPC client's timeout, so the sender still reads the reply.
const REPLY_TIMEOUT: Duration = Duration::from_secs(4);
/// Lets the reply to `quit` reach the sender before the app exits.
const QUIT_DELAY: Duration = Duration::from_millis(200);

/// What the page is asked to do on behalf of another process.
#[derive(Serialize, Debug, Clone, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RemoteRequest {}

#[derive(Serialize, Debug, Clone, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RemoteEvent {
    pub id: String,
    pub request: RemoteRequest,
}

/// The reply to `status`.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct StatusReport {
    pub version: String,
    pub window_visible: bool,
}

#[derive(Default)]
pub struct Bridge {
    pending: PendingReplies<HandlerResult>,
}

impl Bridge {
    /// Asks the page and waits for its answer, at most `REPLY_TIMEOUT`.
    pub async fn ask(&self, handle: &AppHandle, request: RemoteRequest) -> HandlerResult {
        let sent = self.pending.ask(REPLY_TIMEOUT, |id| {
            let event = RemoteEvent {
                id: id.to_string(),
                request,
            };
            handle.emit(REMOTE_EVENT, event)
        });
        let failed = |message: String| Err(IpcError::new(ErrorKind::Io, message));
        match sent.await {
            Ok(result) => result,
            Err(NoReply::NotSent(e)) => failed(format!("Couldn't reach the Daybook window: {e}")),
            Err(NoReply::Dropped) => failed("The Daybook window dropped the request".into()),
            Err(NoReply::TimedOut) => failed("The Daybook window did not answer in time".into()),
        }
    }

    /// False when no request with `id` waits (answered, timed out or unknown).
    pub fn answer(&self, id: &str, result: HandlerResult) -> bool {
        self.pending.answer(id, result)
    }
}

/// The page's answer to a `REMOTE_EVENT`: `error` set means it failed.
#[tauri::command]
pub fn remote_reply(
    bridge: State<'_, Bridge>,
    id: String,
    result: Option<serde_json::Value>,
    error: Option<String>,
) {
    let outcome = match error {
        Some(error) => Err(IpcError::new(ErrorKind::Io, error)),
        None => Ok(result.unwrap_or_default()),
    };
    if !bridge.answer(&id, outcome) {
        log::debug!("Late or unknown remote reply {id}");
    }
}

fn json(value: impl Serialize) -> HandlerResult {
    serde_json::to_value(value).map_err(|e| AppError::from(e).into())
}

pub fn status_report(app: &AppHandle) -> StatusReport {
    StatusReport {
        version: app.package_info().version.to_string(),
        window_visible: window::main_visible(app),
    }
}

pub fn quit_after_reply(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(QUIT_DELAY).await;
        app.exit(0);
    });
}

/// Handles one message from another process and returns what its sender prints.
pub async fn handle(app: AppHandle, message: IpcMessage) -> HandlerResult {
    log::info!("Received {} from another process", message.kind());
    match message {
        IpcMessage::Show { focus } => {
            if focus {
                window::show_main(&app);
            } else {
                window::show_main_in_background(&app);
            }
            Ok(serde_json::Value::Null)
        }
        IpcMessage::Hide => {
            window::hide_main(&app);
            Ok(serde_json::Value::Null)
        }
        IpcMessage::Quit => {
            quit_after_reply(app);
            Ok(serde_json::Value::Null)
        }
        IpcMessage::Status => json(status_report(&app)),
        IpcMessage::Cli {
            words,
            json: as_json,
        } => json(crate::automation::cli::answer(&app, &words, as_json).await),
        launch @ (IpcMessage::Activate { .. } | IpcMessage::Error { .. }) => {
            launch::queue(&app, launch);
            Ok(serde_json::Value::Null)
        }
        #[cfg(debug_assertions)]
        IpcMessage::Eval { js } => {
            crate::dev::eval(&app, &js);
            Ok(serde_json::Value::Null)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_report_round_trips_with_camel_case_keys() {
        let report = StatusReport {
            version: "1.2.3".into(),
            window_visible: true,
        };
        let value = serde_json::to_value(&report).unwrap();
        assert_eq!(
            value,
            serde_json::json!({ "version": "1.2.3", "windowVisible": true })
        );
        assert_eq!(
            serde_json::from_value::<StatusReport>(value).unwrap(),
            report
        );
    }
}
