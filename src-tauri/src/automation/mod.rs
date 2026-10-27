//! The actions the `daybook` command, `daybook://` links and the HTTP API share (docs/automation.md):
//! `state`, `show`, `status`, `slack` and `command`. Errors never open a window; each entry reports
//! them its own way.

pub mod cli;
mod request;
pub mod state;

use std::time::Duration;

use serde::Serialize;
use serde_json::{Map, Value, json};
use tauri::{AppHandle, Manager};
use ts_rs::TS;

pub use request::{Action, AutomationError, Params, Request};
pub use state::AutomationState;

use crate::bus::{self, Host};
use crate::error::{AppError, AppResult};
use crate::http_api::{self, ApiStatus};
use crate::state::AppState;
use crate::store::settings::SlackStatus;
use crate::{action_log, slack, stage, status, window};
use cli::CliInfo;

/// Lets the answer to a command that quits reach its caller before the app exits.
const QUIT_DELAY: Duration = Duration::from_millis(200);

/// What an action returns besides the state.
pub type Outcome = Map<String, Value>;

/// Runs `action` and returns its result with the state after it under `state`.
pub async fn run(
    app: &AppHandle,
    action: &str,
    params: &Params,
) -> Result<Outcome, AutomationError> {
    let action = Action::parse(action)?;
    if action.needs_vault() && !app.has_vault() {
        return Err(AutomationError::no_vault());
    }
    let request = Request::read(action, params)?;
    let mut outcome = match request {
        Request::State => perform(app, request).await?,
        request => {
            let title = format!("Automation: {}", action.name());
            action_log::traced(&title, perform(app, request)).await?
        }
    };
    outcome.insert("state".into(), json!(state::now(app).await));
    Ok(outcome)
}

fn result(word: &str) -> Outcome {
    let mut outcome = Outcome::new();
    outcome.insert("result".into(), json!(word));
    outcome
}

async fn perform(app: &AppHandle, request: Request) -> Result<Outcome, AutomationError> {
    match request {
        Request::State => Ok(Outcome::new()),
        Request::Show => {
            window::show_main_in_background(app);
            Ok(Outcome::new())
        }
        Request::Status { status, note } => {
            if status::today(app).status == Some(status) {
                return Ok(result("unchanged"));
            }
            status::set_status(app, status, &note, None, false)?;
            Ok(result("set"))
        }
        Request::SlackLike { status, note } => {
            let settings = app.state::<AppState>().settings.get();
            let mapping = settings.slack.statuses.get(status).clone();
            slack::set_now(app, &mapping, &note).await?;
            Ok(result("set"))
        }
        Request::SlackCustom {
            emoji,
            text,
            presence,
        } => {
            slack::set_now(app, &SlackStatus::new(&emoji, &text, presence), "").await?;
            Ok(result("set"))
        }
        Request::Command { id } => {
            command(app, &id).await?;
            Ok(Outcome::new())
        }
    }
}

/// Runs a command by id and waits for it; one that quits the app is answered first.
pub(crate) async fn command(app: &AppHandle, id: &str) -> Result<(), AutomationError> {
    let commands = app.state::<bus::Commands>();
    let info = commands
        .info(id)
        .ok_or_else(|| AutomationError::new(404, format!("Unknown command “{id}”.")))?;
    if info.needs_vault && !app.has_vault() {
        return Err(AutomationError::no_vault());
    }
    if !commands.is_enabled(app, id) {
        return Err(AutomationError::bad(format!(
            "“{id}” isn't available right now."
        )));
    }
    if info.quits {
        let (app, id) = (app.clone(), id.to_string());
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(QUIT_DELAY).await;
            app.state::<bus::Commands>().run(app.clone(), &id).await;
        });
        return Ok(());
    }
    Ok(commands.run_or_err(app.clone(), id).await?)
}

/// Settings → Automation.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct AutomationInfo {
    pub cli: CliInfo,
    pub api: ApiStatus,
    pub token: String,
    /// `http://127.0.0.1:<port>/v1`
    pub base_url: String,
    /// Requests to copy, with `<token>` for the token.
    pub examples: Vec<String>,
    /// The Stage discovery file while Stage can find Daybook.
    pub stage_file: Option<String>,
    /// When a presenter last sent a request, in epoch milliseconds; memory only.
    #[ts(type = "number | null")]
    pub last_presenter_request: Option<i64>,
    /// `stage register <url> --token <token>`, to paste into a terminal.
    pub register_command: String,
}

#[tauri::command]
pub async fn automation_info(app: AppHandle) -> AppResult<AutomationInfo> {
    let token = http_api::token(&app).await.map_err(AppError::Io)?;
    let api = http_api::api_status(&app);
    let presentations = stage::presentations_info(&app, api.port, token.expose());
    Ok(AutomationInfo {
        stage_file: presentations.stage_file,
        last_presenter_request: presentations.last_request,
        register_command: presentations.register_command,
        cli: cli::cli_info(),
        token: token.expose().to_string(),
        base_url: http_api::base_url(api.port),
        examples: http_api::examples(api.port),
        api,
    })
}
