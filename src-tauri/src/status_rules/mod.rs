//! Calendar absences → Slack heads-ups (docs/slack.md): checked every minute and when the calendar, the
//! settings or the context change. Slack hears only of a changed heads-up.

pub mod rules;

use std::ffi::CStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use chrono::{Local, NaiveDateTime, TimeZone};
use tauri::{AppHandle, Listener, Manager};

use crate::calendar;
use crate::events::{CALENDAR_CHANGED_EVENT, CONTEXT_CHANGED_EVENT, SETTINGS_CHANGED_EVENT};
use crate::slack::{self, HeadsUp};
use crate::state::AppState;

const TICK: Duration = Duration::from_secs(60);

/// A check runs at a time; one asked for meanwhile is dropped.
#[derive(Default)]
struct Checking(AtomicBool);

/// Clears `Checking` even when the check stops halfway.
struct Running<'a>(&'a AtomicBool);

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// The Mac's time zone abbreviation at `at` (`CEST`).
fn zone_of(at: NaiveDateTime) -> String {
    let Some(local) = Local.from_local_datetime(&at).earliest() else {
        return String::new();
    };
    let time: libc::time_t = local.timestamp();
    // SAFETY: `localtime_r` fills the zeroed `tm`; `tm_zone` then points at a static C string.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        if libc::localtime_r(&time, &mut tm).is_null() || tm.tm_zone.is_null() {
            return String::new();
        }
        CStr::from_ptr(tm.tm_zone).to_string_lossy().into_owned()
    }
}

/// The heads-up Slack should show now, in a context with Slack.
async fn current_heads_up(app: &AppHandle) -> Option<HeadsUp> {
    let settings = app.state::<AppState>().settings.get();
    let rules = &settings.status_rules;
    let days = rules::lookahead_days(rules)?;
    if !slack::in_context(app) {
        return None;
    }
    let now = Local::now().naive_local();
    let events = calendar::events_ahead(app, now.date(), days).await;
    rules::pick_heads_up(now, &events, rules, zone_of)
}

/// Tells Slack the heads-up that holds now.
async fn refresh(app: &AppHandle) {
    let checking = &app.state::<Checking>().inner().0;
    if checking.swap(true, Ordering::SeqCst) {
        return;
    }
    let _running = Running(checking);
    let heads_up = current_heads_up(app).await;
    slack::set_heads_up(app, heads_up);
}

fn refresh_soon(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { refresh(&app).await });
}

/// Checks every `TICK` and on calendar, context and settings changes.
pub fn start(app: &AppHandle) {
    app.manage(Checking::default());
    for event in [
        CALENDAR_CHANGED_EVENT,
        CONTEXT_CHANGED_EVENT,
        SETTINGS_CHANGED_EVENT,
    ] {
        let handle = app.clone();
        app.listen(event, move |_| refresh_soon(&handle));
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticks = tokio::time::interval(TICK);
        loop {
            ticks.tick().await;
            refresh(&app).await;
        }
    });
}
