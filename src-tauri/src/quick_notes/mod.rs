//! Quick notes (docs/vault.md#quick-notes): pick or name a topic below the notes heading of today's note,
//! then write it in the editor window. A save never overwrites a change made in the note meanwhile.

pub mod presentation;
pub mod topic;

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use crate::bus::{ArgChoice, Command, CommandBus};
use crate::error::{AppError, AppResult};
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::store::json_store::JsonStore;
use crate::store::settings::Settings;
use crate::vault::commands::blocking;
use crate::vault::note::Note;
use crate::vault::ops::notes::{
    NoteSectionRef, list_note_sections, read_note_section, section_holds,
};
use crate::vault::ops::{NoteLayout, NoteOp};
use crate::window::display::Presentation;
use crate::{action_log, window};
use topic::TopicRow;

const PICKER: &str = "quicknotes";
const MAX_BODY: usize = 200_000;

/// The editor window's frame in points.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
struct Stored {
    frame: Option<Frame>,
}

/// The topic the editor shows, the editor's frame (`quick_notes.json`) and, in memory only, its
/// presentation.
pub struct QuickNotes {
    target: Mutex<Option<NoteSectionRef>>,
    stored: JsonStore<Stored>,
    presentation: Mutex<Option<Presentation>>,
}

impl QuickNotes {
    pub fn open(config_dir: &Path) -> Self {
        Self {
            target: Mutex::default(),
            stored: JsonStore::open(config_dir.join("quick_notes.json")),
            presentation: Mutex::default(),
        }
    }

    pub fn frame(&self) -> Option<Frame> {
        self.stored.read(|stored| stored.frame)
    }

    pub fn set_frame(&self, frame: Frame) {
        if let Err(e) = self.stored.update(|stored| stored.frame = Some(frame)) {
            log::warn!("Couldn't keep the quick notes editor's frame: {e}");
        }
    }

    fn target(&self) -> Option<NoteSectionRef> {
        self.target.lock_unpoisoned().clone()
    }

