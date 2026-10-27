//! The activity overlay and check-ins (docs/timetracker.md#the-activity-overlay): pick a project and type
//! what you work on, and TimeTracker's timer starts on it; the scheduler opens the overlay as a
//! check-in every `checkInMinutes`. Daybook never records time itself.

pub mod check_in;
pub mod commands;
pub mod pause;
pub mod pick;
pub mod prefill;
pub mod recent;
pub mod session;
pub mod suggest;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::{DateTime, Local, NaiveDateTime};
use tauri::{AppHandle, Listener, Manager};

use self::check_in::{CheckIns, Conditions, Moment};
use self::pick::Plan;
use self::prefill::Prefill;
use self::session::{ActivityMode, ActivitySession, SessionInput, build_session};
use crate::bus::{Command, CommandBus};
use crate::clock::is_within_window;
use crate::error::AppResult;
use crate::events::CONTEXT_CHANGED_EVENT;
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::store::settings::{Feature, Project, Settings, Status};
use crate::timetracker::reply::RecentTimer;
use crate::timetracker::{self, TtState, client};
use crate::tools::RecentActivity;
use crate::{calls, presence, projects, status, tray, window};

pub const VIEW: &str = "activity";
/// Recent timers this young aren't asked for again when the overlay opens.
const FRESH: Duration = Duration::from_secs(4);
/// The menu's recent timers are asked for again after this long; a change in TimeTracker drops them
/// sooner.
const STALE: Duration = Duration::from_secs(60);

struct Fetched {
    at: Instant,
    timers: Vec<RecentTimer>,
}

#[derive(Default)]
pub struct Activity {
    check_ins: Mutex<CheckIns>,
    recent: Mutex<Option<Fetched>>,
    fetching: AtomicBool,
}

fn activity(app: &AppHandle) -> &Activity {
    app.state::<Activity>().inner()
}

/// The projects that count in the current context.
fn offered(settings: &Settings, app: &AppHandle) -> Vec<Project> {
    let state = app.state::<AppState>();
    let context = state.contexts.current(settings).map(|c| c.id.clone());
    projects::current_projects(&settings.projects, context.as_deref())
        .into_iter()
        .cloned()
        .collect()
}

fn conditions(app: &AppHandle) -> Conditions {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let context = state.contexts.current(&settings);
    let now = Local::now().naive_local();
    Conditions {
        has_vault: settings.vault.path.is_some(),
        working: status::today(app).status == Some(Status::Working),
        status_feature: context.is_some_and(|c| c.features.contains(&Feature::Status)),
        check_in_minutes: context.map_or(0, |c| c.check_in_minutes),
        in_work_hours: context
            .and_then(|c| c.work_hours.as_ref())
            .is_none_or(|hours| is_within_window(&hours.days, &hours.from, &hours.to, now)),
        paused_today: paused_today(app),
        tt: timetracker::tracking(app).tt,
    }
}

pub fn paused_today(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    state.check_in_pause.is_paused(state.notes.today())
}

/// The scheduler's tick: opens the check-in when it is due.
pub async fn tick(app: &AppHandle) {
    let conditions = conditions(app);
    let moment = Moment {
        in_call: calls::active(app).is_some(),
        away: presence::is_away(app),
        overlay_open: window::panel::view(app).is_some(),
    };
    let due = activity(app)
        .check_ins
        .lock_unpoisoned()
        .tick(&conditions, &moment, Instant::now());
    if due && let Err(e) = open(app, ActivityMode::Checkin, None).await {
        log::warn!("Opening the check-in failed: {e}");
    }
}

/// The next check-in is a full interval away.
pub fn restart_check_ins(app: &AppHandle) {
    let conditions = conditions(app);
    activity(app)
        .check_ins
        .lock_unpoisoned()
        .restart(&conditions, Instant::now());
}

/// The recent timers are asked for again next time.
pub fn forget_recent(app: &AppHandle) {
    *activity(app).recent.lock_unpoisoned() = None;
}

/// TimeTracker's recordings changed: the recent timers are asked for again.
pub fn recordings_changed(app: &AppHandle) {
    forget_recent(app);
    tray::refresh(app);
}

/// When the running timer started, else `now`: meetings since then are suggested.
fn since_timer_started(app: &AppHandle, now: NaiveDateTime) -> NaiveDateTime {
    timetracker::tracking(app)
        .timer
        .and_then(|timer| DateTime::from_timestamp_millis(timer.started_at))
        .map_or(now, |started| started.with_timezone(&Local).naive_local())
}

/// After ↵: Working when a timer started during a break or after the day, the next check-in a full
/// interval away, and the overlay closed.
fn finish_pick(app: &AppHandle, plan: &Plan) {
    if matches!(plan, Plan::Start { .. })
        && status::today(app).status != Some(Status::Working)
        && let Err(e) = status::set_working_by_activity(app)
    {
        log::warn!("Couldn't set Working for the activity: {e}");
    }
    restart_check_ins(app);
    if window::panel::view(app).as_deref() == Some(VIEW) {
        window::panel::close(app, false);
    }
}

