//! Calls and TimeTracker (docs/timetracker.md#calls-become-suggestions): a call pauses the timer and
//! shows as a suggestion that grows while it runs. One task handles the call events, the minute tick and
//! the final call in turn, so they never race.

use std::time::Duration;

use chrono::{Local, NaiveDateTime};
use tauri::{AppHandle, Manager};
use tokio::sync::broadcast::error::RecvError;
use tokio::time::MissedTickBehavior;

use super::call_suggestion::{CallSuggestion, FinishedCall, Skip, call_suggestion, same_day_end};
use super::call_timer::{CallStart, PausedTimer, pause_for_call, resume_after_call};
use super::feedback::{NOT_STARTED, NOT_STOPPED, context, not_running, timer_failed};
use super::live_call::{Settle, settle};
use super::split_calls::{EndedCall, SplitCalls};
use super::{blocking, client, tracking};
use crate::action_log::{self, Trigger};
use crate::calls::{self, ActiveCall, Announced, CallEvent};
use crate::error::{AppError, ErrorKind};
use crate::state::AppState;
use crate::store::settings::Status;
use crate::{calendar, status};

/// How often a running call's suggestion grows.
const GROW_EVERY: Duration = Duration::from_secs(60);
/// The id a debug build's dry run stands in for the suggestion's, so it isn't sent again each minute.
const DRY_RUN_ID: &str = "dry-run";

#[derive(Default)]
struct CallState {
    split: SplitCalls,
    running: Option<ActiveCall>,
    paused: Option<PausedTimer>,
    /// The ended call's trigger, which its final suggestion joins.
    waiting_trigger: Option<Trigger>,
}

fn now() -> NaiveDateTime {
    Local::now().naive_local()
}

async fn suggestion_of(
    app: &AppHandle,
    call: &ActiveCall,
    end: NaiveDateTime,
    status: Option<Status>,
) -> Result<CallSuggestion, Skip> {
    let settings = app.state::<AppState>().settings.get();
    let (offered, status_feature) = context(app, &settings);
    let events = calendar::events_on(app, call.since.date(), now().date()).await;
    call_suggestion(&FinishedCall {
        call,
        end,
        status,
        status_feature,
        events: &events,
        offered: &offered,
    })
}

fn not_sent(app: &AppHandle, error: &AppError, note: &str) {
    if error.kind() == ErrorKind::TimetrackerNotRunning {
        not_running(app, &format!("The call “{note}” wasn't sent."));
    } else {
        log::info!("Call → TimeTracker: \"{note}\" not sent: {error}");
    }
}

/// Records the suggestion; its id (a stand-in in a dry run), `None` when it failed.
async fn record(app: &AppHandle, suggestion: CallSuggestion) -> Option<String> {
    let CallSuggestion {
        project_id,
        project,
        start,
        end,
        note,
    } = suggestion;
    let sent_note = note.clone();
    let recorded = blocking(app, move |tool| {
        client::record(tool, project_id.as_deref(), (start, end), &sent_note)
    })
    .await;
    match recorded {
        Ok(id) => {
            let id = id.unwrap_or_else(|| DRY_RUN_ID.into());
            log::info!(
                "Call → TimeTracker: suggested {} {}–{} \"{note}\" ({id})",
                project.as_deref().unwrap_or("without a project"),
                start.format("%H:%M"),
                end.format("%H:%M"),
            );
            Some(id)
        }
        Err(e) => {
            not_sent(app, &e, &note);
            None
        }
    }
}

/// The suggestion of the running (joined) call reaches `end`.
async fn extend(app: &AppHandle, id: &str, call: &ActiveCall, end: NaiveDateTime) {
    let (id, times) = (id.to_string(), (call.since, same_day_end(call.since, end)));
    if let Err(e) = blocking(app, move |tool| client::extend(tool, &id, times)).await {
        log::info!("Call → TimeTracker: the suggestion didn't grow: {e}");
    }
}

/// Every minute: the running call's suggestion grows; one not sent yet goes once the call passes.
async fn grow(app: &AppHandle, calls: &mut CallState) {
    let Some(running) = &calls.running else {
        return;
    };
    let call = calls.split.live(running);
    let end = now();
    if let Some(id) = calls.split.sent() {
        return extend(app, id, &call, end).await;
    }
    let status = status::today(app).status;
    let Ok(suggestion) = suggestion_of(app, &call, end, status).await else {
        return;
    };
    let title = format!("Call → TimeTracker: {}", call.app);
    if let Some(id) = action_log::traced(&title, record(app, suggestion)).await {
        calls.split.set_sent(id);
    }
}

async fn update(app: &AppHandle, id: String, suggestion: CallSuggestion) {
    let note = suggestion.note.clone();
    let updated = blocking(app, move |tool| {
        client::update(
            tool,
            &id,
            suggestion.project_id.as_deref(),
            (suggestion.start, suggestion.end),
            &suggestion.note,
        )
    })
    .await;
    match updated {
        Ok(()) => log::info!("Call → TimeTracker: \"{note}\" final"),
        Err(e) => not_sent(app, &e, &note),
    }
}

