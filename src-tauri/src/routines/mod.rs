//! Routines (docs/routines.md): named checklists from Settings, ticked in the wizard overlay and written
//! as `- [ ]` / `- [x]` items into the note's plan section.

mod fired;
pub mod progress;

use std::sync::Arc;

use chrono::{Datelike, NaiveDate};
use serde_json::json;
use tauri::{AppHandle, Manager};

pub use fired::Fired;
pub use progress::{RoutineProgress, RoutineView, active_routines, routine_view};

use crate::bus::{ArgChoice, Command, CommandBus};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::store::settings::{Routine, Settings};
use crate::vault::commands::{blocking, open_target};
use crate::vault::note::Note;
use crate::vault::ops::{NoteLayout, NoteOp};
use crate::{action_log, window};

const OVERLAY: &str = "wizard";

fn weekday(day: NaiveDate) -> u8 {
    u8::try_from(day.weekday().num_days_from_sunday()).unwrap_or(0)
}

fn find<'a>(settings: &'a Settings, id: &str) -> AppResult<&'a Routine> {
    settings
        .routines
        .iter()
        .find(|routine| routine.id == id)
        .ok_or_else(|| AppError::NotFound("This routine doesn't exist any more.".into()))
}

fn step_title<'a>(settings: &'a Settings, id: &str, index: u32) -> AppResult<&'a str> {
    let steps = &find(settings, id)?.steps;
    usize::try_from(index)
        .ok()
        .and_then(|index| steps.get(index))
        .map(|step| step.title.as_str())
        .ok_or_else(|| AppError::NotFound("This item doesn't exist any more.".into()))
}

/// `day`'s note as lines; none without the note.
fn read_lines(app: &AppHandle, settings: &Settings, day: NaiveDate) -> AppResult<Vec<String>> {
    let text = app.state::<AppState>().notes.read_note(settings, day)?;
    Ok(text
        .map(|text| Note::parse(&text).lines)
        .unwrap_or_default())
}

/// The current context's routines on `day`; nothing ticked without a vault or when the note can't be read.
pub fn views_of(app: &AppHandle, settings: &Settings, day: NaiveDate) -> Vec<RoutineView> {
    let context = app.state::<AppState>().contexts.current(settings);
    let routines = active_routines(&settings.routines, context);
    if routines.is_empty() {
        return Vec::new();
    }
    let lines = if settings.vault.path.is_none() {
        Vec::new()
    } else {
        read_lines(app, settings, day).unwrap_or_else(|e| {
            log::warn!("Couldn't read the routines of {day}: {e}");
            Vec::new()
        })
    };
    let heading = NoteLayout::from_settings(settings).plan;
    routines
        .into_iter()
        .map(|routine| routine_view(routine, &lines, &heading, weekday(day)))
        .collect()
}

pub fn progress_of(app: &AppHandle, settings: &Settings, day: NaiveDate) -> Vec<RoutineProgress> {
    views_of(app, settings, day)
        .iter()
        .map(RoutineView::progress)
        .collect()
}

/// `id` as it stands in today's note.
pub fn today_view(app: &AppHandle, settings: &Settings, id: &str) -> AppResult<RoutineView> {
    let day = app.state::<AppState>().notes.today();
    let lines = read_lines(app, settings, day)?;
    let heading = NoteLayout::from_settings(settings).plan;
    Ok(routine_view(
        find(settings, id)?,
        &lines,
        &heading,
        weekday(day),
    ))
}

async fn edit_today(app: &AppHandle, op: NoteOp) -> AppResult<()> {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let notes = Arc::clone(&state.notes);
    blocking(move || notes.edit_today(&settings, op)).await?;
    Ok(())
}

/// Adds today's items to the note, unticked where they're missing, and shows the routine in the wizard.
pub async fn open(app: &AppHandle, id: &str) -> AppResult<()> {
    let settings = app.state::<AppState>().settings.get();
    let routine = find(&settings, id)?;
    let title = format!("Routine opened: {}", routine.name);
    action_log::traced(&title, open_routine(app, id, routine)).await
}

async fn open_routine(app: &AppHandle, id: &str, routine: &Routine) -> AppResult<()> {
    let titles: Vec<String> = routine
        .steps_on(weekday(app.state::<AppState>().notes.today()))
        .into_iter()
        .map(|(_, step)| step.title.clone())
        .collect();
    if !titles.is_empty() {
        edit_today(app, NoteOp::PlanList { titles }).await?;
    }
    window::panel::open(app, OVERLAY, json!({ "routine": id })).await;
    Ok(())
}

#[tauri::command]
pub async fn routine_view_today(app: AppHandle, id: String) -> AppResult<RoutineView> {
    let handle = app.clone();
    blocking(move || {
        let settings = handle.state::<AppState>().settings.get();
        today_view(&handle, &settings, &id)
    })
    .await
}

/// Ticks (`done`) or unticks item `index` of routine `id` in today's note.
#[tauri::command]
pub async fn routine_step(app: AppHandle, id: String, index: u32, done: bool) -> AppResult<()> {
    let settings = app.state::<AppState>().settings.get();
    let title = step_title(&settings, &id, index)?.to_string();
    let ticked = if done { "checked" } else { "unchecked" };
    let trigger = format!("Checklist item {ticked}: {title}");
    action_log::traced(&trigger, edit_today(&app, NoteOp::PlanStep { title, done })).await
}

/// Opens an item's link: a URL, or a vault path in Obsidian.
#[tauri::command]
pub fn routine_open_link(app: AppHandle, link: String) -> AppResult<()> {
    open_target(&app, &link)
}

fn routine_choices(app: &AppHandle) -> Vec<ArgChoice> {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    active_routines(&settings.routines, state.contexts.current(&settings))
        .into_iter()
        .map(|routine| ArgChoice {
            word: routine.id.clone(),
            title: Some(routine.name.clone()),
            argument: routine.id.clone(),
        })
        .collect()
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add_family(
        Command::new("routine:", "Open a routine")
            .needs_vault()
            .family("<routine>")
            .choices(routine_choices)
            .run(|app, id| async move { open(&app, &id).await }),
    );
}
