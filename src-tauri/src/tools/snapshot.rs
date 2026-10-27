//! The `ToolState` of the running app: each feature fills its part.

use tauri::{AppHandle, Manager};

use super::state::{
    ContextItem, GroupProgress, LinkItem, Progress, RoutineProgress, TodayNote, ToolState, Topic,
};
use crate::state::AppState;
use crate::{calls, jira, journal, links, music, projects, quick_notes, routines, search, status};

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
    let note_topics = quick_notes::today_topics(app, &settings).unwrap_or_else(|e| {
        log::warn!("Couldn't list today's note topics: {e}");
        Vec::new()
    });
    let quick_access = links::current(app);
    let link_items = |items: Vec<_>| items.into_iter().map(LinkItem::Link).collect();
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
        search_scopes: search::scopes::resolve(&settings.search.scopes).names(),
        note_topics: note_topics
            .into_iter()
            .map(|topic| Topic {
                title: topic.title,
                level: topic.level,
            })
            .collect(),
        links_now: link_items(quick_access.now),
        links_more: link_items(quick_access.more),
        tickets: jira::today_tickets(app),
        music: music::tool_state(app),
    }
}
