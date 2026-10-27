//! Window layout that survives a restart (`ui_state.json`): the sidebar, the inspector (its tab and the
//! Activity tab's view) and folded todo groups. Unknown keys are ignored.

use std::path::Path;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::json_store::JsonStore;
use crate::error::AppResult;

/// The inspector's tab.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum InspectorTab {
    #[default]
    Todos,
    /// TimeTracker's day.
    Activity,
}

/// How the Activity tab shows the day.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum ActivityTabView {
    #[default]
    Timeline,
    ByProject,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default, rename_all = "camelCase")]
pub struct UiState {
    pub sidebar_hidden: bool,
    /// The Daybook window's right column.
    pub inspector_hidden: bool,
    /// The board lists only today's events that aren't over.
    pub hide_past_events: bool,
    /// The labels of the inspector's todo groups that are folded away.
    pub collapsed_todo_groups: Vec<String>,
    /// Stays when the Activity tab is hidden, so it comes back with it.
    pub inspector_tab: InspectorTab,
    pub activity_view: ActivityTabView,
}

pub struct UiStateStore {
    store: JsonStore<UiState>,
}

impl UiStateStore {
    pub fn open(dir: &Path) -> Self {
        Self {
            store: JsonStore::open(dir.join("ui_state.json")),
        }
    }

    pub fn get(&self) -> UiState {
        self.store.read(UiState::clone)
    }

