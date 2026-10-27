//! The day's focus list (docs/slack.md#the-days-message): today's note holds it as typed; the focus
//! overlay edits it, ⌘S puts it into the day's message in Slack, and going Off posts the end-of-day list
//! as a reply.

pub mod board;

use std::sync::Arc;

use chrono::NaiveDate;
use serde::Serialize;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

pub use board::{FocusLine, focus_lines};

use crate::bus::{Command, CommandBus};
use crate::error::{AppError, AppResult};
use crate::slack::day::{focus_channel, focus_target, post_day_reply, update_day_message};
use crate::slack::day_message::{FocusTarget, paragraphs};
use crate::state::AppState;
use crate::store::settings::Settings;
use crate::vault::commands::blocking;
use crate::vault::note::Note;
use crate::vault::ops::focus::{focus_to_slack, read_focus};
use crate::vault::ops::{NoteLayout, NoteOp};
use crate::{action_log, window};

const OVERLAY: &str = "focus";
const MAX_LIST_LENGTH: usize = 8000;

/// A project the overlay's ⌘1…9 insert.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct FocusProject {
    pub name: String,
    /// A shortcode, `:nexo:`; empty when the project has none.
    pub emoji: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct FocusInfo {
    /// The list as today's note holds it.
    pub body: String,
    /// The current projects, in Settings order.
    pub projects: Vec<FocusProject>,
    pub slack: FocusTarget,
    /// Where the list goes in Slack; empty when it doesn't.
    pub channel: String,
}

/// `day`'s focus list; empty without a note or a list.
pub fn read_day(app: &AppHandle, settings: &Settings, day: NaiveDate) -> AppResult<String> {
    let text = app.state::<AppState>().notes.read_note(settings, day)?;
    Ok(text.map_or_else(String::new, |text| {
        let heading = NoteLayout::from_settings(settings).focus;
        read_focus(&Note::parse(&text).lines, &heading)
    }))
}

fn already_sent(app: &AppHandle) -> bool {
    focus_target(app) == FocusTarget::Sent
}

fn checked(body: &str) -> AppResult<()> {
    if body.chars().count() > MAX_LIST_LENGTH {
        return Err(AppError::InvalidInput(format!(
            "The focus list is longer than {MAX_LIST_LENGTH} characters."
        )));
    }
    Ok(())
}

async fn write(app: &AppHandle, body: &str) -> AppResult<()> {
    checked(body)?;
    let state = app.state::<AppState>();
    let notes = Arc::clone(&state.notes);
    let settings = state.settings.get();
    let op = NoteOp::Focus { body: body.into() };
    blocking(move || notes.edit_today(&settings, op)).await?;
    Ok(())
}

/// Fails when today's note can't be read, so an empty list never replaces the note's.
#[tauri::command]
pub async fn focus_info(app: AppHandle) -> AppResult<FocusInfo> {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let projects = state
        .contexts
        .current_projects(&settings)
        .into_iter()
        .map(|project| FocusProject {
            name: project.name.clone(),
            emoji: project.slack_emoji.clone(),
        })
        .collect();
    let today = state.notes.today();
    let handle = app.clone();
    let body =
        blocking(move || read_day(&handle, &handle.state::<AppState>().settings.get(), today))
            .await?;
    Ok(FocusInfo {
        body,
        projects,
        slack: focus_target(&app),
        channel: focus_channel(&app),
    })
}

/// Writes the list to today's note as typed and closes the overlay; Slack hears of it only on ⌘S.
#[tauri::command]
pub async fn focus_save(app: AppHandle, body: String) -> AppResult<()> {
    action_log::traced("Focus saved", write(&app, &body)).await?;
    if window::panel::view(&app).as_deref() == Some(OVERLAY) {
        window::panel::close(&app, false);
    }
    Ok(())
}

/// Puts the list as typed into the day's message in Slack, without writing the note.
#[tauri::command]
pub async fn focus_send(app: AppHandle, body: String) -> AppResult<()> {
    checked(&body)?;
    if already_sent(&app) {
        return Err(AppError::InvalidInput(
            "The end-of-day list already went out today.".into(),
        ));
    }
    let list = focus_to_slack(&body);
    action_log::traced("Focus sent to Slack", update_day_message(&app, &list)).await
}

/// Writes the list to today's note and posts one reply under the day's message: the end-of-day title,
/// the list, then `closing` (the Off message). `true`: the closing went out with it.
pub async fn send_end_of_day_list(app: &AppHandle, body: &str, closing: &str) -> AppResult<bool> {
    write(app, body).await?;
    let list = focus_to_slack(body);
    if list.is_empty() || focus_channel(app).is_empty() {
        return Ok(false);
    }
    // A retry after a failed status change doesn't post the reply again.
    if already_sent(app) {
        return Ok(true);
    }
    let title = app
        .state::<AppState>()
        .settings
        .get()
        .focus
        .end_of_day_title;
    post_day_reply(app, &paragraphs(&[&title, &list, closing])).await
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("focus:open", "Today's Focus…")
            .help("Edit today's focus list")
            .needs_vault()
            .run(|app| async move {
                window::panel::open(&app, OVERLAY, serde_json::Value::Null).await;
                Ok(())
            }),
    );
}
