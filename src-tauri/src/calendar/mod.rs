//! The active context's calendars through EventKit: the events of a day, cached for five minutes and
//! dropped when a calendar changes; calendar access for Settings (docs/calendar.md).

mod eventkit;
pub mod events;

use std::time::{Duration, Instant};

use chrono::{Days, NaiveDate, NaiveTime};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
use ts_rs::TS;

pub use eventkit::CalendarAccess;
pub use events::{CalendarEvent, Event, MeetingLink};

use crate::error::{AppError, AppResult};
use crate::events::CALENDAR_CHANGED_EVENT;
use crate::lock::MutexExt;
use crate::state::AppState;
use eventkit::Store;

const CACHE_FOR: Duration = Duration::from_secs(5 * 60);
const PRIVACY_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Calendars";

/// Fresh for `CACHE_FOR` while the day and the calendars match.
struct Cached {
    day: NaiveDate,
    calendars: Vec<String>,
    at: Instant,
    events: Vec<Event>,
}

/// The events from a day on, for the absence heads-ups.
struct CachedAhead {
    from: NaiveDate,
    days: u64,
    calendars: Vec<String>,
    at: Instant,
    events: Vec<CalendarEvent>,
}

#[derive(Default)]
struct Inner {
    store: Option<Store>,
    /// Today and the other day the Daybook window looked at last.
    cached: Vec<Cached>,
    ahead: Option<CachedAhead>,
}

impl Inner {
    fn store(&mut self) -> &mut Store {
        self.store.get_or_insert_with(Store::new)
    }

    fn clear_caches(&mut self) {
        self.cached.clear();
        self.ahead = None;
    }
}

fn has_access() -> bool {
    eventkit::access() == CalendarAccess::FullAccess
}

#[derive(Default)]
pub struct Calendar(std::sync::Mutex<Inner>);

impl Calendar {
    fn events(&self, day: NaiveDate, calendars: &[String], today: NaiveDate) -> Vec<Event> {
        if calendars.is_empty() || !has_access() {
            return Vec::new();
        }
        let mut inner = self.0.lock_unpoisoned();
        let fresh = inner.cached.iter().find(|cached| {
            cached.day == day && cached.calendars == calendars && cached.at.elapsed() < CACHE_FOR
        });
        if let Some(cached) = fresh {
            return cached.events.clone();
        }
        let start = day.and_time(NaiveTime::MIN);
        let end = start + Days::new(1);
        let events = events::timed_events(inner.store().events(calendars, start, end));
        inner
            .cached
            .retain(|cached| cached.day == today && cached.day != day);
        inner.cached.push(Cached {
            day,
            calendars: calendars.to_vec(),
            at: Instant::now(),
            events: events.clone(),
        });
        events
    }

    /// Every event from `from` through `days` more days, all-day ones too; canceled and declined ones
    /// included.
    fn ahead(&self, from: NaiveDate, days: u64, calendars: &[String]) -> Vec<CalendarEvent> {
        if calendars.is_empty() || !has_access() {
            return Vec::new();
        }
        let mut inner = self.0.lock_unpoisoned();
        let fresh = inner.ahead.as_ref().filter(|cached| {
            cached.from == from
                && cached.days == days
                && cached.calendars == calendars
                && cached.at.elapsed() < CACHE_FOR
        });
        if let Some(cached) = fresh {
            return cached.events.clone();
        }
        let start = from.and_time(NaiveTime::MIN);
        let events = inner
            .store()
            .events(calendars, start, start + Days::new(days + 1));
        inner.ahead = Some(CachedAhead {
            from,
            days,
            calendars: calendars.to_vec(),
            at: Instant::now(),
            events: events.clone(),
        });
        events
    }

    fn clear_cache(&self) {
        self.0.lock_unpoisoned().clear_caches();
    }

    /// A store made before access was granted shows no calendars.
    fn drop_store(&self) {
        let mut inner = self.0.lock_unpoisoned();
        inner.clear_caches();
        inner.store = None;
    }

