//! The Daybook window's view model: what the window shows for a day, built here and rendered by the
//! page.

mod day;
mod days;
mod index;
mod reminder;
mod tools;

use chrono::{Local, NaiveDate, NaiveDateTime, Timelike};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

pub use day::{DayEvents, DayInput, day_events};
pub use days::adjacent_days;
pub use reminder::{ReminderLine, reminder_line};
pub use tools::{HubInput, HubTool, ToolRow, hub_tools, shortcut_for};

use crate::bus;
use crate::calendar;
use crate::clock::{iso_day, parse_day};
use crate::error::AppResult;
use crate::focus::{self, FocusLine, focus_lines};
use crate::journal::{self, JournalProgress};
use crate::locale::{self, DateLocale};
use crate::reminders::{ReminderInput, ReminderJournalGroup, ReminderRoutine, pick_reminder};
use crate::routines::{self, RoutineProgress};
use crate::slack::{self, ShownStatus};
use crate::state::AppState;
use crate::store::settings::{Settings, Status};
use crate::timetracker::commands::{TimeTrackerLink, show_link};
use crate::todos::TodoGroup;
use crate::vault::commands::blocking;
use crate::vault::ops::notes::NoteSectionRef;
use crate::{shortcuts, tools as tool_state};
use index::{read_index, todo_dates};

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct HubView {
    /// `YYYY-MM-DD`.
    pub day: String,
    pub is_today: bool,
    /// "Saturday", in the Mac's locale.
    pub weekday: String,
    /// "24. Oktober 2026", in the Mac's locale.
    pub date: String,
    /// The board's title: a greeting today, the day's name (in the Mac's locale) on another day.
    pub title: String,
    /// The closest days with a note, for ‹ and ›.
    pub previous_day: Option<String>,
    pub next_day: Option<String>,
    /// The sidebar's tool rows, in Settings → General → Tools order.
    pub tools: Vec<HubTool>,
    /// Today's status, for the status row's dot; `None` before the day's first.
    pub status: Option<Status>,
    /// Today only: the first reminder rule that applies now.
    pub reminder: Option<ReminderLine>,
    /// The day's events in the current context's calendars.
    pub events: DayEvents,
    /// What Slack shows, in contexts with Slack once it is connected.
    pub slack: Option<ShownStatus>,
    /// The day's focus list; `None` while the focus workflow is off.
    pub focus: Option<Vec<FocusLine>>,
    /// The day's journal groups, in Settings order.
    pub journal: Vec<JournalProgress>,
    /// The current context's routines on the day, in Settings order.
    pub routines: Vec<RoutineProgress>,
    /// The topics of the day's notes section.
    pub notes: Vec<NoteSectionRef>,
    /// The global shortcuts beside the Focus and Today's notes titles, in Settings notation.
    pub focus_shortcut: Option<String>,
    pub notes_shortcut: Option<String>,
    /// Every open todo in the daily notes, the same on every day.
    pub todos: Vec<TodoGroup>,
    /// Shows the day in TimeTracker.
    pub timetracker: TimeTrackerLink,
    /// The inspector offers the Activity tab: the context has the Status feature and TimeTracker is
    /// installed.
    pub activity_tab: bool,
}

fn greeting(hour: u32) -> &'static str {
    match hour {
        0..12 => "Good morning",
        12..18 => "Good afternoon",
        _ => "Good evening",
    }
}

/// The view of `day` (today when `None`) at `now`, its dates in `locale`.
pub fn hub_view_at(
    day: Option<NaiveDate>,
    now: NaiveDateTime,
    locale: DateLocale,
    tools: Vec<HubTool>,
    status: Option<Status>,
) -> HubView {
    let today = now.date();
    let day = day.unwrap_or(today);
    let is_today = day == today;
    HubView {
        day: iso_day(day),
        is_today,
        weekday: locale.weekday(day),
        date: locale.long_date(day),
        title: if is_today {
            greeting(now.hour()).to_string()
        } else {
            locale.day_title(day)
        },
        previous_day: None,
        next_day: None,
        tools,
        status,
        reminder: None,
        events: DayEvents::default(),
        slack: None,
        focus: None,
        journal: Vec::new(),
        routines: Vec::new(),
        notes: Vec::new(),
        focus_shortcut: None,
        notes_shortcut: None,
        todos: Vec::new(),
        timetracker: show_link(day, is_today, locale),
        activity_tab: false,
    }
}

/// The Daybook window's content for `day` (`YYYY-MM-DD`, today when absent).
#[tauri::command]
pub async fn hub_view(app: AppHandle, day: Option<String>) -> AppResult<HubView> {
    let day = day.as_deref().map(parse_day).transpose()?;
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
    let locale = locale::system_async().await;
    let mut view = hub_view_at(day, now, locale, tools, state.status);
    view.activity_tab = state.offers_day_activity();
    let set = |shortcut: &String| (!shortcut.is_empty()).then(|| shortcut.clone());
    view.focus_shortcut = set(&hotkeys.focus);
    view.notes_shortcut = set(&hotkeys.quick_notes);
    view.slack = slack::shown_now(&app).await;
    if settings.focus.enabled {
        let body = read_off_thread(&app, &settings, shown, "the focus list", focus::read_day).await;
        view.focus = Some(focus_lines(&body, &settings.projects));
    }
    view.journal = read_off_thread(
        &app,
        &settings,
        shown,
        "the journal",
        |app, settings, day| Ok(journal::progress_of(app, settings, day)),
    )
    .await;
    view.routines = read_off_thread(
        &app,
        &settings,
        shown,
        "the routines",
        |app, settings, day| Ok(routines::progress_of(app, settings, day)),
    )
    .await;
    let index = read_index(&app, &settings, shown, now.date()).await;
    view.previous_day = index.previous.map(|day| day.to_string());
    view.next_day = index.next.map(|day| day.to_string());
    view.notes = index.notes;
    view.todos = index.todos;
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
            routines: &reminder_routines(&view.routines, &settings),
            journal: &reminder_journal(&view.journal, &settings),
            todo_dates: &todo_dates(&view.todos),
        })
        .map(reminder_line);
    }
    Ok(view)
}