    fn set_target(&self, topic: NoteSectionRef) {
        *self.target.lock_unpoisoned() = Some(topic);
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct QuickNoteTopics {
    pub rows: Vec<TopicRow>,
    /// Whether today's note has any topic.
    pub has_topics: bool,
    /// Where the topics live, e.g. `## Notes`.
    pub notes_heading: String,
}

/// A topic and its text in today's note; empty when the topic or the note doesn't exist yet.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct QuickNote {
    pub title: String,
    pub level: u8,
    pub body: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum QuickNoteSaved {
    /// Written, or waiting for today's note; `topic` is its heading now.
    Saved { topic: NoteSectionRef },
    /// The note holds another text for the topic than the editor started from; nothing was written.
    Changed { note: QuickNote },
}

fn notes_heading(settings: &Settings) -> String {
    NoteLayout::from_settings(settings).notes
}

fn today_lines(app: &AppHandle, settings: &Settings) -> AppResult<Option<Vec<String>>> {
    let notes = &app.state::<AppState>().notes;
    let text = notes.read_note(settings, notes.today())?;
    Ok(text.map(|text| Note::parse(&text).lines))
}

/// The topics below the notes heading of today's note; none before it exists.
pub fn today_topics(app: &AppHandle, settings: &Settings) -> AppResult<Vec<NoteSectionRef>> {
    let heading = notes_heading(settings);
    Ok(today_lines(app, settings)?
        .map(|lines| list_note_sections(&lines, &heading))
        .unwrap_or_default())
}

fn read(app: &AppHandle, settings: &Settings, topic: NoteSectionRef) -> AppResult<QuickNote> {
    let heading = notes_heading(settings);
    let body = today_lines(app, settings)?
        .and_then(|lines| read_note_section(&lines, &heading, &topic))
        .unwrap_or_default();
    Ok(QuickNote {
        title: topic.title,
        level: topic.level,
        body,
    })
}

fn choose(app: &AppHandle, topic: NoteSectionRef) {
    app.state::<AppState>().quick_notes.set_target(topic);
    window::quick_notes::open(app);
}

fn current_settings(app: &AppHandle) -> Settings {
    app.state::<AppState>().settings.get()
}

/// Writes `body` as `topic`'s text under `new_title`, unless the note holds another text than `expected`.
async fn write_topic(
    app: &AppHandle,
    topic: &NoteSectionRef,
    new_title: &str,
    body: String,
    expected: String,
) -> AppResult<()> {
    let notes = Arc::clone(&app.state::<AppState>().notes);
    let settings = current_settings(app);
    let op = NoteOp::NoteSection {
        title: topic.title.clone(),
        level: topic.level,
        new_title: new_title.to_string(),
        body,
        expected: Some(expected),
    };
    let title = format!("Quick note: {}", topic.title);
    action_log::traced_merged(&title, blocking(move || notes.edit_today(&settings, op))).await?;
    Ok(())
}

/// The note's text for `topic` when it isn't `base` any more.
async fn changed_since(
    app: &AppHandle,
    topic: NoteSectionRef,
    base: String,
) -> AppResult<Option<QuickNote>> {
    let app = app.clone();
    blocking(move || {
        let settings = current_settings(&app);
        let lines = today_lines(&app, &settings)?.unwrap_or_default();
        if section_holds(&lines, &notes_heading(&settings), &topic, &base) {
            return Ok(None);
        }
        read(&app, &settings, topic).map(Some)
    })
    .await
}

/// The picker's rows for `query`: today's matching topics, then a new one named after it.
#[tauri::command]
pub async fn quick_notes_list(app: AppHandle, query: String) -> AppResult<QuickNoteTopics> {
    blocking(move || {
        let settings = current_settings(&app);
        let topics = today_topics(&app, &settings)?;
        Ok(QuickNoteTopics {
            rows: topic::rows_for(&topics, &query),
            has_topics: !topics.is_empty(),
            notes_heading: notes_heading(&settings),
        })
    })
    .await
}

/// Closes the picker and opens the editor on `topic`. A new topic is written as an empty section at
/// once, so it shows in Obsidian too; a topic that has text already keeps it.
#[tauri::command]
pub async fn quick_notes_open(
    app: AppHandle,
    topic: NoteSectionRef,
    create: bool,
) -> AppResult<()> {
    let topic = topic::checked(topic)?;
    if create
        && let Err(e) = write_topic(&app, &topic, &topic.title, String::new(), String::new()).await
    {
        log::info!("Didn't add the topic {}: {e}", topic.title);
    }
    window::panel::close(&app, true);
    choose(&app, topic);
    Ok(())
}

/// The editor's topic and its text; `None` before a topic was chosen.
#[tauri::command]
pub async fn quick_note_get(app: AppHandle) -> AppResult<Option<QuickNote>> {
    let Some(topic) = app.state::<AppState>().quick_notes.target() else {
        return Ok(None);
    };
    blocking(move || read(&app, &current_settings(&app), topic).map(Some)).await
}

/// Writes `body` as `topic`'s text, renamed to `new_title` (empty keeps the title), unless the note
/// no longer holds `base`, the text the editor started from.
#[tauri::command]
pub async fn quick_note_save(
    app: AppHandle,
    topic: NoteSectionRef,
    new_title: String,
    body: String,
    base: String,
) -> AppResult<QuickNoteSaved> {
    let topic = topic::checked(topic)?;
    if body.chars().count() > MAX_BODY {
        return Err(AppError::InvalidInput(format!(
            "A quick note is longer than {MAX_BODY} characters."
        )));
    }
    let renamed = match new_title.trim() {
        "" => topic.clone(),
        title => topic::checked(NoteSectionRef {
            title: title.to_string(),
            level: topic.level,
        })?,
    };
    if let Err(e) = write_topic(&app, &topic, &renamed.title, body, base.clone()).await {
        return match changed_since(&app, topic, base).await? {
            Some(note) => Ok(QuickNoteSaved::Changed { note }),
            None => Err(e),
        };
    }
    app.state::<AppState>()
        .quick_notes
        .set_target(renamed.clone());
    Ok(QuickNoteSaved::Saved { topic: renamed })
}

#[tauri::command]
pub fn quick_notes_close(app: AppHandle) {
    window::quick_notes::hide(&app);
}

fn topic_choices(app: &AppHandle) -> Vec<ArgChoice> {
    let topics = today_topics(app, &current_settings(app)).unwrap_or_else(|e| {
        log::warn!("Couldn't list today's note topics: {e}");
        Vec::new()
    });
    topics
        .iter()
        .map(|topic| ArgChoice {
            word: topic.title.clone(),
            title: None,
            argument: topic::to_argument(topic),
        })
        .collect()
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("notes:quick", "Quick Notes…")
            .help("Write a topic of today's notes; brings an open editor forward")
            .needs_vault()
            .run(|app| async move {
                if window::quick_notes::visible(&app) {
                    window::quick_notes::focus(&app);
                } else {
                    window::panel::open(&app, PICKER, Value::Null).await;
                }
                Ok(())
            }),
    );
    presentation::register(bus);
    bus.add_family(
        Command::new("notes:topic:", "Write a topic of today's notes")
            .needs_vault()
            .family("<level>:<title>")
            .choices(topic_choices)
            .run(|app, argument| async move {
                choose(&app, topic::from_argument(&argument)?);
                Ok(())
            }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_config_dir;

    const FRAME: Frame = Frame {
        x: 10.0,
        y: 20.5,
        width: 480.0,
        height: 320.0,
    };

    #[test]
    fn the_editor_has_no_frame_at_first() {
        let dir = temp_config_dir();
        assert_eq!(QuickNotes::open(dir.path()).frame(), None);
    }

    #[test]
    fn a_frame_survives_reopening_the_store() {
        let dir = temp_config_dir();
        QuickNotes::open(dir.path()).set_frame(FRAME);
        assert_eq!(QuickNotes::open(dir.path()).frame(), Some(FRAME));
    }

    #[test]
    fn the_latest_frame_wins() {
        let dir = temp_config_dir();
        let quick_notes = QuickNotes::open(dir.path());
        quick_notes.set_frame(FRAME);
        let moved = Frame { x: 0.0, ..FRAME };
        quick_notes.set_frame(moved);
        assert_eq!(QuickNotes::open(dir.path()).frame(), Some(moved));
    }
}
