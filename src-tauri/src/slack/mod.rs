//! Slack (docs/slack.md): the profile status and presence follow the status, calls and calendar
//! heads-ups; a status change posts the confirmed channel message. Only in contexts with the Slack
//! feature, once a token is stored.

pub mod api;
pub mod channel;
pub mod commands;
pub mod day;
pub mod day_message;
pub mod decide;
#[cfg(test)]
mod fake_slack;
mod lifecycle;
mod send;
pub mod sender;
pub mod shown;
pub mod status;
mod token;

use std::path::Path;
use std::sync::Mutex;

use chrono::Local;
use tauri::{AppHandle, Emitter, Listener, Manager};

pub use decide::HeadsUp;
pub use lifecycle::restore_on_exit;
pub use send::{last_error, set_now};
pub use shown::ShownStatus;

pub(crate) use send::logged_result;
use send::run;

use self::api::{SlackApi, SlackIdentity};
use self::channel::{LastMessage, status_message};
use self::day_message::DayMessage;
use self::sender::Sender;
use self::status::{Live, Picked, Setup, Step};
use self::token::Tokens;
use crate::action_log::{self, ActionState, ActionTarget};
use crate::calls::{self, Announced, CallEvent};
use crate::events::{CONTEXT_CHANGED_EVENT, SLACK_CHANGED_EVENT, STATUS_CHANGED_EVENT};
use crate::lock::MutexExt;
use crate::secrets::Secret;
use crate::state::AppState;
use crate::status::StatusChanged;
use crate::store::json_store::JsonStore;
use crate::store::settings::{Feature, SlackStatus};
use crate::{notify, status as day_status, timetracker};

/// The action log's detail for a send without a token.
pub const NOT_CONNECTED: &str = "Not sent: Slack isn't connected (Settings → Connections → Slack)";

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
    /// The last failed send, `HH:MM` and the error, for Settings → Status.
    last_error: Mutex<Option<(String, String)>>,
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

fn with_setup<R>(app: &AppHandle, work: impl FnOnce(&Setup) -> R) -> R {
    let settings = app.state::<AppState>().settings.get();
    work(&Setup {
        slack_on: in_context(app),
        statuses: &settings.slack.statuses,
        workflows: &settings.slack.manual_workflows,
        today: day_status::today(app).status,
    })
}

/// Steps the live state with what the owner set up now.
fn step(app: &AppHandle, change: impl FnOnce(&mut Live, &Setup) -> Step) {
    let next = with_setup(app, |setup| {
        change(&mut slack(app).live.lock_unpoisoned(), setup)
    });
    run(app, next);
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

/// TimeTracker's timer moved to another project (or none).
pub fn project_changed(app: &AppHandle, project: Option<String>) {
    step(app, |live, setup| live.project_changed(project, setup));
}

/// A calendar heads-up started, changed or ended.
pub fn set_heads_up(app: &AppHandle, next: Option<HeadsUp>) {
    if !slack(app)
        .live
        .lock_unpoisoned()
        .heads_up_differs(next.as_ref())
    {
        return;
    }
    let title = next.as_ref().map_or_else(
        || "Status rule ended".to_string(),
        |next| format!("Status rule: {}", next.label),
    );
    action_log::traced_sync(&title, |_| {
        step(app, |live, setup| live.heads_up_changed(next, setup));
    });
}

fn status_changed(app: &AppHandle, change: &StatusChanged) {
    let picked = Picked {
        status: change.next,
        note: change.message.clone(),
        project: timetracker::current_project(app),
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
    let described = format!("{channel}: {text}");
    if !slack(app)
        .last_message
        .lock_unpoisoned()
        .admit(&change.message)
    {
        log::info!("Not posted to {channel}: the same message as the last one");
        action_log::log(
            ActionTarget::SlackMessage,
            &described,
            ActionState::Skipped,
            Some("Not sent: same message as the last one".into()),
        );
        return;
    }
    let logged = action_log::sending(ActionTarget::SlackMessage, &described);
    let first = change.first;
    let app = app.clone();
    action_log::spawn(async move {
        let Some(token) = token(&app).await else {
            log::info!("Not posted to {channel}: Slack isn't connected");
            logged.update(ActionState::Skipped, Some(NOT_CONNECTED.into()));
            return;
        };
        let posted = slack(&app).api.post_message(&token, &channel, &text).await;
        match logged_result(&app, &logged, posted) {
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

/// A call's start, move or end; a new title for the same call changes nothing.
fn call_changed(app: &AppHandle, event: CallEvent) {
    let call = match event {
        CallEvent::Started(call) | CallEvent::Switched { to: call, .. } => call,
        CallEvent::Ended { .. } => {
            step(app, |live, setup| live.call_changed(None, setup));
            return;
        }
        CallEvent::Changed(_) => return,
    };
    let Some(status) = trigger_status(app, &call.bundle_id) else {
        log::warn!(
            "No app trigger for {} ({}) for the call's Slack status",
            call.app,
            call.bundle_id
        );
        return;
    };
    step(app, |live, setup| live.call_changed(Some(status), setup));
}

/// The Slack status of the app trigger for `bundle_id`.
fn trigger_status(app: &AppHandle, bundle_id: &str) -> Option<SlackStatus> {
    let settings = app.state::<AppState>().settings.get();
    settings
        .meetings
        .rules
        .iter()
        .find(|rule| rule.bundle_id == bundle_id)
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
    lifecycle::restore_after_restart(app);
}

/// Every call start, move and end, for as long as the app runs.
fn follow_calls(app: &AppHandle) {
    let handle = app.clone();
    let mut calls = calls::subscribe(app);
    tauri::async_runtime::spawn(async move {
        use tokio::sync::broadcast::error::RecvError;
        loop {
            match calls.recv().await {
                Ok(Announced { event, trigger }) => {
                    let _entered = trigger.as_ref().map(action_log::Trigger::enter);
                    call_changed(&handle, event);
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
}
