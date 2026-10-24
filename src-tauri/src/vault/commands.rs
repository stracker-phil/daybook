//! The vault's Tauri commands and its events to the page.

use std::path::Path;
use std::sync::Arc;

use chrono::NaiveDate;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_opener::OpenerExt;
use ts_rs::TS;

use super::{DailyNoteState, DailyNotes, NoteEvent, VaultInfo, obsidian};
use crate::error::{AppError, AppResult};
use crate::events::NOTE_CHANGED_EVENT;
use crate::state::AppState;
use crate::store::settings::Settings;

/// The payload of `note_changed`.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct NoteChanged {
    /// The day whose note changed (`YYYY-MM-DD`); `None` for another note in the daily-notes folder.
    pub day: Option<String>,
}

fn iso(day: NaiveDate) -> String {
    day.format("%Y-%m-%d").to_string()
}

fn parse_day(day: Option<&str>, notes: &DailyNotes) -> AppResult<NaiveDate> {
    match day {
        None => Ok(notes.today()),
        Some(day) => NaiveDate::parse_from_str(day, "%Y-%m-%d")
            .map_err(|_| AppError::InvalidInput(format!("Not a day: {day}"))),
    }
}

/// The watcher reports the paths the system knows (`/private/var/…` for `/var/…`).
fn same_file(a: &Path, b: &Path) -> bool {
    a == b
        || a.canonicalize()
            .ok()
            .zip(b.canonicalize().ok())
            .is_some_and(|(a, b)| a == b)
}

/// Forwards the vault's events to the page, writes changes that waited since the last session and
/// starts watching today's folder.
pub fn install(app: &AppHandle) {
    let state = app.state::<AppState>();
    let handle = app.clone();
    state
        .notes
        .set_listener(move |event| on_note_event(&handle, event));
    let notes = Arc::clone(&state.notes);
    let settings = state.settings.get();
    notes.watch_today_folder(&settings);
    tauri::async_runtime::spawn_blocking(move || {
        let drained = notes.drain(&settings);
        if drained.written > 0 {
            log::info!(
                "Wrote {} changes that waited since the last session",
                drained.written
            );
        }
    });
}

/// Today, unless only another day's note changed.
fn changed_day(event: &NoteEvent, notes: &DailyNotes, settings: &Settings) -> Option<NaiveDate> {
    let today = notes.today();
    let NoteEvent::Changed(paths) = event else {
        return Some(today);
    };
    let today_path = notes.today_path(settings).ok()?;
    paths
        .iter()
        .any(|path| same_file(path, &today_path))
        .then_some(today)
}

fn on_note_event(app: &AppHandle, event: NoteEvent) {
    let state = app.state::<AppState>();
    let notes = Arc::clone(&state.notes);
    let settings = state.settings.get();
    let day = changed_day(&event, &notes, &settings);
    if let Err(e) = app.emit(NOTE_CHANGED_EVENT, NoteChanged { day: day.map(iso) }) {
        log::warn!("Couldn't announce a note change: {e}");
    }
    crate::tray::refresh(app);
    if matches!(event, NoteEvent::Changed(_)) {
        tauri::async_runtime::spawn_blocking(move || notes.note_appeared(&settings));
    }
}

#[tauri::command]
pub async fn vault_info(state: State<'_, AppState>) -> AppResult<VaultInfo> {
    let notes = Arc::clone(&state.notes);
    let settings = state.settings.get();
    blocking(move || Ok(notes.info(&settings))).await
}

#[tauri::command]
pub fn daily_note_state(state: State<'_, AppState>) -> DailyNoteState {
    state.notes.state(&state.settings.get())
}

fn obsidian_url(settings: &Settings, file: &Path) -> AppResult<String> {
    let vault = super::vault(settings)?;
    obsidian::open_url(&settings.vault.name, &vault, file, None)
        .ok_or_else(|| AppError::InvalidInput(format!("{} is outside the vault", file.display())))
}

/// Opens `day`'s note (default today) in Obsidian; today's is created when it doesn't exist yet.
#[tauri::command]
pub async fn daily_note_open(app: AppHandle, day: Option<String>) -> AppResult<()> {
    open_day(&app, day.as_deref()).await
}

pub async fn open_day(app: &AppHandle, day: Option<&str>) -> AppResult<()> {
    let state = app.state::<AppState>();
    let notes = Arc::clone(&state.notes);
    let settings = state.settings.get();
    let day = parse_day(day, &notes)?;
    let url = blocking(move || {
        let Some(file) = notes.find_note(&settings, day)? else {
            if day != notes.today() {
                return Err(AppError::NotFound(format!(
                    "No daily note for {}",
                    iso(day)
                )));
            }
            notes.create_today(&settings)?;
            return Ok(None);
        };
        obsidian_url(&settings, &file).map(Some)
    })
    .await?;
    let Some(url) = url else {
        return Ok(());
    };
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Io(e.to_string()))
}

/// Creates today's note and reports whether it exists afterwards.
#[tauri::command]
pub async fn daily_note_create(app: AppHandle) -> AppResult<bool> {
    create_today(&app).await
}

pub async fn create_today(app: &AppHandle) -> AppResult<bool> {
    let state = app.state::<AppState>();
    let notes = Arc::clone(&state.notes);
    let settings = state.settings.get();
    blocking(move || notes.create_today(&settings)).await
}

/// Runs vault work that waits on the disk or on Obsidian off the async workers.
pub async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|e| AppError::Io(format!("The vault task failed: {e}")))?
}
