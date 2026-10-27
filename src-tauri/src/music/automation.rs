//! Calls and status changes pause, lower, resume or start the music, as the active context says
//! (`rules`). Steps run one after another, across events.

use std::sync::Mutex;

use tauri::{AppHandle, Listener, Manager};

use super::rules::{self, Action, Held, Lowered, Moment, Step};
use super::state::entry_label;
use crate::action_log::{self, ActionState, ActionTarget};
use crate::calls::{self, CallEvent};
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

/// On Break or Off.
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
    let (action, lowered_before, lowered) = {
        let mut inner = automation(app).inner.lock_unpoisoned();
        let before = inner.held.lowered;
        let action = rules::action(step, &now, &mut inner.held, &settings.music);
        (action, before, inner.held.lowered)
    };
    if lowered != lowered_before {
        remember_lowered(app, lowered);
    }
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

fn remember_lowered(app: &AppHandle, lowered: Option<Lowered>) {
    let saved = app
        .state::<AppState>()
        .music_held
        .update(|held| *held = lowered);
    if let Err(e) = saved {
        log::warn!("Couldn't remember the lowered music volume: {e}");
    }
}

/// Runs `steps` in order after the steps already waiting.
async fn run_steps(app: &AppHandle, title: &str, steps: Vec<Step>) {
    let _in_order = automation(app).queue.lock().await;
    for step in steps {
        match run(app, &step).await {
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
    action_log::spawn(async move { run_steps(&app, title, steps).await });
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

/// A call that moves to another app keeps the music where it is.
fn follow_call(app: &AppHandle, event: CallEvent) {
    match event {
        CallEvent::Started(_) => call_changed(app, true),
        CallEvent::Ended { .. } => call_changed(app, false),
        CallEvent::Changed(_) | CallEvent::Switched { .. } => {}
    }
}

fn has_lowered(app: &AppHandle) -> bool {
    automation(app)
        .inner
        .lock_unpoisoned()
        .held
        .lowered
        .is_some()
}

async fn restore_volume(app: &AppHandle) {
    run_steps(app, "Music", vec![Step::Restore]).await;
}

/// After a restart, once the first call check found no call, a volume a call lowered goes back.
fn restore_after_restart(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        calls::first_check(&app).await;
        if in_call(&app) || !has_lowered(&app) {
            return;
        }
        action_log::traced("Music lowered before the restart", restore_volume(&app)).await;
    });
}

/// Daybook quits: a volume a call lowered goes back.
pub async fn restore_on_exit(app: &AppHandle) {
    if has_lowered(app) {
        log::info!("Quitting during a call: restoring the music volume");
        restore_volume(app).await;
    }
}

pub fn install(app: &AppHandle) {
    let lowered = app.state::<AppState>().music_held.read(|held| *held);
    {
        let mut inner = automation(app).inner.lock_unpoisoned();
        inner.moment = Moment {
            in_call: in_call(app),
            away: away_now(app),
        };
        inner.held.lowered = lowered;
    }
    listen_status(app);
    calls::follow(app, follow_call);
    restore_after_restart(app);
}
