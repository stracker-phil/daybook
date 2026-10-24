//! The active context (`context.json`): which one is current, switching by hand and by work hours,
//! and the context overlay's data.

mod schedule;

pub use schedule::{AutoSwitch, scheduled};

use std::path::Path;
use std::sync::Mutex;
use std::time::Duration;

use chrono::{DateTime, Local, NaiveDateTime, SecondsFormat, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use crate::bus::{ArgChoice, Command, CommandBus};
use crate::error::{AppError, AppResult};
use crate::events::CONTEXT_CHANGED_EVENT;
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::store::json_store::JsonStore;
use crate::store::settings::{Context, ContextColor, Settings};
use crate::{notify, tray, window};

pub const FILE_NAME: &str = "context.json";
/// How often the work hours are checked.
const TICK: Duration = Duration::from_secs(30);

/// "🐈 Personal": the context's label (an emoji) before its name.
pub fn context_title(label: &str, name: &str) -> String {
    [label.trim(), name]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

/// What `context.json` holds; the contexts themselves are in the settings.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Active {
    pub id: Option<String>,
    /// When it was set, RFC 3339.
    pub set_at: Option<String>,
}

pub struct Contexts {
    store: JsonStore<Active>,
    auto: Mutex<AutoSwitch>,
}

/// The stored context, else the first; `None` without contexts.
pub fn current<'a>(contexts: &'a [Context], active: &Active) -> Option<&'a Context> {
    contexts
        .iter()
        .find(|context| active.id.as_deref() == Some(context.id.as_str()))
        .or_else(|| contexts.first())
}

fn local_time(stamp: &str) -> Option<NaiveDateTime> {
    DateTime::parse_from_rfc3339(stamp)
        .ok()
        .map(|time| time.with_timezone(&Local).naive_local())
}

impl Contexts {
    pub fn open(config_dir: &Path) -> Self {
        Self {
            store: JsonStore::open(config_dir.join(FILE_NAME)),
            auto: Mutex::default(),
        }
    }

    pub fn active(&self) -> Active {
        self.store.read(Active::clone)
    }

    pub fn current<'a>(&self, settings: &'a Settings) -> Option<&'a Context> {
        current(&settings.contexts, &self.active())
    }

    fn set(&self, id: &str) -> AppResult<()> {
        let set_at = Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        self.store.update(|active| {
            *active = Active {
                id: Some(id.to_string()),
                set_at: Some(set_at),
            }
        })
    }

    /// The context whose work hours just started, if it isn't current yet.
    fn due(&self, settings: &Settings, now: NaiveDateTime) -> Option<String> {
        let active = self.active();
        let current = current(&settings.contexts, &active).map(|context| context.id.as_str());
        let set_at = active.set_at.as_deref().and_then(local_time);
        self.auto
            .lock_unpoisoned()
            .check(&settings.contexts, current, set_at, now)
    }
}

/// The payload of `context_changed`.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ContextChanged {
    pub id: String,
}

/// A context as the switcher shows it.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ContextItem {
    pub id: String,
    /// `🐈 Personal`
    pub title: String,
    pub color: ContextColor,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ContextState {
    pub contexts: Vec<ContextItem>,
    pub current: Option<String>,
}

pub fn describe(settings: &Settings, active: &Active) -> ContextState {
    ContextState {
        contexts: settings
            .contexts
            .iter()
            .map(|context| ContextItem {
                id: context.id.clone(),
                title: context_title(&context.label, &context.name),
                color: context.color,
            })
            .collect(),
        current: current(&settings.contexts, active).map(|context| context.id.clone()),
    }
}

#[tauri::command]
pub fn context_state(state: tauri::State<'_, AppState>) -> ContextState {
    describe(&state.settings.get(), &state.contexts.active())
}

/// Makes `id` current; `Ok(None)` when it already is, else the contexts it switched from and to.
fn switch(app: &AppHandle, id: &str) -> AppResult<Option<(Option<Context>, Context)>> {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let next = settings
        .contexts
        .iter()
        .find(|context| context.id == id)
        .ok_or_else(|| AppError::NotFound(format!("No context “{id}”.")))?;
    let previous = state.contexts.current(&settings).cloned();
    if previous.as_ref().is_some_and(|previous| previous.id == id) {
        return Ok(None);
    }
    state.contexts.set(id)?;
    log::info!("Context changed to {}", next.name);
    if let Err(e) = app.emit(CONTEXT_CHANGED_EVENT, ContextChanged { id: id.to_string() }) {
        log::warn!("Couldn't announce the context change: {e}");
    }
    tray::refresh(app);
    Ok(Some((previous, next.clone())))
}

/// A switch from the switcher, the menu or a shortcut; the switcher closes afterwards.
async fn switch_by_hand(app: AppHandle, id: String) -> AppResult<()> {
    switch(&app, &id)?;
    if window::panel::view(&app).as_deref() == Some("context") {
        window::panel::close(&app, false);
    }
    Ok(())
}

