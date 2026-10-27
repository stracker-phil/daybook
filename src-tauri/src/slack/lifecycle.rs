//! A call status left on Slack by a quit or a crash mid-call is replaced: on quit, and after a
//! restart once the first check found no call.

use chrono::Local;
use tauri::{AppHandle, Manager};

use super::{send, slack, step, with_setup};
use crate::action_log;
use crate::calls;
use crate::lock::MutexExt;
use crate::state::AppState;

/// Slack shows a call status of one of the app triggers.
fn shows_call_status(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let rules = state.settings.get().meetings.rules;
    let now_ms = Local::now().timestamp_millis() as f64;
    state
        .slack
        .shown
        .read(|shown| shown.at(now_ms).is_call_status(&rules))
}

pub(super) fn restore_after_restart(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        calls::first_check(&app).await;
        if calls::active(&app).is_some() || !shows_call_status(&app) {
            return;
        }
        log::info!("Slack still shows a call status from before the restart");
        action_log::traced_sync("Call status left from before the restart", |_| {
            step(&app, |live, setup| live.restore_after_restart(setup));
        });
    });
}

/// Daybook quits: a call status Slack shows is replaced by the status wanted without the call.
pub async fn restore_on_exit(app: &AppHandle) {
    let decision = with_setup(app, |setup| {
        slack(app).live.lock_unpoisoned().quitting(setup)
    });
    if let Some(decision) = decision {
        log::info!("Quitting during a call: restoring the Slack status");
        send::send_now(app, decision).await;
    }
}
