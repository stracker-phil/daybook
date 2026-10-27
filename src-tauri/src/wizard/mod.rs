//! The wizard (docs/routines.md#the-wizard): opens a routine's checklist or a journal group by itself when
//! it is due or on an event, never while away, never over another overlay (it waits) and never twice a day
//! for the same once-a-day trigger.

pub mod prompts;

use std::time::Duration;

use chrono::{Local, NaiveDateTime};
use serde_json::json;
use tauri::{AppHandle, Listener, Manager};
use tokio::sync::broadcast::error::RecvError;

use prompts::{Prompt, Reason, Target, Waiting, first_to_open};

use crate::calls::{self, Announced, CallEvent};
use crate::events::{CONTEXT_CHANGED_EVENT, STATUS_CHANGED_EVENT};
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::status::StatusChanged;
use crate::store::settings::{OpenEvent, Settings, Status};
use crate::vault::commands::blocking;
use crate::{journal, presence, routines, window};

/// Done for today is picked in the overlay; what it opens waits until that has closed.
const AFTER_OVERLAY: Duration = Duration::from_millis(600);

#[derive(Default)]
pub struct Wizard {
    waiting: std::sync::Mutex<Waiting>,
    /// One opening at a time, so a tick and an event never open two things.
    opening: tokio::sync::Mutex<()>,
}

fn wizard(app: &AppHandle) -> &Wizard {
    app.state::<Wizard>().inner()
}

fn overlay_busy(app: &AppHandle) -> bool {
    window::panel::view(app).is_some()
}

/// The current context's routines first, then the journal groups; each in Settings order.
fn prompts(app: &AppHandle, settings: &Settings) -> Vec<Prompt> {
    let context = app.state::<AppState>().contexts.current(settings);
    let routines = routines::active_routines(&settings.routines, context)
        .into_iter()
        .map(|routine| Prompt {
            target: Target::Routine(routine.id.clone()),
            timing: routine.timing.clone(),
        });
    let journal = settings.journal.iter().map(|group| Prompt {
        target: Target::Journal(group.title.clone()),
        timing: group.timing.clone(),
    });
    routines.chain(journal).collect()
}

/// A routine with unticked items, a journal group with nothing written yet.
fn pending(app: &AppHandle, settings: &Settings, target: &Target) -> bool {
    let result = match target {
        Target::Routine(id) => {
            routines::today_view(app, settings, id).map(|view| view.has_open_steps())
        }
        Target::Journal(title) => {
            let today = app.state::<AppState>().notes.today();
            journal::read_day(app, settings, today).map(|(groups, _)| {
                groups
                    .iter()
                    .any(|group| &group.title == title && group.is_unwritten())
            })
        }
    };
    result.unwrap_or_else(|e| {
        log::warn!("Couldn't tell whether {target:?} is still open: {e}");
        false
    })
}

/// A store that can't remember counts as fired, so a broken file never opens the same prompt every tick.
fn mark(app: &AppHandle, key: &str) -> bool {
    let state = app.state::<AppState>();
    state
        .fired
        .mark(key, state.notes.today())
        .unwrap_or_else(|e| {
            log::warn!("Couldn't remember that {key} opened: {e}");
            false
        })
}

async fn choose(app: &AppHandle, reason: Reason, now: NaiveDateTime) -> Option<Target> {
    let handle = app.clone();
    blocking(move || {
        let settings = handle.state::<AppState>().settings.get();
        let prompts = prompts(&handle, &settings);
        Ok(first_to_open(
            &prompts,
            reason,
            now,
            |key| mark(&handle, key),
            |target| pending(&handle, &settings, target),
        ))
    })
    .await
    .unwrap_or_else(|e| {
        log::warn!("Choosing what the wizard opens failed: {e}");
        None
    })
}

async fn show(app: &AppHandle, target: Target) {
    log::info!("The wizard opens {target:?}");
    match target {
        Target::Routine(id) => {
            if let Err(e) = routines::open(app, &id).await {
                log::warn!("Couldn't open the routine {id}: {e}");
            }
        }
        Target::Journal(group) => {
            window::panel::open(app, "journal", json!({ "group": group })).await;
        }
    }
}

pub async fn fire(app: &AppHandle, reason: Reason, now: NaiveDateTime) {
    let _one_at_a_time = wizard(app).opening.lock().await;
    if overlay_busy(app) {
        return;
    }
    if let Some(target) = choose(app, reason, now).await {
        show(app, target).await;
    }
}

fn has_vault(app: &AppHandle) -> bool {
    app.state::<AppState>().settings.get().vault.path.is_some()
}

/// Tries the waiting events, then the due times. Runs on the scheduler's tick.
pub async fn tick(app: &AppHandle) {
    if !has_vault(app) {
        return;
    }
    loop {
        let next = {
            let away = presence::is_away(app);
            wizard(app)
                .waiting
                .lock_unpoisoned()
                .next(away, overlay_busy(app))
        };
        let Some(event) = next else { break };
        fire(app, Reason::Event(event), Local::now().naive_local()).await;
    }
    if !presence::is_away(app) && !overlay_busy(app) {
        fire(app, Reason::Due, Local::now().naive_local()).await;
    }
}

/// `event` happened: it waits for the overlay that caused it to close, then gets its chance.
fn happened(app: &AppHandle, event: OpenEvent) {
    if !has_vault(app) {
        return;
    }
    wizard(app).waiting.lock_unpoisoned().push(event);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(AFTER_OVERLAY).await;
        tick(&app).await;
    });
}

fn status_changed(app: &AppHandle, change: &StatusChanged) {
    if change.next == Status::Working && change.previous != Some(Status::Working) {
        happened(app, OpenEvent::Start);
    }
    if change.next == Status::Off {
        happened(app, OpenEvent::Off);
    }
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

fn listen_context(app: &AppHandle) {
    let handle = app.clone();
    app.listen(CONTEXT_CHANGED_EVENT, move |_| {
        happened(&handle, OpenEvent::Context);
    });
}

fn watch_calls(app: &AppHandle) {
    let handle = app.clone();
    let mut calls = calls::subscribe(app);
    tauri::async_runtime::spawn(async move {
        loop {
            match calls
                .recv()
                .await
                .map(|announced: Announced| announced.event)
            {
                Ok(CallEvent::Started(_)) => happened(&handle, OpenEvent::MeetingStart),
                Ok(CallEvent::Ended { .. }) => happened(&handle, OpenEvent::MeetingEnd),
                // A call moving to another app is still the same meeting.
                Ok(CallEvent::Changed(_) | CallEvent::Switched { .. })
                | Err(RecvError::Lagged(_)) => {
                    continue;
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
}

/// Follows the status, context and call changes that can open something.
pub fn install(app: &AppHandle) {
    listen_status(app);
    listen_context(app);
    watch_calls(app);
}
