//! TimeTracker's Tauri commands (Settings, the project picker) and its bus commands.

use chrono::NaiveDate;
use serde::Serialize;
use tauri::AppHandle;
use ts_rs::TS;

use super::day::TimeTrackerDay;
use super::reply::TtProject;
use super::{Tracking, TtState, blocking, client, tracking};
use crate::bus::{Command, CommandBus};
use crate::error::{AppError, AppResult};
use crate::window;

/// `timetracker:show:<day>` shows a day.
pub const SHOW_PREFIX: &str = "timetracker:show:";

/// Settings → Connections → TimeTracker.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct TimeTrackerInfo {
    /// The tool Daybook runs.
    pub path: String,
    /// The tool's path when Settings leaves it empty.
    pub default_path: String,
    pub state: TtState,
    /// From `status`, while it runs.
    pub version: Option<String>,
}

/// The picker's projects; the archived ones too, so a linked one still has its name.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct TimeTrackerProjects {
    pub state: TtState,
    pub projects: Vec<TtProject>,
}

#[tauri::command]
pub async fn timetracker_info(app: AppHandle) -> TimeTrackerInfo {
    super::info(&app).await
}

/// The projects listed last, or listed now when `fresh` or never listed.
#[tauri::command]
pub async fn timetracker_projects(app: AppHandle, fresh: bool) -> TimeTrackerProjects {
    super::projects_for_page(&app, fresh).await
}

#[tauri::command]
pub fn timetracker_timer(app: AppHandle) -> Tracking {
    tracking(&app)
}

/// The Activity tab: TimeTracker's `day` (`YYYY-MM-DD`), read-only. Later changes to it come as
/// `activity_changed`.
#[tauri::command]
pub async fn timetracker_day(app: AppHandle, day: String) -> AppResult<TimeTrackerDay> {
    super::day_for_page(&app, parse_day(&day)?).await
}

/// Brings TimeTracker to the front, starting it if needed.
#[tauri::command]
pub async fn timetracker_focus(app: AppHandle) -> AppResult<()> {
    blocking(&app, |tool| client::focus(tool)).await
}

/// The Daybook window's way to the shown day in TimeTracker.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct TimeTrackerLink {
    pub command: String,
    pub label: String,
}

/// "Open in TimeTracker" today, "Show 26 October 2026 in TimeTracker" on another day.
pub fn show_link(day: NaiveDate, is_today: bool) -> TimeTrackerLink {
    if is_today {
        return TimeTrackerLink {
            command: "timetracker:show".into(),
            label: "Open in TimeTracker".into(),
        };
    }
    TimeTrackerLink {
        command: format!("{SHOW_PREFIX}{}", day.format("%Y-%m-%d")),
        label: format!("Show {} in TimeTracker", day.format("%-d %B %Y")),
    }
}

/// `YYYY-MM-DD`.
pub fn parse_day(argument: &str) -> AppResult<NaiveDate> {
    NaiveDate::parse_from_str(argument.trim(), "%Y-%m-%d")
        .map_err(|_| AppError::InvalidInput(format!("Not a day: {argument} (YYYY-MM-DD)")))
}

async fn show(app: AppHandle, day: Option<NaiveDate>) -> AppResult<()> {
    window::panel::close(&app, true);
    blocking(&app, move |tool| client::show(tool, day)).await
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("timetracker:show", "Open in TimeTracker")
            .help("Show today in TimeTracker")
            .run(|app| show(app, None)),
    );
    bus.add_family(
        Command::new(SHOW_PREFIX, "Show a day in TimeTracker")
            .family("<day>")
            .run(|app, argument| async move { show(app, Some(parse_day(&argument)?)).await }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    #[test]
    fn a_day_is_read_from_year_month_day() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 26).unwrap();
        for input in ["2026-10-26", " 2026-10-26\n"] {
            assert_eq!(parse_day(input), Ok(day), "{input:?}");
        }
    }

    #[test]
    fn today_links_to_the_plain_show_command() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 27).unwrap();
        assert_eq!(
            show_link(day, true),
            TimeTrackerLink {
                command: "timetracker:show".into(),
                label: "Open in TimeTracker".into(),
            }
        );
    }

    #[test]
    fn another_day_links_to_that_day_with_a_dated_label() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 26).unwrap();
        assert_eq!(
            show_link(day, false),
            TimeTrackerLink {
                command: "timetracker:show:2026-10-26".into(),
                label: "Show 26 October 2026 in TimeTracker".into(),
            }
        );
    }

    #[test]
    fn the_link_and_the_info_serialize_with_camel_case_fields() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 26).unwrap();
        assert_eq!(
            serde_json::to_value(show_link(day, true)).unwrap(),
            serde_json::json!({ "command": "timetracker:show", "label": "Open in TimeTracker" })
        );
        let info = TimeTrackerInfo {
            path: "/bin/tt".into(),
            default_path: client::DEFAULT_PATH.into(),
            state: TtState::Available,
            version: Some("1.0".into()),
        };
        assert_eq!(
            serde_json::to_value(info).unwrap(),
            serde_json::json!({
                "path": "/bin/tt",
                "defaultPath": client::DEFAULT_PATH,
                "state": "available",
                "version": "1.0"
            })
        );
    }

    #[test]
    fn anything_else_is_invalid_input_naming_the_argument() {
        for input in [
            "",
            "tomorrow",
            "26-10-2026",
            "2026-13-01",
            "2026-02-30",
            "2026-10",
        ] {
            let err = parse_day(input).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{input:?}");
            assert_eq!(
                err.to_string(),
                format!("Not a day: {input} (YYYY-MM-DD)"),
                "{input:?}"
            );
        }
    }
}
