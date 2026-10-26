//! The `ToolState` of the running app: each feature fills its part.

use tauri::{AppHandle, Manager};

use super::state::{ContextItem, GroupProgress, Progress, RoutineProgress, TodayNote, ToolState};
use crate::state::AppState;
use crate::{calls, journal, projects, routines, status};

pub fn snapshot(app: &AppHandle) -> ToolState {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let today = state.notes.state(&settings);
    let context = state.contexts.current(&settings);
    let context_id = context.map(|context| context.id.as_str());
    let offered = projects::current_projects(&settings.projects, context_id);
    let today_status = status::today(app);
    let call = calls::active(app);
    let journal_groups = journal::progress_of(app, &settings, state.notes.today());
    let (answered, total) = journal::overall(&journal_groups);
    let routines = routines::progress_of(app, &settings, state.notes.today());
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
        meeting_since: call
            .as_ref()
            .map(|call| call.since.format("%H:%M").to_string()),
        meeting_app: call.map(|call| call.app),
        meeting_detection: calls::detecting(&settings),
        journal: Progress {
            done: answered,
            total,
        },
        journal_groups: journal_groups
            .into_iter()
            .map(|group| GroupProgress {
                title: group.title,
                progress: Progress {
                    done: group.answered,
                    total: group.total,
                },
            })
            .collect(),
        routines: routines
            .into_iter()
            .map(|routine| RoutineProgress {
                id: routine.id,
                name: routine.name,
                progress: Progress {
                    done: routine.done,
                    total: routine.total,
                },
            })
            .collect(),
        ..ToolState::default()
    }
}
