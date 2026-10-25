//! Slack (docs/slack.md): the profile status and presence follow the status, calls and calendar
//! heads-ups; a status change posts the confirmed channel message. Only in contexts with the Slack
//! feature, once a token is stored.

pub mod api;
pub mod channel;
pub mod commands;
pub mod day_message;
pub mod decide;
#[cfg(test)]
mod fake_slack;
pub mod sender;
pub mod shown;
pub mod status;
mod token;

use std::path::Path;
use std::sync::Mutex;

use chrono::Local;
use tauri::{AppHandle, Emitter, Listener, Manager};

pub use decide::HeadsUp;
pub use shown::ShownStatus;

use self::api::{Applied, SlackApi, SlackIdentity};
use self::channel::{LastMessage, status_message};
use self::day_message::DayMessage;
use self::decide::{Decision, changes_nothing, describe_status};
use self::sender::{Outcome, SendError, Sender};
use self::status::{Live, Picked, Setup, Step};
use self::token::Tokens;
use crate::calls::{self, CallEvent};
use crate::events::{CONTEXT_CHANGED_EVENT, SLACK_CHANGED_EVENT, STATUS_CHANGED_EVENT};
use crate::lock::MutexExt;
use crate::secrets::Secret;
use crate::state::AppState;
use crate::status::StatusChanged;
use crate::store::json_store::JsonStore;
use crate::store::settings::{Feature, SlackStatus};
use crate::{notify, status as day_status};

/// What Slack shows and the day's message, kept across restarts.
pub struct SlackFiles {
    shown: JsonStore<ShownStatus>,
    day_message: JsonStore<DayMessage>,
}

impl SlackFiles {
    pub fn open(config_dir: &Path) -> Self {
        Self {
            shown: JsonStore::open_with(config_dir.join(shown::FILE_NAME), ShownStatus::normalized),
            day_message: JsonStore::open(config_dir.join(day_message::FILE_NAME)),
        }
    }
}

#[derive(Default)]
pub struct Slack {
    api: SlackApi,
    tokens: Tokens,
    sender: Sender,
    live: Mutex<Live>,
    last_message: Mutex<LastMessage>,
    /// From the last token check, for Settings.
    identity: Mutex<Option<SlackIdentity>>,
}

fn slack(app: &AppHandle) -> &Slack {
    app.state::<Slack>().inner()
}

/// The stored token; the first read asks the Keychain.
async fn token(app: &AppHandle) -> Option<Secret> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || slack(&handle).tokens.get())
        .await
        .unwrap_or_default()
}

pub fn in_context(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    state
        .contexts
        .current(&settings)
        .is_some_and(|context| context.features.contains(&Feature::Slack))
}

fn changed(app: &AppHandle) {
    if let Err(e) = app.emit(SLACK_CHANGED_EVENT, ()) {
        log::warn!("Couldn't announce the Slack change: {e}");
    }
}

/// Steps the live state with what the owner set up now.
fn step(app: &AppHandle, change: impl FnOnce(&mut Live, &Setup) -> Step) {
    let settings = app.state::<AppState>().settings.get();
    let setup = Setup {
        slack_on: in_context(app),
        statuses: &settings.slack.statuses,
        workflows: &settings.slack.manual_workflows,
        today: day_status::today(app).status,
    };
    let next = change(&mut slack(app).live.lock_unpoisoned(), &setup);
    run(app, next);
}

fn run(app: &AppHandle, step: Step) {
    match step {
        Step::Idle => {}
        Step::Skip {
            mapping,
            note,
            reason,
        } => log::info!(
            "Slack status {}: {reason}",
            describe_status(&mapping, &note)
        ),
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
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
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
            notify::show(&app, "Slack status not updated", &body);
        };
        let outcome = slack(&app).sender.push(attempt, on_failure).await;
        report(&app, outcome, &described, is_call.then_some(&mapping));
    });
}

