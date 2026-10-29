//! What the timer and calls tell the owner when TimeTracker didn't do as asked, and the context they read.

use tauri::{AppHandle, Manager};

use crate::error::{AppError, ErrorKind};
use crate::state::AppState;
use crate::store::settings::{Feature, Project, Settings};
use crate::{notify, projects};

const NOT_RUNNING: &str = "TimeTracker isn't running";
const REFUSED: &str = "TimeTracker refused the timer";
pub const NOT_STARTED: &str = "The timer wasn't started.";
pub const NOT_STOPPED: &str = "The timer wasn't stopped.";
const OPEN: (&str, &str) = ("timetracker:show", "Open TimeTracker");

/// The current context's projects and whether it has the Status feature.
pub fn context<'a>(app: &AppHandle, settings: &'a Settings) -> (Vec<&'a Project>, bool) {
    let state = app.state::<AppState>();
    let context = state.contexts.current(settings);
    let offered = projects::current_projects(&settings.projects, context.map(|c| c.id.as_str()));
    let status_feature = context.is_some_and(|c| c.features.contains(&Feature::Status));
    (offered, status_feature)
}

/// The notification with "Open TimeTracker".
pub fn not_running(app: &AppHandle, body: &str) {
    log::info!("{body} {NOT_RUNNING}");
    notify::show_with_action(app, NOT_RUNNING, body, OPEN);
}

/// Not running: a notification with "Open TimeTracker"; missing: only the log.
pub fn timer_failed(app: &AppHandle, error: &AppError, body: &str) {
    match error.kind() {
        ErrorKind::TimetrackerMissing => log::info!("{body} TimeTracker isn't installed"),
        ErrorKind::TimetrackerNotRunning => not_running(app, body),
        _ => {
            log::info!("{REFUSED}: {error}");
            notify::show(app, REFUSED, &error.to_string());
        }
    }
}