/// Switches to the context whose work hours just started and says so.
fn apply_schedule(app: &AppHandle) {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let Some(id) = state.contexts.due(&settings, Local::now().naive_local()) else {
        return;
    };
    match switch(app, &id) {
        Ok(Some((previous, next))) => {
            let was = previous.map_or(String::new(), |previous| format!("Was {}.", previous.name));
            notify::show(app, &format!("Switched to {}", next.name), &was);
        }
        Ok(None) => {}
        Err(e) => log::warn!("Couldn't switch the context by its work hours: {e}"),
    }
}

/// Checks the work hours now and every `TICK`.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticks = tokio::time::interval(TICK);
        loop {
            ticks.tick().await;
            apply_schedule(&app);
        }
    });
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("context:switch", "Switch Context…")
            .help("Switch Context…: the switcher")
            .needs_vault()
            .run(|app| async move {
                window::panel::open(&app, "context", serde_json::Value::Null).await;
                Ok(())
            }),
    );
    bus.add_family(
        Command::new("mode:", "Switch to a context")
            .needs_vault()
            .family("<context>")
            .choices(|app: &AppHandle| {
                app.state::<AppState>()
                    .settings
                    .get()
                    .contexts
                    .iter()
                    .map(|context| ArgChoice {
                        word: context.id.clone(),
                        title: Some(context_title(&context.label, &context.name)),
                        argument: context.id.clone(),
                    })
                    .collect()
            })
            .run(switch_by_hand),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_config_dir;
    use serde_json::json;

    fn contexts() -> Vec<Context> {
        Context::read_all(&[
            json!({ "name": "Personal", "id": "personal", "label": "🐈", "color": "teal" }),
            json!({ "name": "Work", "id": "work", "label": "  " }),
        ])
    }

    fn settings() -> Settings {
        let mut settings = Settings::default();
        settings.contexts = contexts();
        settings
    }

    fn active(id: Option<&str>) -> Active {
        Active {
            id: id.map(String::from),
            set_at: None,
        }
    }

    #[test]
    fn the_title_puts_the_label_before_the_name() {
        assert_eq!(context_title("🐈", "Personal"), "🐈 Personal");
        assert_eq!(context_title(" [W] ", "Work"), "[W] Work");
    }

    #[test]
    fn an_empty_or_blank_label_is_left_out_of_the_title() {
        for label in ["", "   "] {
            assert_eq!(context_title(label, "Work"), "Work", "{label:?}");
        }
    }

    #[test]
    fn the_current_context_is_the_stored_one() {
        let contexts = contexts();
        let found = current(&contexts, &active(Some("work")));
        assert_eq!(found.map(|c| c.id.as_str()), Some("work"));
    }

    #[test]
    fn without_a_stored_or_known_id_the_first_context_is_current() {
        let contexts = contexts();
        for stored in [None, Some("gone")] {
            let found = current(&contexts, &active(stored));
            assert_eq!(found.map(|c| c.id.as_str()), Some("personal"), "{stored:?}");
        }
    }

    #[test]
    fn without_contexts_there_is_no_current_one() {
        assert_eq!(current(&[], &active(Some("work"))), None);
    }

    #[test]
    fn the_state_lists_titles_colours_and_the_current_id() {
        assert_eq!(
            describe(&settings(), &active(Some("work"))),
            ContextState {
                contexts: vec![
                    ContextItem {
                        id: "personal".into(),
                        title: "🐈 Personal".into(),
                        color: ContextColor::Teal,
                    },
                    ContextItem {
                        id: "work".into(),
                        title: "Work".into(),
                        color: ContextColor::Purple,
                    },
                ],
                current: Some("work".into()),
            }
        );
    }

    #[test]
    fn the_state_without_contexts_has_no_current_one() {
        let mut settings = settings();
        settings.contexts.clear();
        assert_eq!(
            describe(&settings, &Active::default()),
            ContextState {
                contexts: vec![],
                current: None
            }
        );
    }

    #[test]
    fn the_imported_file_shape_is_read() {
        let parsed: Active =
            serde_json::from_str(r#"{"id":"work","setAt":"2026-10-19T09:39:15.688Z"}"#).unwrap();
        assert_eq!(
            parsed,
            Active {
                id: Some("work".into()),
                set_at: Some("2026-10-19T09:39:15.688Z".into()),
            }
        );
    }

    #[test]
    fn missing_fields_in_the_file_are_empty() {
        let parsed: Active = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, Active::default());
        let only_id: Active = serde_json::from_str(r#"{"id":"work"}"#).unwrap();
        assert_eq!(only_id, active(Some("work")));
    }

    #[test]
    fn opening_reads_the_stored_context_from_the_file() {
        let dir = temp_config_dir();
        std::fs::write(
            dir.path().join(FILE_NAME),
            r#"{"id":"work","setAt":"2026-10-19T09:39:15.688Z"}"#,
        )
        .unwrap();
        let store = Contexts::open(dir.path());
        let settings = settings();
        assert_eq!(store.active().id.as_deref(), Some("work"));
        assert_eq!(
            store.current(&settings).map(|c| c.id.as_str()),
            Some("work")
        );
    }

    #[test]
    fn without_a_file_the_first_context_is_current() {
        let dir = temp_config_dir();
        let store = Contexts::open(dir.path());
        let settings = settings();
        assert_eq!(store.active(), Active::default());
        assert_eq!(
            store.current(&settings).map(|c| c.id.as_str()),
            Some("personal")
        );
    }
}
