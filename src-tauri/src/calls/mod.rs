//! Calls: which app trigger is in a call (docs/presence-and-calls.md). `watch` checks every 15 s while the
//! Mac is awake; `meetings:check` checks now and ends a call that is gone without the grace.

pub mod access;
pub mod checks;
pub mod detect;
mod join;
mod pick;
mod probe;
mod titles;
mod tracker;
mod watch;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tokio::sync::broadcast;
use ts_rs::TS;

pub use join::join;
pub use tracker::{ActiveCall, CallEvent};
pub use watch::{check_now, first_check, start, woke};

use self::detect::Missing;
use self::tracker::Tracker;
use self::watch::check_soon;
use crate::action_log;
use crate::bus::{Command, CommandBus};
use crate::error::{AppError, AppResult};
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::store::settings::{AppRef, MeetingRule, Settings, built_in_call_apps};

pub struct Calls {
    tracker: Mutex<Tracker>,
    watch: watch::Watch,
    /// Whether macOS's Accessibility prompt was shown this run.
    prompted: AtomicBool,
    events: broadcast::Sender<Announced>,
}

/// A call event with its action-log trigger ("Call started: Zoom"), which the followers' actions join.
#[derive(Clone, Debug)]
pub struct Announced {
    pub event: CallEvent,
    pub trigger: Option<action_log::Trigger>,
}

impl Default for Calls {
    fn default() -> Self {
        Self {
            tracker: Mutex::default(),
            watch: watch::Watch::default(),
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

/// Every call start, change, switch and end, for the features that follow calls. Subscribe before
/// `start`, so the first check's events reach every follower.
pub fn subscribe(app: &AppHandle) -> broadcast::Receiver<Announced> {
    calls(app).events.subscribe()
}

/// Runs `on_event` for every call event within its action-log trigger, for as long as the app runs.
/// Install before `start`, like `subscribe`.
pub fn follow(app: &AppHandle, on_event: impl Fn(&AppHandle, CallEvent) + Send + 'static) {
    let handle = app.clone();
    let mut events = subscribe(app);
    tauri::async_runtime::spawn(async move {
        loop {
            match events.recv().await {
                Ok(Announced { event, trigger }) => {
                    let _entered = trigger.as_ref().map(action_log::Trigger::enter);
                    on_event(&handle, event);
                }
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    });
}

/// Detection runs with a vault and the switch in Settings on.
pub fn detecting(settings: &Settings) -> bool {
    settings.meetings.detection && settings.vault.path.is_some()
}

/// Re-checks when a setting detection reads (the switch, the rules, the vault) changes.
pub fn settings_changed(app: &AppHandle, previous: &Settings, saved: &Settings) {
    fn reads(
        s: &Settings,
    ) -> (
        bool,
        &[crate::store::settings::MeetingRule],
        &Option<String>,
    ) {
        (detecting(s), &s.meetings.rules, &s.vault.path)
    }
    if reads(previous) != reads(saved) {
        check_soon(app);
    }
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
    let status = state.watch.status.lock_unpoisoned();
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
                watch::check_asked(&app).await;
                Ok(())
            }),
    );
}