/// `read` of `day` on a blocking thread; empty when it fails.
async fn read_off_thread<T: Default + Send + 'static>(
    app: &AppHandle,
    settings: &Settings,
    day: NaiveDate,
    what: &str,
    read: fn(&AppHandle, &Settings, NaiveDate) -> AppResult<T>,
) -> T {
    let (handle, reading) = (app.clone(), settings.clone());
    blocking(move || read(&handle, &reading, day))
        .await
        .unwrap_or_else(|e| {
            log::warn!("Couldn't read {what} of {day}: {e}");
            T::default()
        })
}

fn reminder_routines(routines: &[RoutineProgress], settings: &Settings) -> Vec<ReminderRoutine> {
    routines
        .iter()
        .map(|routine| ReminderRoutine {
            id: routine.id.clone(),
            name: routine.name.clone(),
            done: routine.done,
            total: routine.total,
            due: settings
                .routines
                .iter()
                .find(|config| config.id == routine.id)
                .and_then(|config| config.timing.due.clone()),
        })
        .collect()
}

fn reminder_journal(groups: &[JournalProgress], settings: &Settings) -> Vec<ReminderJournalGroup> {
    groups
        .iter()
        .map(|group| ReminderJournalGroup {
            title: group.title.clone(),
            answered: group.answered,
            total: group.total,
            has_note: group.has_note,
            due: settings
                .journal
                .iter()
                .find(|config| config.title == group.title)
                .and_then(|config| config.timing.due.clone()),
        })
        .collect()
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

    fn english() -> DateLocale {
        DateLocale::parse("en_GB")
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
            let view = hub_view_at(None, at(hour), english(), Vec::new(), None);
            assert_eq!(view.title, title, "{hour}");
            assert!(view.is_today, "{hour}");
        }
    }

    #[test]
    fn today_shows_its_day_weekday_and_date() {
        let view = hub_view_at(None, at(9), english(), Vec::new(), None);
        assert_eq!(view.day, "2026-10-24");
        assert_eq!(view.weekday, "Saturday");
        assert_eq!(view.date, "24 October 2026");
        assert_eq!(view.previous_day, None);
        assert_eq!(view.next_day, None);
    }

    #[test]
    fn another_day_is_titled_by_its_name() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 23);
        let view = hub_view_at(day, at(9), english(), Vec::new(), None);
        assert!(!view.is_today);
        assert_eq!(view.title, "Friday, 23 October");
        assert_eq!(view.day, "2026-10-23");
        assert_eq!(view.weekday, "Friday");
        assert_eq!(view.date, "23 October 2026");
    }

    #[test]
    fn dates_follow_the_locale() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 27);
        let view = hub_view_at(day, at(9), DateLocale::parse("de_DE"), Vec::new(), None);
        assert_eq!(view.weekday, "Dienstag");
        assert_eq!(view.date, "27. Oktober 2026");
        assert_eq!(view.title, "Dienstag, 27. Oktober");
    }

    #[test]
    fn the_shortcuts_of_focus_and_notes_are_unset_until_the_command_fills_them() {
        let view = hub_view_at(None, at(9), english(), Vec::new(), None);
        let json = serde_json::to_value(&view).unwrap();
        assert_eq!(view.focus_shortcut, None);
        assert_eq!(view.notes_shortcut, None);
        assert_eq!(json["focusShortcut"], serde_json::Value::Null);
        assert_eq!(json["notesShortcut"], serde_json::Value::Null);
    }

    #[test]
    fn asking_for_today_explicitly_is_today() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 24);
        let view = hub_view_at(day, at(19), english(), Vec::new(), None);
        assert!(view.is_today);
        assert_eq!(view.title, "Good evening");
    }

    #[test]
    fn the_activity_tab_is_not_offered_until_the_command_decides() {
        let view = hub_view_at(None, at(9), english(), Vec::new(), None);
        assert!(!view.activity_tab);
        assert_eq!(
            serde_json::to_value(&view).unwrap()["activityTab"],
            serde_json::json!(false)
        );
    }

    #[test]
    fn the_view_carries_todays_status_for_the_status_dot() {
        for status in [
            None,
            Some(Status::Working),
            Some(Status::Break),
            Some(Status::Off),
        ] {
            let view = hub_view_at(None, at(9), english(), Vec::new(), status);
            assert_eq!(view.status, status, "{status:?}");
        }
        let view = hub_view_at(None, at(9), english(), Vec::new(), Some(Status::Break));
        assert_eq!(
            serde_json::to_value(&view).unwrap()["status"],
            serde_json::json!("break")
        );
    }
}
