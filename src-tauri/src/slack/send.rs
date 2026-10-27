//! Sending a status to Slack: in the background with a retry, or right away for automation; each send
//! is a line in the action log, and a failure is kept as the last error for Settings → Status.

use std::fmt::Display;

use chrono::Local;
use tauri::{AppHandle, Manager};

use super::api::{self, Applied};
use super::decide::{Decision, changes_nothing, describe_status};
use super::sender::{Outcome, SendError};
use super::status::Step;
use super::{NOT_CONNECTED, changed, slack, token};
use crate::action_log::{self, ActionState, ActionTarget, Logged};
use crate::error::{AppError, AppResult};
use crate::lock::MutexExt;
use crate::notify;
use crate::state::AppState;
use crate::store::settings::SlackStatus;

/// The last failed send since the start: `HH:MM` and the error.
pub fn last_error(app: &AppHandle) -> Option<(String, String)> {
    slack(app).last_error.lock_unpoisoned().clone()
}

fn record_error(app: &AppHandle, error: &str) {
    let at = Local::now().format("%H:%M").to_string();
    *slack(app).last_error.lock_unpoisoned() = Some((at, error.to_string()));
}

/// Moves a send's action-log line on by its result; a failure is remembered as the last error.
pub(crate) fn logged_result<T, E: Display>(
    app: &AppHandle,
    logged: &Logged,
    result: Result<T, E>,
) -> Result<T, E> {
    match &result {
        Ok(_) => logged.update(ActionState::Done, None),
        Err(e) => {
            record_error(app, &e.to_string());
            logged.update(ActionState::Failed, Some(e.to_string()));
        }
    }
    result
}

pub(super) fn run(app: &AppHandle, step: Step) {
    match step {
        Step::Idle => {}
        Step::Skip {
            mapping,
            note,
            reason,
        } => {
            let described = describe_status(&mapping, &note);
            log::info!("Slack status {described}: {reason}");
            action_log::log(
                ActionTarget::SlackStatus,
                &described,
                ActionState::Skipped,
                Some(reason.into()),
            );
        }
        Step::Send(decision) => push(app, decision),
    }
}

/// What a decision sends: the status, its note, its expiry and whether it is a call's.
fn sending(decision: Decision) -> (SlackStatus, String, i64, bool) {
    match decision {
        Decision::Call(mapping) => (mapping, String::new(), 0, true),
        Decision::HeadsUp {
            mapping,
            expiration,
            detail,
        } => {
            log::info!("{detail}");
            (mapping, String::new(), expiration, false)
        }
        Decision::Status { mapping, note } | Decision::WorkflowOff { mapping, note } => {
            (mapping, note, 0, false)
        }
    }
}

/// Sends a decision in the background; a call's status counts as shown once Slack took it.
fn push(app: &AppHandle, decision: Decision) {
    let (mapping, note, expiration, is_call) = sending(decision);
    let described = describe_status(&mapping, &note);
    if changes_nothing(&mapping) {
        log::info!("Slack status {described}: set to \"no change\"");
        action_log::log(
            ActionTarget::SlackStatus,
            &described,
            ActionState::Skipped,
            Some("Set to “no change”".into()),
        );
        return;
    }
    let logged = action_log::sending(ActionTarget::SlackStatus, &described);
    let app = app.clone();
    action_log::spawn(async move {
        let attempt = || async {
            let token = token(&app).await.ok_or(SendError::NotConnected)?;
            slack(&app)
                .api
                .apply(&token, &mapping, &note, expiration)
                .await
                .map_err(SendError::Slack)
        };
        let on_failure = |error: &api::SlackError, retry_follows: bool| {
            log::warn!("Setting the Slack status failed: {error}");
            let body = if retry_follows {
                format!("{error} Retrying in 30 seconds.")
            } else {
                error.to_string()
            };
            record_error(&app, &error.to_string());
            let detail = if retry_follows {
                body.clone()
            } else {
                format!("{error} (also on retry)")
            };
            logged.update(ActionState::Failed, Some(detail));
            notify::show(&app, "Slack status not updated", &body);
        };
        let outcome = slack(&app).sender.push(attempt, on_failure).await;
        report(
            &app,
            outcome,
            &described,
            is_call.then_some(&mapping),
            &logged,
        );
    });
}

/// Logs a send and records what Slack took; `call` is the call's status when it was one.
fn report(
    app: &AppHandle,
    outcome: Outcome<Applied>,
    described: &str,
    call: Option<&SlackStatus>,
    logged: &Logged,
) {
    match outcome {
        Outcome::Sent(applied) => {
            log::info!("Slack status set: {described}");
            logged.update(ActionState::Done, applied.problem.map(String::from));
            if let Some(problem) = applied.problem {
                log::info!("{problem}");
            }
            if let Some(call) = call {
                slack(app).live.lock_unpoisoned().call_sent(call);
            }
            record_shown(app, applied);
        }
        Outcome::Replaced => {
            log::info!("Slack status {described}: replaced by a newer one");
            logged.update(
                ActionState::Skipped,
                Some("Replaced by a newer status".into()),
            );
        }
        Outcome::NotConnected => {
            log::info!("Slack status {described}: Slack isn't connected");
            logged.update(ActionState::Skipped, Some(NOT_CONNECTED.into()));
        }
        Outcome::Failed(_) => {}
    }
}

/// Sets a Slack status right away and waits for Slack, outside the contexts and workflows; a pending
/// send and its retry are dropped.
pub async fn set_now(app: &AppHandle, mapping: &SlackStatus, note: &str) -> AppResult<()> {
    let described = describe_status(mapping, note);
    let logged = action_log::sending(ActionTarget::SlackStatus, &described);
    let Some(token) = token(app).await else {
        logged.update(ActionState::Skipped, Some(NOT_CONNECTED.into()));
        return Err(AppError::InvalidInput(
            "Connect Slack in Settings → Slack first.".into(),
        ));
    };
    slack(app).sender.supersede();
    let applied = slack(app)
        .api
        .apply(&token, mapping, note, 0)
        .await
        .map_err(|e| AppError::Network(e.to_string()));
    let applied = logged_result(app, &logged, applied)?;
    if let Some(problem) = applied.problem {
        logged.update(ActionState::Done, Some(problem.into()));
    }
    log::info!("Slack status set: {described}");
    if let Some(problem) = applied.problem {
        log::info!("{problem}");
    }
    record_shown(app, applied);
    Ok(())
}

fn record_shown(app: &AppHandle, applied: Applied) {
    let files = &app.state::<AppState>().slack;
    if let Err(e) = files
        .shown
        .update(|shown| shown.record(applied.profile, applied.presence))
    {
        log::warn!("Couldn't remember the Slack status: {e}");
    }
    changed(app);
}
