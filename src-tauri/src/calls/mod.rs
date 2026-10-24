//! Calls: which app trigger is in a call (docs/presence-and-calls.md). Checked every `TICK` while the
//! owner is at the Mac; `meetings:check` checks now and ends a call that is gone without the grace.

pub mod access;
pub mod checks;
pub mod detect;
mod join;
mod pick;
mod probe;
pub mod tracker;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::Local;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tokio::sync::broadcast;
use ts_rs::TS;

pub use join::{join, open_args};
pub use tracker::{ActiveCall, CallEvent};

use self::detect::{Detected, Detection, Missing, detect};
use self::probe::MacProbe;
use self::tracker::{Seen, Tracker};
use crate::bus::{Command, CommandBus};
use crate::calendar::{self, events::running_event};
use crate::error::{AppError, AppResult};
use crate::events::CALL_CHANGED_EVENT;
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::store::settings::{AppRef, MeetingRule, Settings, built_in_call_apps};
use crate::{presence, tray};

const TICK: Duration = Duration::from_secs(15);

#[derive(Default)]
struct Status {
    /// Permissions a check was refused and hasn't used since.
    missing: Vec<Missing>,
    error: Option<String>,
}

impl Status {
    fn record(&mut self, detection: &Result<Detection, String>) {
        match detection {
            Ok(detection) => {
                self.missing.retain(|m| !detection.granted.contains(m));
                for m in &detection.missing {
                    if !self.missing.contains(m) {
                        self.missing.push(*m);
                    }
                }
                self.error = None;
            }
            Err(e) => {
                if self.error.as_ref() != Some(e) {
                    log::warn!("Checking for a call failed: {e}");
                }
                self.error = Some(e.clone());
            }
        }
    }
}

pub struct Calls {
    tracker: Mutex<Tracker>,
    status: Mutex<Status>,
    probe: MacProbe,
    /// A regular check skips while one runs; an asked-for one waits and runs after it.
    checking: tokio::sync::Mutex<()>,
    prompted: AtomicBool,
    events: broadcast::Sender<CallEvent>,
}

impl Default for Calls {
    fn default() -> Self {
        Self {
            tracker: Mutex::default(),
            status: Mutex::default(),
            probe: MacProbe::default(),
            checking: tokio::sync::Mutex::default(),
            prompted: AtomicBool::new(false),
            events: broadcast::channel(16).0,
        }
    }
}

fn calls(app: &AppHandle) -> &Calls {
    app.state::<Calls>().inner()
}

pub fn active(app: &AppHandle) -> Option<ActiveCall> {
    calls(app).tracker.lock_unpoisoned().active().cloned()
}

/// Every call start, change and end, for the features that follow calls.
pub fn subscribe(app: &AppHandle) -> broadcast::Receiver<CallEvent> {
    calls(app).events.subscribe()
}

/// Detection runs with a vault and the switch in Settings on.
pub fn detecting(settings: &Settings) -> bool {
    settings.meetings.detection && settings.vault.path.is_some()
}

async fn detect_now(app: &AppHandle, rules: Vec<MeetingRule>) -> Result<Detection, String> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || detect(&rules, &calls(&handle).probe))
        .await
        .map_err(|e| e.to_string())
        .and_then(|result| result)
}

/// A call its app calls just "Meeting" takes the title of the calendar event running now.
async fn with_event_title(app: &AppHandle, found: Option<Detected>) -> Option<Detected> {
    let mut found = found?;
    if found.title == checks::GENERIC_TITLE {
        let now = Local::now().naive_local();
        let events = calendar::events_on(app, now.date(), now.date()).await;
        found.title = checks::call_title(
            &found.title,
            running_event(&events, now).map(|e| e.title.as_str()),
        );
    }
    Some(found)
}

async fn check(app: &AppHandle, asked: bool) {
    let state = calls(app);
    let _one_at_a_time = if asked {
        state.checking.lock().await
    } else {
        let Ok(guard) = state.checking.try_lock() else {
            return;
        };
        guard
    };
    let settings = app.state::<AppState>().settings.get();
    let seen = if detecting(&settings) {
        let detection = detect_now(app, settings.meetings.rules).await;
        state.status.lock_unpoisoned().record(&detection);
        let Ok(detection) = detection else {
            return;
        };
        Seen::Call(with_event_title(app, detection.found).await)
    } else {
        Seen::Off
    };
    let now = Local::now().naive_local();
    let changes = state.tracker.lock_unpoisoned().observe(seen, now, asked);
    if !changes.is_empty() {
        announce(app, changes);
    }
}

