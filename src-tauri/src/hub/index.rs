//! What the daily-notes index adds to the hub: the days ‹ › step to, the shown day's note topics and the
//! open todos.

use chrono::NaiveDate;
use tauri::{AppHandle, Manager};

use super::adjacent_days;
use crate::error::AppResult;
use crate::state::AppState;
use crate::store::settings::Settings;
use crate::todos::{self, TodoGroup};
use crate::vault::commands::blocking;
use crate::vault::note::Note;
use crate::vault::ops::NoteLayout;
use crate::vault::ops::notes::{NoteSectionRef, list_note_sections};

fn note_topics(
    app: &AppHandle,
    settings: &Settings,
    day: NaiveDate,
) -> AppResult<Vec<NoteSectionRef>> {
    let text = app.state::<AppState>().notes.read_note(settings, day)?;
    let lines = text
        .map(|text| Note::parse(&text).lines)
        .unwrap_or_default();
    Ok(list_note_sections(
        &lines,
        &NoteLayout::from_settings(settings).notes,
    ))
}

pub fn todo_dates(groups: &[TodoGroup]) -> Vec<NaiveDate> {
    groups
        .iter()
        .flat_map(|group| &group.todos)
        .filter_map(|todo| NaiveDate::parse_from_str(&todo.date, "%Y-%m-%d").ok())
        .collect()
}

/// The note days next to the shown one, the shown day's note topics and every open todo.
#[derive(Default)]
pub struct IndexView {
    pub previous: Option<NaiveDate>,
    pub next: Option<NaiveDate>,
    pub notes: Vec<NoteSectionRef>,
    pub todos: Vec<TodoGroup>,
}

pub async fn read_index(
    app: &AppHandle,
    settings: &Settings,
    day: NaiveDate,
    today: NaiveDate,
) -> IndexView {
    if settings.vault.path.is_none() {
        return IndexView::default();
    }
    let handle = app.clone();
    let reading = settings.clone();
    blocking(move || {
        let (previous, next) = adjacent_days(&todos::note_days(&handle, &reading), day, today);
        Ok(IndexView {
            previous,
            next,
            notes: note_topics(&handle, &reading, day)?,
            todos: todos::open_todos(&handle, &reading, today),
        })
    })
    .await
    .unwrap_or_else(|e| {
        log::warn!("Couldn't read the daily notes for {day}: {e}");
        IndexView::default()
    })
}
