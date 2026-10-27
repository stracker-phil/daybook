//! The status drives TimeTracker's timer and a finished call goes to TimeTracker as a suggestion
//! (docs/timetracker.md#status-and-calls). The decisions are in `status_timer`, `call_suggestion` and
//! `split_calls` (a call that restarts within `split_calls::RESTART_WITHIN` is one suggestion).

use std::sync::{Arc, Mutex};

use chrono::{Local, NaiveDateTime};
use tauri::{AppHandle, Listener, Manager};
use tokio::sync::broadcast::error::RecvError;

use super::call_suggestion::{CallSuggestion, FinishedCall, call_suggestion};
use super::split_calls::{EndedCall, SplitCalls};
use super::status_timer::{TimerKnown, TimerStep, needs_last_timer, timer_step};
use super::{blocking, client, tracking};
use crate::action_log;
use crate::activity::{self, session::ActivityMode};
use crate::calls::{self, ActiveCall, Announced, CallEvent};
use crate::error::{AppError, ErrorKind};
use crate::events::STATUS_CHANGED_EVENT;
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::status::{self, StatusChanged};
use crate::store::settings::{Feature, Project, Settings};
use crate::{calendar, notify, projects};

const NOT_RUNNING: &str = "TimeTracker isn't running";
const REFUSED: &str = "TimeTracker refused the timer";
const NOT_STARTED: &str = "The timer wasn't started.";
const NOT_STOPPED: &str = "The timer wasn't stopped.";
const OPEN: (&str, &str) = ("timetracker:show", "Open TimeTracker");

/// The current context's projects and whether it has the Status feature.
fn context<'a>(app: &AppHandle, settings: &'a Settings) -> (Vec<&'a Project>, bool) {
    let state = app.state::<AppState>();
    let context = state.contexts.current(settings);
    let offered = projects::current_projects(&settings.projects, context.map(|c| c.id.as_str()));
    let status_feature = context.is_some_and(|c| c.features.contains(&Feature::Status));
    (offered, status_feature)
}

/// The notification with "Open TimeTracker".
fn not_running(app: &AppHandle, body: &str) {
    log::info!("{body} {NOT_RUNNING}");
    notify::show_with_action(app, NOT_RUNNING, body, OPEN);
}

/// Not running: a notification with "Open TimeTracker"; missing: only the log.
fn timer_failed(app: &AppHandle, error: &AppError, body: &str) {
    match error.kind() {
        ErrorKind::TimetrackerMissing => log::info!("{body} TimeTracker isn't installed"),
        ErrorKind::TimetrackerNotRunning => not_running(app, body),
        _ => {
            log::info!("{REFUSED}: {error}");
            notify::show(app, REFUSED, &error.to_string());
        }
    }
}

async fn run_step(app: &AppHandle, step: TimerStep) {
    match step {
        TimerStep::Nothing => {}
        TimerStep::Ask => {
            log::info!("Status → timer: asked what you work on");
            if let Err(e) = activity::open(app, ActivityMode::Switch, None).await {
                log::warn!("Couldn't ask what you work on: {e}");
            }
        }
        TimerStep::Stop => match blocking(app, |tool| client::stop(tool)).await {
            Ok(()) => log::info!("Status → timer: stopped"),
            Err(e) => timer_failed(app, &e, NOT_STOPPED),
        },
        TimerStep::Start {
            project_id,
            note,
            tag,
        } => {
            let line = format!("{project_id} · {note} · {}", tag.as_deref().unwrap_or("–"));
            let started = blocking(app, move |tool| {
                client::start(tool, &project_id, &note, tag.as_deref())
            })
            .await;
            match started {
                Ok(()) => log::info!("Status → timer: started {line}"),
                Err(e) => timer_failed(app, &e, NOT_STARTED),
            }
        }
    }
}

async fn status_changed(app: &AppHandle, change: StatusChanged) {
    let settings = app.state::<AppState>().settings.get();
    if !settings.timetracker.status_timer {
        return;
    }
    let timer = TimerKnown::of(&tracking(app));
    let last = if needs_last_timer(&change, timer) {
        match blocking(app, |tool| client::recent(tool, 1)).await {
            Ok(timers) => timers.into_iter().next(),
            Err(e) => return timer_failed(app, &e, NOT_STARTED),
        }
    } else {
        None
    };
    let (offered, status_feature) = context(app, &settings);
    let step = timer_step(&change, timer, last.as_ref(), &offered, status_feature);
    run_step(app, step).await;
}