fn announce(app: &AppHandle, changes: Vec<CallEvent>) {
    for change in changes {
        match &change {
            CallEvent::Started(call) => log::info!("In a {} call: {}", call.app, call.title),
            CallEvent::Changed(call) => log::info!("The {} call is now: {}", call.app, call.title),
            CallEvent::Ended { call, end } => {
                log::info!("The {} call ended at {}", call.app, end.format("%H:%M:%S"))
            }
        }
        // Nobody may be listening yet.
        let _ = calls(app).events.send(change);
    }
    tray::refresh(app);
    if let Err(e) = app.emit(CALL_CHANGED_EVENT, ()) {
        log::warn!("Couldn't announce the call change: {e}");
    }
}

/// A regular check, without waiting for it.
pub fn check_soon(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { check(&app, false).await });
}

/// Checks every `TICK`; while away it only checks whether the owner is back.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticks = tokio::time::interval(TICK);
        loop {
            ticks.tick().await;
            if presence::is_away(&app) {
                presence::verify(&app).await;
            } else {
                check(&app, false).await;
            }
        }
    });
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct CallView {
    pub app: String,
    pub title: String,
    /// `HH:MM` the call was first seen.
    pub since: String,
}

impl From<&ActiveCall> for CallView {
    fn from(call: &ActiveCall) -> Self {
        Self {
            app: call.app.clone(),
            title: call.title.clone(),
            since: call.since.format("%H:%M").to_string(),
        }
    }
}

/// An app trigger Settings can add again, with what its built-in check looks for.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct BuiltInCall {
    pub rule: MeetingRule,
    pub check: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct CallsInfo {
    /// See `detecting`.
    pub detecting: bool,
    pub active: Option<CallView>,
    /// Daybook may read other apps' window titles.
    pub accessibility: bool,
    /// Permissions a check was refused.
    pub missing: Vec<Missing>,
    /// Why the last check failed.
    pub error: Option<String>,
    pub built_ins: Vec<BuiltInCall>,
}

#[tauri::command]
pub fn calls_info(app: AppHandle) -> CallsInfo {
    let settings = app.state::<AppState>().settings.get();
    let state = calls(&app);
    let status = state.status.lock_unpoisoned();
    CallsInfo {
        detecting: detecting(&settings),
        active: active(&app).as_ref().map(CallView::from),
        accessibility: access::is_trusted(),
        missing: status.missing.clone(),
        error: status.error.clone(),
        built_ins: built_in_call_apps()
            .into_iter()
            .filter_map(|rule| {
                let check = checks::built_in_check(&rule.bundle_id)?.to_string();
                Some(BuiltInCall { rule, check })
            })
            .collect(),
    }
}

/// Accessibility: macOS's prompt the first time, then its privacy settings. Automation: its privacy
/// settings (the first script already asked). Returns whether Accessibility is allowed now.
#[tauri::command]
pub fn calls_request_access(app: AppHandle, permission: Missing) -> AppResult<bool> {
    let pane = match permission {
        Missing::Accessibility => {
            if access::is_trusted() {
                return Ok(true);
            }
            if !calls(&app).prompted.swap(true, Ordering::SeqCst) {
                return Ok(access::prompt());
            }
            access::ACCESSIBILITY_SETTINGS
        }
        Missing::Automation => access::AUTOMATION_SETTINGS,
    };
    app.opener()
        .open_url(pane, None::<&str>)
        .map_err(|e| AppError::Io(format!("Couldn't open the privacy settings: {e}")))?;
    Ok(access::is_trusted())
}

/// An app chosen in the file panel (in /Applications), with its bundle id; `None` when cancelled.
#[tauri::command]
pub async fn calls_pick_app(app: AppHandle) -> AppResult<Option<AppRef>> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    app.dialog()
        .file()
        .set_title("Choose an App")
        .set_directory("/Applications")
        .pick_file(move |path| {
            let _ = tx.send(path);
        });
    let chosen = rx
        .await
        .map_err(|_| AppError::Io("The file panel closed without an answer".into()))?;
    let Some(path) = chosen else { return Ok(None) };
    let path = path
        .into_path()
        .map_err(|e| AppError::Io(format!("The chosen app is not a file path: {e}")))?;
    tauri::async_runtime::spawn_blocking(move || pick::app_ref(&path))
        .await
        .map_err(|e| AppError::Io(e.to_string()))?
        .map(Some)
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("meetings:check", "Check for a Call Now")
            .help("Check for a call now; a call that is gone ends at once")
            .run(|app| async move {
                check(&app, true).await;
                Ok(())
            }),
    );
}
