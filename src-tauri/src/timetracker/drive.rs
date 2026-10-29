//! The status drives TimeTracker's timer (docs/timetracker.md#the-status-drives-the-timer; the decisions
//! are in `status_timer`); calls follow in `call_drive`.

use tauri::{AppHandle, Listener, Manager};

use super::feedback::{NOT_STARTED, NOT_STOPPED, context, timer_failed};
use super::status_timer::{TimerKnown, TimerStep, needs_last_timer, timer_step};
use super::{blocking, call_drive, client, tracking};
use crate::action_log;
use crate::activity::{self, session::ActivityMode};
use crate::events::STATUS_CHANGED_EVENT;
use crate::state::AppState;
use crate::status::StatusChanged;

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

pub fn install(app: &AppHandle) {
    listen_status(app);
    call_drive::follow_calls(app);
}