async fn send(app: &AppHandle, suggestion: CallSuggestion) {
    let CallSuggestion {
        project_id,
        project,
        start,
        end,
        note,
    } = suggestion;
    let span = format!("{}–{}", start.format("%H:%M"), end.format("%H:%M"));
    let sent_note = note.clone();
    let recorded = blocking(app, move |tool| {
        client::record(tool, &project_id, (start, end), &sent_note)
    })
    .await;
    match recorded {
        Ok(id) => log::info!(
            "Call → TimeTracker: suggested {project} {span} \"{note}\" ({})",
            id.as_deref().unwrap_or("dry run")
        ),
        Err(e) if e.kind() == ErrorKind::TimetrackerNotRunning => {
            not_running(app, &format!("The call “{note}” wasn't sent."));
        }
        Err(e) => log::info!("Call → TimeTracker: \"{note}\" not sent: {e}"),
    }
}

async fn call_ended(app: &AppHandle, ended: EndedCall) {
    let EndedCall { call, end, status } = ended;
    let settings = app.state::<AppState>().settings.get();
    let (offered, status_feature) = context(app, &settings);
    let today = Local::now().date_naive();
    let events = calendar::events_on(app, call.since.date(), today).await;
    let finished = FinishedCall {
        call: &call,
        end,
        status,
        status_feature,
        events: &events,
        offered: &offered,
    };
    match call_suggestion(&finished) {
        Ok(suggestion) => send(app, suggestion).await,
        Err(skip) => log::info!(
            "Call → TimeTracker: {} - {} not sent: {}",
            call.app,
            call.title,
            skip.reason()
        ),
    }
}

fn listen_status(app: &AppHandle) {
    let handle = app.clone();
    app.listen(
        STATUS_CHANGED_EVENT,
        move |event| match serde_json::from_str::<StatusChanged>(event.payload()) {
            Ok(change) => {
                let app = handle.clone();
                action_log::spawn(async move { status_changed(&app, change).await });
            }
            Err(e) => log::warn!("Couldn't read a status change: {e}"),
        },
    );
}

type Split = Arc<Mutex<SplitCalls>>;

/// The call waits for a restart of its app; once it is final it is suggested.
fn settle(
    app: &AppHandle,
    split: &Split,
    call: ActiveCall,
    end: NaiveDateTime,
) -> Option<EndedCall> {
    let status = status::today(app).status;
    let (ready, due_at) = {
        let mut split = split.lock_unpoisoned();
        let ready = split.ended(EndedCall { call, end, status });
        (ready, split.due_at())
    };
    let Some(due_at) = due_at else {
        return ready;
    };
    let wait = (due_at - Local::now().naive_local())
        .to_std()
        .unwrap_or_default();
    let (app, split) = (app.clone(), Arc::clone(split));
    action_log::spawn(async move {
        tokio::time::sleep(wait).await;
        let due = split.lock_unpoisoned().due(Local::now().naive_local());
        if let Some(call) = due {
            call_ended(&app, call).await;
        }
    });
    ready
}

async fn follow(app: &AppHandle, split: &Split, event: CallEvent) {
    let mut done = Vec::new();
    match event {
        CallEvent::Started(call) => done.extend(split.lock_unpoisoned().started(&call)),
        CallEvent::Switched { from, to, end } => {
            done.extend(settle(app, split, from, end));
            done.extend(split.lock_unpoisoned().started(&to));
        }
        CallEvent::Ended { call, end, .. } => done.extend(settle(app, split, call, end)),
        CallEvent::Changed(_) => {}
    }
    for call in done {
        call_ended(app, call).await;
    }
}

fn follow_calls(app: &AppHandle) {
    let handle = app.clone();
    let split = Split::default();
    let mut rx = calls::subscribe(app);
    tauri::async_runtime::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(Announced { event, trigger }) => {
                    action_log::within(trigger, follow(&handle, &split, event)).await;
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
}

pub fn install(app: &AppHandle) {
    listen_status(app);
    follow_calls(app);
}
