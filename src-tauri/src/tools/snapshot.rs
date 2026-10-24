//! The `ToolState` of the running app: each feature fills its part.

use tauri::{AppHandle, Manager};

use super::state::{ContextItem, TodayNote, ToolState};
use crate::state::AppState;

pub fn snapshot(app: &AppHandle) -> ToolState {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let today = state.notes.state(&settings);
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
        ..ToolState::default()
    }
}
