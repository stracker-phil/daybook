//! The journal (docs/vault.md#journal): question groups from Settings, answered in the journal overlay
//! one group at a time and written as callouts into the note's Journal section.

pub mod progress;
mod rename;

use std::sync::Arc;

use chrono::NaiveDate;
use serde::Serialize;
use serde_json::json;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

pub use progress::{JournalGroupView, JournalProgress, journal_groups, overall};
pub use rename::follow_renamed_groups;

use crate::bus::{ArgChoice, Command, CommandBus};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::store::settings::Settings;
use crate::vault::commands::blocking;
use crate::vault::ops::journal::{JournalAnswer, JournalGroupAnswers};
use crate::vault::ops::{NoteLayout, NoteOp};
use crate::{action_log, window};

const OVERLAY: &str = "journal";
const MAX_TEXT: usize = 20_000;
const MAX_ITEMS: usize = 50;

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct JournalInfo {
    pub groups: Vec<JournalGroupView>,
    /// Whether today's note exists; saving answers waits for it.
    pub note_exists: bool,
}

/// `day`'s groups with their answers (all empty without a note), and whether the note exists.
pub fn read_day(
    app: &AppHandle,
    settings: &Settings,
    day: NaiveDate,
) -> AppResult<(Vec<JournalGroupView>, bool)> {
    let text = app.state::<AppState>().notes.read_note(settings, day)?;
    let heading = NoteLayout::from_settings(settings).journal;
    let groups = journal_groups(text.as_deref(), &settings.journal, &heading);
    Ok((groups, text.is_some()))
}

/// `day`'s progress per group; nothing answered without a vault or when the note can't be read.
pub fn progress_of(app: &AppHandle, settings: &Settings, day: NaiveDate) -> Vec<JournalProgress> {
    let unanswered = || journal_groups(None, &settings.journal, "");
    let groups = if settings.vault.path.is_none() {
        unanswered()
    } else {
        read_day(app, settings, day).map_or_else(
            |e| {
                log::warn!("Couldn't read the journal of {day}: {e}");
                unanswered()
            },
            |(groups, _)| groups,
        )
    };
    groups.iter().map(JournalGroupView::progress).collect()
}

fn too_long(text: &str) -> AppResult<()> {
    if text.chars().count() > MAX_TEXT {
        return Err(AppError::InvalidInput(format!(
            "A journal answer is longer than {MAX_TEXT} characters."
        )));
    }
    Ok(())
}

fn checked_answer(answer: &JournalAnswer) -> AppResult<()> {
    if answer.items.len() > MAX_ITEMS || answer.tasks.len() > MAX_ITEMS {
        return Err(AppError::InvalidInput(format!(
            "A journal answer has more than {MAX_ITEMS} entries."
        )));
    }
    too_long(&answer.text)?;
    answer.items.iter().try_for_each(|item| too_long(item))?;
    answer
        .tasks
        .iter()
        .try_for_each(|task| too_long(&task.text))
}

fn ensure_group(settings: &Settings, group: &str) -> AppResult<()> {
    if settings.journal.iter().any(|config| config.title == group) {
        Ok(())
    } else {
        Err(AppError::NotFound(format!(
            "“{group}” isn't a journal group."
        )))
    }
}

fn checked(settings: &Settings, group: &str, answers: &JournalGroupAnswers) -> AppResult<()> {
    ensure_group(settings, group)?;
    if answers.answers.len() > MAX_ITEMS {
        return Err(AppError::InvalidInput(format!(
            "More than {MAX_ITEMS} journal answers."
        )));
    }
    too_long(&answers.note)?;
    answers.answers.values().try_for_each(checked_answer)
}

#[tauri::command]
pub async fn journal_info(app: AppHandle) -> AppResult<JournalInfo> {
    let today = app.state::<AppState>().notes.today();
    let handle = app.clone();
    let (groups, note_exists) =
        blocking(move || read_day(&handle, &handle.state::<AppState>().settings.get(), today))
            .await?;
    Ok(JournalInfo {
        groups,
        note_exists,
    })
}

/// Rewrites one group's callout in today's note; waits for the note when it doesn't exist yet.
#[tauri::command]
pub async fn journal_save(
    app: AppHandle,
    group: String,
    answers: JournalGroupAnswers,
) -> AppResult<()> {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    checked(&settings, &group, &answers)?;
    let notes = Arc::clone(&state.notes);
    let title = format!("Journal saved: {group}");
    let op = NoteOp::JournalGroup { group, answers };
    action_log::traced_merged(&title, blocking(move || notes.edit_today(&settings, op))).await?;
    Ok(())
}

fn group_choices(app: &AppHandle) -> Vec<ArgChoice> {
    let settings = app.state::<AppState>().settings.get();
    settings
        .journal
        .iter()
        .map(|group| ArgChoice::new(&group.title))
        .collect()
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("journal:open", "Journal…")
            .help("Answer today's journal questions")
            .needs_vault()
            .run(|app| async move {
                window::panel::open(&app, OVERLAY, serde_json::Value::Null).await;
                Ok(())
            }),
    );
    bus.add_family(
        Command::new("journal:open:", "Open a journal group")
            .needs_vault()
            .family("<group>")
            .choices(group_choices)
            .run(|app, group| async move {
                ensure_group(&app.state::<AppState>().settings.get(), &group)?;
                window::panel::open(&app, OVERLAY, json!({ "group": group })).await;
                Ok(())
            }),
    );
}