async fn dismiss(app: &AppHandle, id: String, call_app: &str, skip: Skip) {
    log::info!(
        "Call → TimeTracker: {call_app} dismissed: {}",
        skip.reason()
    );
    if let Err(e) = blocking(app, move |tool| client::dismiss(tool, &id)).await {
        log::info!("Call → TimeTracker: the suggestion wasn't dismissed: {e}");
    }
}

async fn finish(app: &AppHandle, ended: EndedCall) {
    let EndedCall {
        call,
        end,
        status,
        sent,
    } = ended;
    let suggestion = suggestion_of(app, &call, end, status).await;
    match settle(sent, suggestion) {
        Settle::Record(suggestion) => {
            record(app, suggestion).await;
        }
        Settle::Update { id, suggestion } => update(app, id, suggestion).await,
        Settle::Dismiss { id, skip } => dismiss(app, id, &call.app, skip).await,
        Settle::Skip(skip) => log::info!(
            "Call → TimeTracker: {} - {} not sent: {}",
            call.app,
            call.title,
            skip.reason()
        ),
    }
}

async fn pause(app: &AppHandle, calls: &mut CallState) {
    if calls.paused.is_some() {
        return;
    }
    let settings = app.state::<AppState>().settings.get();
    let (_, status_feature) = context(app, &settings);
    let timer = tracking(app).timer;
    let start = CallStart {
        status_timer: settings.timetracker.status_timer,
        status: status::today(app).status,
        status_feature,
        timer: timer.as_ref(),
    };
    let Some(paused) = pause_for_call(&start) else {
        return;
    };
    match blocking(app, |tool| client::stop(tool)).await {
        Ok(()) => {
            log::info!("Call → timer: paused {}", paused.project);
            calls.paused = Some(paused);
        }
        Err(e) => timer_failed(app, &e, NOT_STOPPED),
    }
}

async fn resume(app: &AppHandle, calls: &mut CallState) {
    let Some(paused) = calls.paused.take() else {
        return;
    };
    let status = status::today(app).status;
    let running = tracking(app).timer.is_some();
    let Some(PausedTimer {
        project_id,
        project,
        note,
        tag,
    }) = resume_after_call(paused, status, running)
    else {
        return log::info!("Call → timer: not resumed (not Working, or another timer runs)");
    };
    let started = blocking(app, move |tool| {
        client::start(tool, &project_id, &note, tag.as_deref())
    })
    .await;
    match started {
        Ok(()) => log::info!("Call → timer: resumed {project}"),
        Err(e) => timer_failed(app, &e, NOT_STARTED),
    }
}

/// The call ended: its suggestion reaches the end and it waits for a restart.
async fn ended(app: &AppHandle, calls: &mut CallState, call: ActiveCall, end: NaiveDateTime) {
    if let Some(id) = calls.split.sent() {
        extend(app, id, &calls.split.live(&call), end).await;
    }
    let status = status::today(app).status;
    let final_call = calls.split.ended(EndedCall {
        call,
        end,
        status,
        sent: None,
    });
    calls.waiting_trigger = action_log::current();
    if let Some(final_call) = final_call {
        finish(app, final_call).await;
    }
}

async fn started(app: &AppHandle, calls: &mut CallState, call: ActiveCall) {
    if let Some(final_call) = calls.split.started(&call) {
        finish(app, final_call).await;
    }
    calls.running = Some(call);
}

async fn follow(app: &AppHandle, calls: &mut CallState, event: CallEvent) {
    match event {
        CallEvent::Started(call) => {
            started(app, calls, call).await;
            pause(app, calls).await;
        }
        CallEvent::Changed(call) => calls.running = Some(call),
        CallEvent::Switched { from, to, end } => {
            ended(app, calls, from, end).await;
            started(app, calls, to).await;
        }
        CallEvent::Ended { call, end, .. } => {
            calls.running = None;
            ended(app, calls, call, end).await;
            resume(app, calls).await;
        }
    }
}

async fn finish_due(app: &AppHandle, calls: &mut CallState) {
    if let Some(final_call) = calls.split.due(now()) {
        let trigger = calls.waiting_trigger.take();
        action_log::within(trigger, finish(app, final_call)).await;
    }
}

pub fn follow_calls(app: &AppHandle) {
    let app = app.clone();
    let mut rx = calls::subscribe(&app);
    tauri::async_runtime::spawn(async move {
        let mut calls = CallState::default();
        let mut grow_tick = tokio::time::interval(GROW_EVERY);
        grow_tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            let due = calls
                .split
                .due_at()
                .map(|at| (at - now()).to_std().unwrap_or_default());
            tokio::select! {
                received = rx.recv() => match received {
                    Ok(Announced { event, trigger }) => {
                        action_log::within(trigger, follow(&app, &mut calls, event)).await;
                    }
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => break,
                },
                _ = grow_tick.tick() => grow(&app, &mut calls).await,
                () = tokio::time::sleep(due.unwrap_or_default()), if due.is_some() => {
                    finish_due(&app, &mut calls).await;
                }
            }
        }
    });
}