fn session_now(app: &AppHandle, mode: ActivityMode, prefill: Option<&Prefill>) -> ActivitySession {
    let settings = app.state::<AppState>().settings.get();
    let offered = offered(&settings, app);
    let offered: Vec<&Project> = offered.iter().collect();
    let tracking = timetracker::tracking(app);
    let (tt, view) = timetracker::tool_state(&tracking, &settings);
    let listed = timetracker::cached_projects(app);
    build_session(&SessionInput {
        mode,
        offered: &offered,
        listed: listed.as_deref(),
        tt,
        timer: tracking.timer.as_ref(),
        timer_text: view.map(|view| view.text()),
        prefill,
    })
}

/// Opens the activity overlay; the same request again closes it.
pub async fn open(app: &AppHandle, mode: ActivityMode, prefill: Option<Prefill>) -> AppResult<()> {
    let session = session_now(app, mode, prefill.as_ref());
    window::panel::open(app, VIEW, serde_json::to_value(session)?).await;
    Ok(())
}

fn fresh_recent(app: &AppHandle) -> Option<Vec<RecentTimer>> {
    activity(app)
        .recent
        .lock_unpoisoned()
        .as_ref()
        .filter(|fetched| fetched.at.elapsed() < FRESH)
        .map(|fetched| fetched.timers.clone())
}

/// Asks TimeTracker for its recent timers and keeps them; `true` when they changed.
async fn fetch_recent(app: &AppHandle) -> bool {
    let timers = timetracker::blocking(app, |tool| client::recent(tool, recent::LIMIT))
        .await
        .unwrap_or_else(|e| {
            log::warn!("Couldn't list TimeTracker's recent timers: {e}");
            Vec::new()
        });
    let mut cached = activity(app).recent.lock_unpoisoned();
    let changed = cached
        .as_ref()
        .is_none_or(|fetched| fetched.timers != timers);
    *cached = Some(Fetched {
        at: Instant::now(),
        timers,
    });
    changed
}

/// The recent timers, asked for again when older than `FRESH`; none while TimeTracker isn't running.
async fn recent_timers(app: &AppHandle) -> Vec<RecentTimer> {
    if timetracker::tracking(app).tt != TtState::Available {
        return Vec::new();
    }
    if let Some(timers) = fresh_recent(app) {
        return timers;
    }
    fetch_recent(app).await;
    fresh_recent(app).unwrap_or_default()
}

/// The tools' recent activities from the timers listed last; missing or stale ones are asked for
/// again in the background and the menu follows.
pub fn tool_activities(app: &AppHandle, offered: &[&Project]) -> Vec<RecentActivity> {
    let available = timetracker::tracking(app).tt == TtState::Available;
    if !available {
        return Vec::new();
    }
    let service = activity(app);
    let (timers, stale) = match service.recent.lock_unpoisoned().as_ref() {
        Some(fetched) => (fetched.timers.clone(), fetched.at.elapsed() >= STALE),
        None => (Vec::new(), true),
    };
    if stale && !service.fetching.swap(true, Ordering::SeqCst) {
        let app = app.clone();
        tauri::async_runtime::spawn(async move {
            let changed = fetch_recent(&app).await;
            activity(&app).fetching.store(false, Ordering::SeqCst);
            if changed {
                tray::refresh(&app);
            }
        });
    }
    recent::recent_activities(&timers, offered)
}

async fn toggle_pause(app: &AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let paused = state.check_in_pause.toggle(state.notes.today())?;
    log::info!(
        "Check-ins {} for today",
        if paused { "paused" } else { "resumed" }
    );
    activity(app).check_ins.lock_unpoisoned().reset();
    tray::refresh(app);
    Ok(())
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("activity:showTab", "Day Activity")
            .help("Show TimeTracker's day in the Daybook window's inspector")
            .run_now(window::show_activity_tab),
    );
    bus.add(
        Command::new("activity:switch", "Log Activity…")
            .help("Start TimeTracker's timer on what you work on")
            .run(|app| async move { open(&app, ActivityMode::Switch, None).await }),
    );
    bus.add_family(
        Command::new(prefill::PREFIX, "Log Activity… with this filled in")
            .family("<project> <activity...>")
            .from_words(prefill::argument_from_words)
            .run(|app, argument| async move {
                let prefill = prefill::parse_prefill(&argument);
                open(&app, ActivityMode::Switch, Some(prefill)).await
            }),
    );
    bus.add(
        Command::new("checkin:pause", "Pause Check-ins Today")
            .help("Pause or resume today’s check-ins")
            .run(|app| async move { toggle_pause(&app).await }),
    );
}

/// Another context brings its own interval.
pub fn install(app: &AppHandle) {
    let handle = app.clone();
    app.listen(CONTEXT_CHANGED_EVENT, move |_| {
        activity(&handle).check_ins.lock_unpoisoned().reset();
    });
}
