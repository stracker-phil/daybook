//! Open todos and the days with a daily note (docs/vault.md#open-todos), from an index over the daily-notes
//! folder that the vault's watcher keeps up to date.

mod groups;
mod index;
pub mod scan;

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use tauri::{AppHandle, Manager};

pub use groups::{OpenTodo, TodoGroup, group_by_date};
pub use index::{DatedNote, NotesIndex};

use crate::action_log;
use crate::error::{AppError, AppResult};
use crate::locale::{self, DateLocale};
use crate::state::AppState;
use crate::store::settings::{DailySection, Settings};
use crate::vault::commands::{blocking, open_target};
use crate::vault::location::DailyNoteFormat;
use crate::vault::ops::NoteOp;
use scan::todo_key;

/// The vault and its daily-notes folder as the watcher reports them (`/private/var/…` for `/var/…`).
struct Folders {
    vault: PathBuf,
    folder: PathBuf,
    format: DailyNoteFormat,
}

fn folders(app: &AppHandle, settings: &Settings) -> AppResult<Folders> {
    let (vault, format) = app.state::<AppState>().notes.vault_and_format(settings)?;
    let vault = vault.canonicalize().unwrap_or(vault);
    Ok(Folders {
        folder: format.folder_in(&vault),
        vault,
        format,
    })
}

fn routine_step_keys(settings: &Settings) -> HashSet<String> {
    settings
        .routines
        .iter()
        .flat_map(|routine| routine.steps.iter().map(|step| todo_key(&step.title)))
        .collect()
}

fn relative(vault: &Path, file: &Path) -> String {
    file.strip_prefix(vault)
        .unwrap_or(file)
        .to_string_lossy()
        .into_owned()
}

fn detail(date: NaiveDate, section: &str, today: NaiveDate, locale: DateLocale) -> String {
    let day = locale.short_date(date, today);
    if section.is_empty() {
        day
    } else {
        format!("{day} · {section}")
    }
}

/// Every open todo in the daily notes, grouped, newest first. Routine items aren't todos (the wizard ticks
/// them), and neither is the focus list. Nothing without a vault.
pub fn open_todos(app: &AppHandle, settings: &Settings, today: NaiveDate) -> Vec<TodoGroup> {
    let Ok(Folders { vault, folder, .. }) = folders(app, settings) else {
        return Vec::new();
    };
    let ignore = routine_step_keys(settings);
    let skip = settings.daily_note.heading(DailySection::Focus);
    let locale = locale::system();
    let todos = app
        .state::<AppState>()
        .todos
        .open_todos(&folder, &skip, &ignore)
        .into_iter()
        .map(|(note, todo)| {
            let open = OpenTodo {
                detail: detail(note.date, &todo.section, today, locale),
                text: todo.text,
                date: note.date.format("%Y-%m-%d").to_string(),
                file: relative(&vault, &note.file),
                line: u32::try_from(todo.line).unwrap_or(u32::MAX),
            };
            (note.date, open)
        })
        .collect();
    group_by_date(todos, today, locale)
}

/// The days whose daily note exists: a dated note that isn't that day's daily note doesn't count.
pub fn note_days(app: &AppHandle, settings: &Settings) -> Vec<NaiveDate> {
    let Ok(Folders {
        vault,
        folder,
        format,
    }) = folders(app, settings)
    else {
        return Vec::new();
    };
    let is_daily_note = |note: &DatedNote| {
        note.file == format.note_path(&vault, note.date)
            || note.file == format.archived_path(&vault, note.date)
    };
    app.state::<AppState>()
        .todos
        .dated_notes(&folder)
        .into_iter()
        .filter(is_daily_note)
        .map(|note| note.date)
        .collect()
}

/// Ticks a todo: `[x]` into `line` of its daily note (`file`, relative to the vault), refused when that line no
/// longer holds the open `text`.
#[tauri::command]
pub async fn todo_complete(app: AppHandle, file: String, line: u32, text: String) -> AppResult<()> {
    action_log::traced("Todo ticked", async move {
        blocking(move || {
            let state = app.state::<AppState>();
            let settings = state.settings.get();
            let Folders { vault, folder, .. } = folders(&app, &settings)?;
            let path = vault.join(&file);
            if !path.starts_with(&folder) || path.extension().is_none_or(|ext| ext != "md") {
                return Err(AppError::InvalidInput(format!(
                    "{file} isn't a daily note."
                )));
            }
            state
                .notes
                .edit_file(&settings, &path, &NoteOp::TodoDone { line, text })?;
            state.todos.changed(&[path]);
            Ok(())
        })
        .await
    })
    .await
}

/// Opens a todo's note in Obsidian; `file` is relative to the vault.
#[tauri::command]
pub fn todo_open(app: AppHandle, file: String) -> AppResult<()> {
    open_target(&app, &file)
}
