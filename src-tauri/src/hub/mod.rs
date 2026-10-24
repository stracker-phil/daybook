//! The Daybook window's view model: what the window shows for a day, built here and rendered by the
//! page.

mod day;
mod reminder;
mod tools;

use chrono::{Local, NaiveDate, NaiveDateTime, Timelike};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

pub use day::{DayEvents, DayInput, day_events};
pub use reminder::{ReminderLine, reminder_line};
pub use tools::{HubInput, HubTool, ToolRow, hub_tools};

use crate::bus;
use crate::calendar;
use crate::error::{AppError, AppResult};
use crate::reminders::{ReminderInput, pick_reminder};
use crate::state::AppState;
use crate::store::settings::Status;
use crate::{shortcuts, tools as tool_state};

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct HubView {
    /// `YYYY-MM-DD`.
    pub day: String,
    pub is_today: bool,
    /// "Saturday".
    pub weekday: String,
    /// "24 October 2026".
    pub date: String,
    /// The board's title: a greeting today, the day's name on another day.
    pub title: String,
    /// The closest days with a note, for ‹ and ›.
    pub previous_day: Option<String>,
    pub next_day: Option<String>,
    /// The sidebar's tool rows, in Settings → Tools order.
    pub tools: Vec<HubTool>,
    /// Today's status, for the status row's dot; `None` before the day's first.
    pub status: Option<Status>,
    /// Today only: the first reminder rule that applies now.
    pub reminder: Option<ReminderLine>,
    /// The day's events in the current context's calendars.
    pub events: DayEvents,
}

fn greeting(hour: u32) -> &'static str {
    match hour {
        0..12 => "Good morning",
        12..18 => "Good afternoon",
        _ => "Good evening",
    }
}

/// The view of `day` (today when `None`) at `now`.
pub fn hub_view_at(
    day: Option<NaiveDate>,
    now: NaiveDateTime,
    tools: Vec<HubTool>,
    status: Option<Status>,
) -> HubView {
    let today = now.date();
    let day = day.unwrap_or(today);
    let is_today = day == today;
    HubView {
        day: day.format("%Y-%m-%d").to_string(),
        is_today,
        weekday: day.format("%A").to_string(),
        date: day.format("%-d %B %Y").to_string(),
        title: if is_today {
            greeting(now.hour()).to_string()
        } else {
            day.format("%A, %-d %B").to_string()
        },
        previous_day: None,
        next_day: None,
        tools,
        status,
        reminder: None,
        events: DayEvents::default(),
    }
}

fn parse_day(day: Option<&str>) -> AppResult<Option<NaiveDate>> {
    day.map(|day| {
        NaiveDate::parse_from_str(day, "%Y-%m-%d")
            .map_err(|_| AppError::InvalidInput(format!("Not a day: {day}")))
    })
    .transpose()
}

/// The Daybook window's content for `day` (`YYYY-MM-DD`, today when absent).
#[tauri::command]
pub async fn hub_view(app: AppHandle, day: Option<String>) -> AppResult<HubView> {
    let day = parse_day(day.as_deref())?;
    let now = Local::now().naive_local();
    let shown = day.unwrap_or(now.date());
    let events = calendar::events_on(&app, shown, now.date()).await;
    let app_state = app.state::<AppState>();
    let settings = app_state.settings.get();
    let state = tool_state::snapshot(&app);
    let hotkeys = shortcuts::shown(&app);
    let commands = app.state::<bus::Commands>();
    let tools = hub_tools(HubInput {
        tools: &settings.tools,
        state: &state,
        hotkeys: &hotkeys,
        commands: &commands.available(&app),
    });
    let mut view = hub_view_at(day, now, tools, state.status);
    let context = app_state.contexts.current(&settings);
    view.events = day_events(&DayInput {
        day: shown,
        now,
        events: &events,
        timeline: &settings.timeline,
        work_hours: context.and_then(|context| context.work_hours.as_ref()),
    });
    if view.is_today {
        view.reminder = pick_reminder(&ReminderInput {
            now,
            rules: &settings.reminders,
            events: &events,
            routines: &[],
            journal: &[],
            todo_dates: &[],
        })
        .map(reminder_line);
    }
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(hour: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 24)
            .unwrap()
            .and_hms_opt(hour, 30, 0)
            .unwrap()
    }

    #[test]
    fn today_is_greeted_by_the_time_of_day() {
        for (hour, title) in [
            (0, "Good morning"),
            (11, "Good morning"),
            (12, "Good afternoon"),
            (17, "Good afternoon"),
            (18, "Good evening"),
            (23, "Good evening"),
        ] {
            let view = hub_view_at(None, at(hour), Vec::new(), None);
            assert_eq!(view.title, title, "{hour}");
            assert!(view.is_today, "{hour}");
        }
    }

    #[test]
    fn today_shows_its_day_weekday_and_date() {
        let view = hub_view_at(None, at(9), Vec::new(), None);
        assert_eq!(view.day, "2026-10-24");
        assert_eq!(view.weekday, "Saturday");
        assert_eq!(view.date, "24 October 2026");
        assert_eq!(view.previous_day, None);
        assert_eq!(view.next_day, None);
    }

    #[test]
    fn another_day_is_titled_by_its_name() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 23);
        let view = hub_view_at(day, at(9), Vec::new(), None);
        assert!(!view.is_today);
        assert_eq!(view.title, "Friday, 23 October");
        assert_eq!(view.day, "2026-10-23");
        assert_eq!(view.weekday, "Friday");
        assert_eq!(view.date, "23 October 2026");
    }

    #[test]
    fn asking_for_today_explicitly_is_today() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 24);
        let view = hub_view_at(day, at(19), Vec::new(), None);
        assert!(view.is_today);
        assert_eq!(view.title, "Good evening");
    }

    #[test]
    fn the_view_carries_todays_status_for_the_status_dot() {
        for status in [
            None,
            Some(Status::Working),
            Some(Status::Break),
            Some(Status::Off),
        ] {
            let view = hub_view_at(None, at(9), Vec::new(), status);
            assert_eq!(view.status, status, "{status:?}");
        }
        let view = hub_view_at(None, at(9), Vec::new(), Some(Status::Break));
        assert_eq!(
            serde_json::to_value(&view).unwrap()["status"],
            serde_json::json!("break")
        );
    }
}