    pub async fn set(&self, ui: UiState) -> AppResult<UiState> {
        self.store
            .update_async(move |current| {
                *current = ui.clone();
                ui
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_config_dir;

    #[test]
    fn defaults_apply_without_a_file() {
        let dir = temp_config_dir();
        let ui = UiStateStore::open(dir.path()).get();
        assert_eq!(
            ui,
            UiState {
                sidebar_hidden: false,
                inspector_hidden: false,
                hide_past_events: false,
                collapsed_todo_groups: Vec::new(),
                inspector_tab: InspectorTab::Todos,
                activity_view: ActivityTabView::Timeline,
            }
        );
    }

    #[tokio::test]
    async fn collapsed_todo_groups_persist_across_reopen_and_default_to_none() {
        let dir = temp_config_dir();
        let ui = UiState {
            collapsed_todo_groups: vec!["Last Week".to_string(), "2025".to_string()],
            ..UiState::default()
        };
        UiStateStore::open(dir.path())
            .set(ui.clone())
            .await
            .unwrap();
        assert_eq!(
            UiStateStore::open(dir.path()).get().collapsed_todo_groups,
            ["Last Week", "2025"]
        );

        let other = temp_config_dir();
        std::fs::write(
            other.path().join("ui_state.json"),
            r#"{"sidebarWidth": 300}"#,
        )
        .unwrap();
        assert!(
            UiStateStore::open(other.path())
                .get()
                .collapsed_todo_groups
                .is_empty()
        );
        assert_eq!(
            serde_json::to_value(&ui).unwrap()["collapsedTodoGroups"],
            serde_json::json!(["Last Week", "2025"])
        );
    }

    #[test]
    fn a_file_without_the_hide_past_events_key_shows_past_events() {
        let dir = temp_config_dir();
        std::fs::write(dir.path().join("ui_state.json"), r#"{"sidebarWidth": 300}"#).unwrap();
        assert!(!UiStateStore::open(dir.path()).get().hide_past_events);
    }

    #[tokio::test]
    async fn hiding_past_events_persists_across_reopen() {
        let dir = temp_config_dir();
        let ui = UiState {
            hide_past_events: true,
            ..UiState::default()
        };
        UiStateStore::open(dir.path()).set(ui).await.unwrap();
        assert!(UiStateStore::open(dir.path()).get().hide_past_events);
    }

    #[tokio::test]
    async fn the_hidden_inspector_persists_and_defaults_to_shown() {
        let dir = temp_config_dir();
        let ui = UiState {
            inspector_hidden: true,
            ..UiState::default()
        };
        UiStateStore::open(dir.path())
            .set(ui.clone())
            .await
            .unwrap();
        assert_eq!(UiStateStore::open(dir.path()).get(), ui);

        let other = temp_config_dir();
        std::fs::write(
            other.path().join("ui_state.json"),
            r#"{"sidebarWidth": 300}"#,
        )
        .unwrap();
        assert!(!UiStateStore::open(other.path()).get().inspector_hidden);
    }

    #[tokio::test]
    async fn saved_state_persists_across_reopen() {
        let dir = temp_config_dir();
        let ui = UiState {
            sidebar_hidden: true,
            inspector_hidden: true,
            hide_past_events: true,
            collapsed_todo_groups: vec!["Today".to_string()],
            inspector_tab: InspectorTab::Activity,
            activity_view: ActivityTabView::ByProject,
        };
        let saved = UiStateStore::open(dir.path())
            .set(ui.clone())
            .await
            .unwrap();
        assert_eq!(saved, ui);
        assert_eq!(UiStateStore::open(dir.path()).get(), ui);
    }

    #[test]
    fn partial_file_fills_the_missing_fields_with_defaults() {
        let dir = temp_config_dir();
        std::fs::write(
            dir.path().join("ui_state.json"),
            r#"{"sidebarHidden": true}"#,
        )
        .unwrap();
        assert_eq!(
            UiStateStore::open(dir.path()).get(),
            UiState {
                sidebar_hidden: true,
                ..UiState::default()
            }
        );
    }

    #[tokio::test]
    async fn an_old_file_with_sidebar_width_and_pane_sizes_loads_and_drops_them_on_save() {
        let dir = temp_config_dir();
        std::fs::write(
            dir.path().join("ui_state.json"),
            r#"{"sidebarHidden": true, "sidebarWidth": 320, "paneSizes": {"list": 300.0}}"#,
        )
        .unwrap();
        let store = UiStateStore::open(dir.path());
        let loaded = store.get();
        assert_eq!(
            loaded,
            UiState {
                sidebar_hidden: true,
                ..UiState::default()
            }
        );

        store.set(loaded).await.unwrap();

        let saved: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.path().join("ui_state.json")).unwrap(),
        )
        .unwrap();
        assert!(saved.get("sidebarWidth").is_none());
        assert!(saved.get("paneSizes").is_none());
        assert_eq!(saved["sidebarHidden"], true);
    }

    #[test]
    fn keys_are_camel_case() {
        assert_eq!(
            serde_json::to_value(UiState::default()).unwrap(),
            serde_json::json!({
                "sidebarHidden": false,
                "inspectorHidden": false,
                "hidePastEvents": false,
                "collapsedTodoGroups": [],
                "inspectorTab": "todos",
                "activityView": "timeline"
            })
        );
    }

    #[test]
    fn the_inspector_tab_and_activity_view_are_read_from_their_camel_case_names() {
        let dir = temp_config_dir();
        std::fs::write(
            dir.path().join("ui_state.json"),
            r#"{"inspectorTab": "activity", "activityView": "byProject"}"#,
        )
        .unwrap();
        let loaded = UiStateStore::open(dir.path()).get();
        assert_eq!(loaded.inspector_tab, InspectorTab::Activity);
        assert_eq!(loaded.activity_view, ActivityTabView::ByProject);
        let json = serde_json::to_value(&loaded).unwrap();
        assert_eq!(json["inspectorTab"], "activity");
        assert_eq!(json["activityView"], "byProject");
    }

    #[test]
    fn a_file_with_unknown_keys_loads_without_quarantining_the_file() {
        let dir = temp_config_dir();
        let raw = r#"{"sidebarHidden": true, "lastDay": "2026-10-21"}"#;
        std::fs::write(dir.path().join("ui_state.json"), raw).unwrap();

        let loaded = UiStateStore::open(dir.path()).get();

        assert_eq!(
            loaded,
            UiState {
                sidebar_hidden: true,
                ..UiState::default()
            }
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("ui_state.json")).unwrap(),
            raw
        );
    }
}
