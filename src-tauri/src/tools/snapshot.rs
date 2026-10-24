//! The `ToolState` of the running app: each feature fills its part.

use tauri::{AppHandle, Manager};

use super::state::{ContextItem, TodayNote, ToolState};
use crate::state::AppState;
use crate::{projects, status};

pub fn snapshot(app: &AppHandle) -> ToolState {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let today = state.notes.state(&settings);
    let context = state.contexts.current(&settings);
    let context_id = context.map(|context| context.id.as_str());
    let offered = projects::current_projects(&settings.projects, context_id);
    let today_status = status::today(app);
    ToolState {
        has_vault: settings.vault.path.is_some(),
        today: TodayNote {
            exists: today.exists,
            creating: today.creating,
            waiting: today.waiting,
        },
        contexts: settings
            .contexts
            .iter()
            .map(|context| ContextItem {
                id: context.id.clone(),
                label: context.label.clone(),
                name: context.name.clone(),
            })
            .collect(),
        context: context_id.map(str::to_string),
        features: context
            .map(|context| context.features.clone())
            .unwrap_or_default(),
        status: today_status.status,
        status_since: today_status.since,
        projects: projects::project_groups(&offered, None),
        ..ToolState::default()
    }
}