/// Logs a send and records what Slack took; `call` is the call's status when it was one.
fn report(app: &AppHandle, outcome: Outcome<Applied>, described: &str, call: Option<&SlackStatus>) {
    match outcome {
        Outcome::Sent(applied) => {
            log::info!("Slack status set: {described}");
            if let Some(problem) = applied.problem {
                log::info!("{problem}");
            }
            if let Some(call) = call {
                slack(app).live.lock_unpoisoned().call_sent(call);
            }
            record_shown(app, applied);
        }
        Outcome::Replaced => log::info!("Slack status {described}: replaced by a newer one"),
        Outcome::NotConnected => log::info!("Slack status {described}: Slack isn't connected"),
        Outcome::Failed(_) => {}
    }
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

/// What the sidebar shows: Slack's status now, in contexts with Slack once a token is stored.
pub async fn shown_now(app: &AppHandle) -> Option<ShownStatus> {
    if !in_context(app) || token(app).await.is_none() {
        return None;
    }
    let now_ms = Local::now().timestamp_millis() as f64;
    Some(
        app.state::<AppState>()
            .slack
            .shown
            .read(|shown| shown.at(now_ms)),
    )
}

/// A calendar heads-up started, changed or ended.
pub fn set_heads_up(app: &AppHandle, next: Option<HeadsUp>) {
    step(app, |live, setup| live.heads_up_changed(next, setup));
}

fn status_changed(app: &AppHandle, change: &StatusChanged) {
    let picked = Picked {
        status: change.next,
        note: change.message.clone(),
        project: None,
    };
    step(app, |live, setup| live.status_picked(picked, setup));
    let settings = app.state::<AppState>().settings.get();
    if in_context(app)
        && let Some(text) = status_message(change, &settings)
    {
        post_message(app, &settings.slack.channel, text, change);
    }
}

/// Posts a status change's message; the day's first Working message is remembered for the focus list.
fn post_message(app: &AppHandle, channel: &str, text: String, change: &StatusChanged) {
    let channel = channel.trim().to_string();
    if channel.is_empty() {
        return;
    }
    if !slack(app)
        .last_message
        .lock_unpoisoned()
        .admit(&change.message)
    {
        log::info!("Not posted to {channel}: the same message as the last one");
        return;
    }
    let first = change.first;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let Some(token) = token(&app).await else {
            log::info!("Not posted to {channel}: Slack isn't connected");
            return;
        };
        match slack(&app).api.post_message(&token, &channel, &text).await {
            Ok(posted) => {
                log::info!("Posted to {channel}");
                if first {
                    remember_day_message(&app, &posted, &text);
                }
            }
            Err(e) => {
                log::warn!("Posting the status message failed: {e}");
                notify::show(&app, "Slack message not sent", &format!("{channel}: {e}"));
            }
        }
    });
}

fn remember_day_message(app: &AppHandle, posted: &api::Posted, text: &str) {
    let today = Local::now().format("%Y-%m-%d").to_string();
    let remembered = app.state::<AppState>().slack.day_message.update(|day| {
        if let Some(next) = day.remember(&today, &posted.channel, &posted.ts, text) {
            *day = next;
        }
    });
    if let Err(e) = remembered {
        log::warn!("Couldn't remember the day's message: {e}");
    }
}

/// A call's start or end; a new title for the same call changes nothing.
fn call_changed(app: &AppHandle, event: CallEvent) {
    let status = match event {
        CallEvent::Started(call) => match trigger_status(app, &call.app) {
            None => {
                log::warn!(
                    "No app trigger named {} for the call's Slack status",
                    call.app
                );
                return;
            }
            status => status,
        },
        CallEvent::Ended { .. } => None,
        CallEvent::Changed(_) => return,
    };
    step(app, |live, setup| live.call_changed(status, setup));
}

/// The Slack status of the app trigger named `name`.
fn trigger_status(app: &AppHandle, name: &str) -> Option<SlackStatus> {
    let settings = app.state::<AppState>().settings.get();
    settings
        .meetings
        .rules
        .iter()
        .find(|rule| rule.app == name)
        .map(|rule| rule.slack.clone())
}

pub fn install(app: &AppHandle) {
    let handle = app.clone();
    app.listen(
        STATUS_CHANGED_EVENT,
        move |event| match serde_json::from_str::<StatusChanged>(event.payload()) {
            Ok(change) => status_changed(&handle, &change),
            Err(e) => log::warn!("Couldn't read a status change: {e}"),
        },
    );
    let handle = app.clone();
    app.listen(CONTEXT_CHANGED_EVENT, move |_| {
        step(&handle, |live, setup| live.context_changed(setup));
    });
    follow_calls(app);
}

/// Every call start and end, for as long as the app runs.
fn follow_calls(app: &AppHandle) {
    let handle = app.clone();
    let mut calls = calls::subscribe(app);
    tauri::async_runtime::spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        loop {
            match calls.recv().await {
                Ok(event) => call_changed(&handle, event),
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
}
