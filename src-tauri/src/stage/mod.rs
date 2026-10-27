//! The Stage hook (docs/stage.md): the presenter app Stage reads Daybook's windows, anchors, commands
//! and state and makes it act, through `/v1/stage/<verb>` on the HTTP API (same port and token) while
//! Settings → Automation → Presentations is on. A discovery file tells Stage where to find it.

mod acts;
pub mod catalog;
pub mod commands;
pub mod discovery;
mod host;
pub mod hub;
mod inbox;
pub mod keys;
pub mod page;
pub mod protocol;
pub mod requests;
pub mod state;
#[cfg(test)]
mod test_host;
mod verbs;
pub mod windows;

use std::path::PathBuf;
use std::sync::Mutex;

use tauri::{AppHandle, Manager};

use crate::automation::{AutomationError, Outcome, Params};
use crate::http_api;
use crate::lock::MutexExt;
use crate::state::AppState;
use host::TauriHost;
use hub::{AppInfo, Hub, Timeouts};
use protocol::{discovery_record, register_command};

#[derive(Default)]
struct Discovery {
    /// Bumped by every publish and removal, so a publish that waited for the token never writes over a
    /// later change.
    generation: u64,
    /// The file as written.
    file: Option<PathBuf>,
}

pub struct Stage {
    hub: Hub<TauriHost>,
    discovery: Mutex<Discovery>,
}

pub fn install(app: &AppHandle) {
    let info = AppInfo {
        name: crate::APP_NAME.into(),
        id: app.config().identifier.clone(),
        version: app.package_info().version.to_string(),
    };
    app.manage(Stage {
        hub: Hub::new(TauriHost(app.clone()), Timeouts::default(), info),
        discovery: Mutex::default(),
    });
}

fn stage(app: &AppHandle) -> &Stage {
    app.state::<Stage>().inner()
}

/// Lets a presenter request in once the token checked out: 404 while Presentations is off. Every
/// request let in counts as the last one, failed ones too.
pub fn admit(app: &AppHandle) -> Result<(), AutomationError> {
    if !app.state::<AppState>().settings.get().api.presentations {
        return Err(AutomationError::new(
            404,
            "Presentations are off (Daybook → Settings → Automation).",
        ));
    }
    stage(app).hub.touch();
    Ok(())
}

pub async fn run(app: &AppHandle, verb: &str, params: &Params) -> Result<Outcome, AutomationError> {
    stage(app).hub.run(verb, params).await
}

/// Writes the discovery file for the server now listening on `port`.
pub fn publish(app: &AppHandle, port: u16) {
    let generation = {
        let mut discovery = stage(app).discovery.lock_unpoisoned();
        discovery.generation += 1;
        discovery.generation
    };
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let token = match http_api::token(&app).await {
            Ok(token) => token,
            Err(e) => return log::warn!("Stage can't find Daybook without the API token: {e}"),
        };
        let Some(home) = std::env::home_dir() else {
            return log::warn!("Stage can't find Daybook without a home folder");
        };
        let stage = stage(&app);
        let info = &stage.hub.app;
        let file = discovery::file_in(&discovery::folder(&home), &info.id);
        let record = discovery_record(
            &info.name,
            &info.id,
            port,
            token.expose(),
            std::process::id(),
            chrono::Utc::now(),
        );
        let mut current = stage.discovery.lock_unpoisoned();
        if current.generation != generation {
            return;
        }
        match discovery::write(&file, &record) {
            Ok(()) => {
                log::info!("Discoverable by Stage: {}", file.display());
                current.file = Some(file);
            }
            Err(e) => log::warn!("Couldn't write {}: {e}", file.display()),
        }
    });
}

/// Removes the discovery file at once (the server stops, Presentations went off, the app quits).
pub fn unpublish(app: &AppHandle) {
    let Some(stage) = app.try_state::<Stage>() else {
        return;
    };
    let mut discovery = stage.discovery.lock_unpoisoned();
    discovery.generation += 1;
    if let Some(file) = discovery.file.take()
        && let Err(e) = discovery::remove(&file)
    {
        log::warn!("Couldn't remove {}: {e}", file.display());
    }
}

/// Settings → Automation → Presentations, part of `AutomationInfo`.
pub struct PresentationsInfo {
    /// The discovery file while Stage can find Daybook.
    pub stage_file: Option<String>,
    /// When a presenter last sent a request, in epoch milliseconds.
    pub last_request: Option<i64>,
    /// `stage register <url> --token <token>`, to paste into a terminal.
    pub register_command: String,
}

/// None until `publish` has written the file.
pub fn stage_file(app: &AppHandle) -> Option<PathBuf> {
    stage(app).discovery.lock_unpoisoned().file.clone()
}

pub fn presentations_info(app: &AppHandle, port: u16, token: &str) -> PresentationsInfo {
    PresentationsInfo {
        stage_file: stage_file(app).map(|f| f.display().to_string()),
        last_request: stage(app).hub.last_request().map(|t| t.timestamp_millis()),
        register_command: register_command(port, token),
    }
}
