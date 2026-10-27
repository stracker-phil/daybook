//! Open Jira Issue: a number, key or link opens the issue in the browser; in a context with the Jira
//! feature the key is remembered for the suggestions and the Jira tool (`tickets`).

pub mod key;
pub mod tickets;

use serde_json::Value;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use crate::action_log::{self, ActionTarget};
use crate::bus::{Command, CommandBus};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::store::settings::Feature;
use crate::vault::commands::blocking;
use crate::{tray, window};

pub use key::checked_settings;
use key::issue_link;

const VIEW: &str = "jira";

fn jira_feature_on(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    state
        .contexts
        .current(&settings)
        .is_some_and(|context| context.features.contains(&Feature::Jira))
}

/// Remembers `key` for today when the context has the Jira feature.
async fn remember(app: &AppHandle, key: &str) -> AppResult<()> {
    if !jira_feature_on(app) {
        return Ok(());
    }
    let handle = app.clone();
    let key = key.to_string();
    blocking(move || {
        let state = handle.state::<AppState>();
        state.jira.record(&key, state.notes.today())
    })
    .await?;
    tray::refresh(app);
    Ok(())
}

/// Opens the issue `input` names; the overlay closes first when `close_overlay`. Returns its key.
async fn open_issue(app: &AppHandle, input: &str, close_overlay: bool) -> AppResult<String> {
    let jira = app.state::<AppState>().settings.get().jira;
    let (key, url) = issue_link(&jira, input)?;
    if close_overlay {
        window::panel::close(app, true);
    }
    let title = format!("Jira issue opened: {key}");
    action_log::traced(&title, async {
        app.opener()
            .open_url(&url, None::<&str>)
            .map_err(|e| AppError::Io(format!("Couldn't open {key}: {e}")))?;
        action_log::done(
            ActionTarget::Daybook,
            &format!("Opened {url} in the browser"),
        );
        remember(app, &key).await
    })
    .await?;
    Ok(key)
}

/// Today's keys with the Jira feature, newest first; none without it.
pub fn today_tickets(app: &AppHandle) -> Vec<String> {
    if !jira_feature_on(app) {
        return Vec::new();
    }
    let state = app.state::<AppState>();
    state.jira.on(state.notes.today())
}

/// Opens the issue typed into the `jira` overlay and closes it; returns the key.
#[tauri::command]
pub async fn jira_open(app: AppHandle, input: String) -> AppResult<String> {
    open_issue(&app, &input, true).await
}

/// The overlay's suggestions: the keys of the last seven days, today's first, newest first.
#[tauri::command]
pub fn jira_suggestions(app: AppHandle) -> Vec<String> {
    let state = app.state::<AppState>();
    state
        .jira
        .recent(state.notes.today(), tickets::SUGGESTED_DAYS)
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("jira:open", "Open Jira Issue…").run(|app| async move {
            window::panel::open(&app, VIEW, Value::Null).await;
            Ok(())
        }),
    );
    bus.add_family(
        Command::new("jira:issue:", "Open a Jira issue")
            .family("<key>")
            .run(|app, key| async move { open_issue(&app, &key, false).await.map(|_| ()) }),
    );
}