    /// The titles of this Mac's event calendars; none without access.
    fn calendar_titles(&self) -> Vec<String> {
        if !has_access() {
            return Vec::new();
        }
        let mut titles = self.0.lock_unpoisoned().store().calendar_titles();
        titles.sort_by_key(|title| title.to_lowercase());
        titles.dedup();
        titles
    }
}

fn current_calendars(app: &AppHandle) -> Vec<String> {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    state
        .contexts
        .current(&settings)
        .map(|context| context.calendars.clone())
        .unwrap_or_default()
}

/// Reads the current context's calendars on a blocking thread; nothing when that fails.
async fn read_current<T: Send + 'static>(
    app: &AppHandle,
    read: impl FnOnce(&Calendar, &[String]) -> Vec<T> + Send + 'static,
) -> Vec<T> {
    let calendars = current_calendars(app);
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || read(app.state::<Calendar>().inner(), &calendars))
        .await
        .unwrap_or_else(|e| {
            log::warn!("Reading the calendar failed: {e}");
            Vec::new()
        })
}

/// The timed events of `day` in the current context's calendars.
pub async fn events_on(app: &AppHandle, day: NaiveDate, today: NaiveDate) -> Vec<Event> {
    read_current(app, move |calendar, calendars| {
        calendar.events(day, calendars, today)
    })
    .await
}

/// Every event in the current context's calendars from the start of `from` through `days` more days.
pub async fn events_ahead(app: &AppHandle, from: NaiveDate, days: u64) -> Vec<CalendarEvent> {
    read_current(app, move |calendar, calendars| {
        calendar.ahead(from, days, calendars)
    })
    .await
}

/// Settings → Calendar and the contexts' calendar lists.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct CalendarInfo {
    pub access: CalendarAccess,
    /// The titles of this Mac's event calendars; empty without full access.
    pub calendars: Vec<String>,
}

#[tauri::command]
pub async fn calendar_info(app: AppHandle) -> CalendarInfo {
    let read = move || app.state::<Calendar>().calendar_titles();
    let calendars = tauri::async_runtime::spawn_blocking(read)
        .await
        .unwrap_or_default();
    CalendarInfo {
        access: eventkit::access(),
        calendars,
    }
}

/// Asks macOS for access the first time; after a refusal it opens the privacy settings instead.
#[tauri::command]
pub async fn calendar_request_access(app: AppHandle) -> AppResult<CalendarAccess> {
    match eventkit::access() {
        CalendarAccess::NotDetermined => {
            let access = eventkit::request_access().await;
            app.state::<Calendar>().drop_store();
            changed(&app);
            Ok(access)
        }
        CalendarAccess::FullAccess => Ok(CalendarAccess::FullAccess),
        _ => {
            app.opener()
                .open_url(PRIVACY_SETTINGS, None::<&str>)
                .map_err(|e| AppError::Io(format!("Couldn't open the privacy settings: {e}")))?;
            Ok(eventkit::access())
        }
    }
}

/// Joins an event's call in its chosen app; only join links are opened.
#[tauri::command]
pub async fn calendar_join(app: AppHandle, url: String) -> AppResult<()> {
    let Some(link) = events::join_link(&url) else {
        return Err(AppError::InvalidInput("Not a meeting link".into()));
    };
    crate::calls::join(&app, link).await
}

fn changed(app: &AppHandle) {
    if let Err(e) = app.emit(CALENDAR_CHANGED_EVENT, ()) {
        log::warn!("Couldn't announce a calendar change: {e}");
    }
}

/// A changed event, calendar or access drops what was read and tells the windows.
pub fn install(app: &AppHandle) {
    app.manage(Calendar::default());
    let handle = app.clone();
    eventkit::on_change(move || {
        handle.state::<Calendar>().clear_cache();
        changed(&handle);
    });
}
