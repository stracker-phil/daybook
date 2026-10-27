//! Calls and status changes pause, lower, resume or start the music, as the active context says
//! (`rules`). Steps run one after another, across events.

use std::sync::Mutex;

use tauri::{AppHandle, Listener, Manager};
use tokio::sync::broadcast::error::RecvError;

use super::rules::{self, Action, Held, Moment, Step};
use super::state::entry_label;
use crate::action_log::{self, ActionState, ActionTarget};
use crate::calls::{self, Announced, CallEvent};
use crate::error::AppResult;
use crate::events::STATUS_CHANGED_EVENT;
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::status::{self, StatusChanged};
use crate::store::settings::{Context, ContextMusic, Feature, Status};

#[derive(Default)]
struct Inner {
    moment: Moment,
    held: Held,
}

#[derive(Default)]
pub struct Automation {
    inner: Mutex<Inner>,
    queue: tokio::sync::Mutex<()>,
}

fn automation(app: &AppHandle) -> &Automation {
    &super::music(app).automation
}

fn has_status_feature(context: Option<&Context>) -> bool {
    context.is_some_and(|context| context.features.contains(&Feature::Status))
}

/// Break and Done for today.
fn is_away(status: Status) -> bool {
    status != Status::Working
}

/// The active context's music settings, as far as its features let them act.
fn context_music(app: &AppHandle) -> ContextMusic {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let context = state.contexts.current(&settings);
    let music = context
        .map(|context| context.music.clone())
        .unwrap_or_default();
    rules::effective(&music, has_status_feature(context))
}

/// Does one step if Music is in the state for it; the text for the log, or `None` when nothing was done.
async fn run(app: &AppHandle, step: &Step) -> AppResult<Option<String>> {
    let now = super::read(app).await?;
    let settings = app.state::<AppState>().settings.get();
    let action = {
        let mut inner = automation(app).inner.lock_unpoisoned();
        rules::action(step, &now, &mut inner.held, &settings.music)
    };
    let Some(action) = action else {
        return Ok(None);
    };
    let text = match &action {
        Action::Pause => {
            super::pause(app).await?;
            "Music paused".to_string()
        }
        Action::Resume => {
            super::resume(app).await?;
            "Music resumed".to_string()
        }
        Action::Volume(volume) => {
            super::set_volume(app, f64::from(*volume)).await?;
            if matches!(step, Step::Restore) {
                format!("Music back to {volume} %")
            } else {
                format!("Music lowered to {volume} %")
            }
        }
        Action::Play(id) => {
            super::play_entry(app, id).await?;
            let name = settings
                .music
                .iter()
                .find(|entry| &entry.id == id)
                .map_or(id.as_str(), entry_label);
            format!("Music started: {name}")
        }
    };
    Ok(Some(text))
}

fn apply(app: &AppHandle, title: &'static str, now: Moment, first: bool) {
    let settings = context_music(app);
    let steps = {
        let mut inner = automation(app).inner.lock_unpoisoned();
        let steps = if first {
            inner.held.paused = false;
            rules::first_working_steps(now, &settings)
        } else {
            rules::steps(inner.moment, now, &inner.held, &settings)
        };
        inner.moment = now;
        steps
    };
    // Nothing is touched before the Music tool is set up.
    if steps.is_empty() || app.state::<AppState>().settings.get().music.is_empty() {
        return;
    }
    let app = app.clone();
    action_log::spawn(async move {
        let _in_order = automation(&app).queue.lock().await;
        for step in steps {
            match run(&app, &step).await {
                Ok(Some(text)) => {
                    log::info!("{title}: {text}");
                    action_log::log_titled(
                        Some(title),
                        ActionTarget::Daybook,
                        &text,
                        ActionState::Done,
                        None,
                    );
                }
                Ok(None) => {}
                Err(e) => log::warn!("{title}: {step:?} failed: {e}"),
            }
        }
    });
}

fn in_call(app: &AppHandle) -> bool {
    calls::active(app).is_some()
}

fn away_now(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    has_status_feature(state.contexts.current(&settings))
        && status::today(app).status.is_some_and(is_away)
}

fn status_changed(app: &AppHandle, change: &StatusChanged) {
    let now = Moment {
        in_call: in_call(app),
        away: is_away(change.next),
    };
    apply(app, "Music", now, change.first);
}

fn call_changed(app: &AppHandle, in_call: bool) {
    let moment = automation(app).inner.lock_unpoisoned().moment;
    if moment.in_call == in_call {
        return;
    }
    let title = if in_call {
        "Call started"
    } else {
        "Call ended"
    };
    apply(app, title, Moment { in_call, ..moment }, false);
}

fn listen_status(app: &AppHandle) {
    let handle = app.clone();
    app.listen(
        STATUS_CHANGED_EVENT,
        move |event| match serde_json::from_str::<StatusChanged>(event.payload()) {
            Ok(change) => status_changed(&handle, &change),
            Err(e) => log::warn!("Couldn't read a status change: {e}"),
        },
    );
}

fn watch_calls(app: &AppHandle) {
    let handle = app.clone();
    let mut calls = calls::subscribe(app);
    tauri::async_runtime::spawn(async move {
        loop {
            match calls.recv().await {
                Ok(Announced { event, trigger }) => {
                    let _entered = trigger.as_ref().map(action_log::Trigger::enter);
                    match event {
                        CallEvent::Started(_) => call_changed(&handle, true),
                        CallEvent::Ended { .. } => call_changed(&handle, false),
                        CallEvent::Changed(_) => {}
                    }
                }
                Err(RecvError::Lagged(_)) => continue,
                Err(RecvError::Closed) => break,
            }
        }
    });
}

pub fn install(app: &AppHandle) {
    automation(app).inner.lock_unpoisoned().moment = Moment {
        in_call: in_call(app),
        away: away_now(app),
    };
    listen_status(app);
    watch_calls(app);
}
