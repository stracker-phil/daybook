//! TimeTracker (docs/timetracker.md): Daybook reads and drives it through its `timetracker` tool and
//! never tracks time itself. One thread follows `timetracker watch --json`, so the timer and whether
//! TimeTracker runs are known live; the menu-bar title, the rail, the sidebar, Slack and the link rules
//! follow through `timer_changed`. The status starts and stops the timer; finished calls become
//! suggestions (`drive`).

mod call_suggestion;
pub mod client;
pub mod commands;
pub mod day;
mod drive;
pub mod link;
pub mod reply;
mod status_timer;
pub mod watch;

use std::process::Child;
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

use chrono::{Local, NaiveDate};
use tauri::{AppHandle, Emitter, Listener, Manager};

pub use commands::register;
use commands::{TimeTrackerInfo, TimeTrackerProjects};
pub use watch::{Tracking, TtState};

use self::client::{Cli, resolve_path, state_of};
use self::day::{DayChange, TimeTrackerDay};
use self::reply::{DataChange, DataKind, TtProject};
use self::watch::{Next, Reconnect, Sink, poll, state_after};
use crate::error::{AppError, AppResult};
use crate::events::{ACTIVITY_CHANGED_EVENT, CONTEXT_CHANGED_EVENT, TIMER_CHANGED_EVENT};
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::store::settings::Settings;
use crate::tools::TimerView;
use crate::{activity, projects, slack, tray};

#[derive(Default)]
pub struct TimeTrackerService {
    tracking: Mutex<Tracking>,
    /// `projects --archived`, as listed last.
    projects: Mutex<Option<Vec<TtProject>>>,
    /// The day the Activity tab read last.
    shown_day: Mutex<Option<NaiveDate>>,
    /// The Daybook project of the timer as Slack heard it last.
    project: Mutex<Option<String>>,
    watching: Arc<Mutex<Option<Child>>>,
    /// Ends a wait between connections (the path changed).
    wake: Mutex<Option<Sender<()>>>,
}

fn service(app: &AppHandle) -> &TimeTrackerService {
    app.state::<TimeTrackerService>().inner()
}

/// The tool at the path from Settings.
pub fn cli(app: &AppHandle) -> Cli {
    let settings = app.state::<AppState>().settings.get();
    Cli::new(
        resolve_path(&settings.timetracker.path),
        Arc::clone(&service(app).watching),
    )
}

/// Runs blocking work with the tool off the async runtime.
pub async fn blocking<T: Send + 'static>(
    app: &AppHandle,
    work: impl FnOnce(&Cli) -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    let tool = cli(app);
    tauri::async_runtime::spawn_blocking(move || work(&tool))
        .await
        .map_err(|_| AppError::TimeTrackerNotRunning(client::NO_ANSWER.into()))?
}

pub fn tracking(app: &AppHandle) -> Tracking {
    service(app).tracking.lock_unpoisoned().clone()
}

/// The projects listed last; `None` before the first list.
pub fn cached_projects(app: &AppHandle) -> Option<Vec<TtProject>> {
    service(app).projects.lock_unpoisoned().clone()
}

/// What the tools show of TimeTracker.
pub fn tool_state(tracking: &Tracking, settings: &Settings) -> (TtState, Option<TimerView>) {
    let timer = tracking
        .timer
        .as_ref()
        .map(|timer| link::timer_view(&settings.projects, timer));
    (tracking.tt, timer)
}

/// The project the timer runs on, among the projects that count now.
pub fn current_project(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let context = state.contexts.current(&settings).map(|c| c.id.clone());
    let offered = projects::current_projects(&settings.projects, context.as_deref());
    link::current_project(&offered, tracking(app).timer.as_ref())
}

fn set_tracking(app: &AppHandle, next: Tracking) {
    let (tt_changed, timer_changed) = {
        let mut tracking = service(app).tracking.lock_unpoisoned();
        if *tracking == next {
            return;
        }
        let tt_changed = tracking.tt != next.tt;
        if tt_changed {
            log::info!("TimeTracker: {:?}", next.tt);
        }
        let timer_changed = tracking.timer != next.timer;
        *tracking = next.clone();
        (tt_changed, timer_changed)
    };
    if let Err(e) = app.emit(TIMER_CHANGED_EVENT, &next) {
        log::warn!("Couldn't announce the timer change: {e}");
    }
    activity::forget_recent(app);
    if timer_changed {
        activity::restart_check_ins(app);
    }
    tray::refresh(app);
    follow_project(app);
    day_changed(
        app,
        DayChange::Tracking {
            tt_changed,
            timer_changed,
        },
    );
}

/// Tells the Activity tab to read its day again when `change` touches it.
fn day_changed(app: &AppHandle, change: DayChange) {
    let Some(shown) = *service(app).shown_day.lock_unpoisoned() else {
        return;
    };
    if day::reloads(change, shown, Local::now().date_naive())
        && let Err(e) = app.emit(ACTIVITY_CHANGED_EVENT, ())
    {
        log::warn!("Couldn't tell the Activity tab of a change: {e}");
    }
}

/// The Activity tab's day; projects are listed first when they never were, for their colours.
pub async fn day_for_page(app: &AppHandle, day: NaiveDate) -> AppResult<TimeTrackerDay> {
    *service(app).shown_day.lock_unpoisoned() = Some(day);
    let projects = match cached_projects(app) {
        Some(projects) => projects,
        None => load_projects_logged(app).await.unwrap_or_default(),
    };
    let now = Local::now().fixed_offset();
    blocking(app, move |tool| day::read_day(tool, &projects, day, now)).await
}

/// Slack hears of the timer's project when it changed.
fn follow_project(app: &AppHandle) {
    let project = current_project(app);
    {
        let mut last = service(app).project.lock_unpoisoned();
        if *last == project {
            return;
        }
        last.clone_from(&project);
    }
    slack::project_changed(app, project);
}

/// Links projects by name the first time projects are listed; whether it did.
fn auto_link_once(settings: &mut Settings, listed: &[TtProject]) -> bool {
    if settings.timetracker.linked || listed.is_empty() {
        return false;
    }
    let unmatched = link::auto_link(&mut settings.projects, listed);
    log::info!(
        "Linked projects to TimeTracker's by name; none for: {}",
        if unmatched.is_empty() {
            "–".to_string()
        } else {
            unmatched.join(", ")
        }
    );
    settings.timetracker.linked = true;
    true
}

/// Links projects by name once and follows renamed TimeTracker projects.
async fn link_projects(app: &AppHandle, listed: &[TtProject]) {
    let state = app.state::<AppState>();
    let mut settings = state.settings.get();
    let renamed = link::follow_names(&mut settings.projects, listed);
    let linked = auto_link_once(&mut settings, listed);
    if (renamed || linked)
        && let Err(e) = crate::commands::save_settings(app, &state, settings).await
    {
        log::warn!("Couldn't save the TimeTracker links: {e}");
    }
}

/// Lists the projects again, keeps them and follows their names.
pub async fn load_projects(app: &AppHandle) -> AppResult<Vec<TtProject>> {
    let listed = blocking(app, |tool| client::projects(tool)).await?;
    *service(app).projects.lock_unpoisoned() = Some(listed.clone());
    link_projects(app, &listed).await;
    Ok(listed)
}

async fn load_projects_logged(app: &AppHandle) -> Option<Vec<TtProject>> {
    load_projects(app)
        .await
        .inspect_err(|e| log::warn!("Couldn't list TimeTracker's projects: {e}"))
        .ok()
}

fn refresh_projects_soon(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        load_projects_logged(&app).await;
    });
}

/// A renamed project sends no timer line, so the timer is read again.
fn read_timer_soon(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(next) = blocking(&app, |tool| Ok(poll(tool))).await {
            set_tracking(&app, next);
        }
    });
}

/// Settings → TimeTracker: where the tool is, whether it runs, its version.
pub async fn info(app: &AppHandle) -> TimeTrackerInfo {
    let path = cli(app).path().display().to_string();
    let default_path = client::DEFAULT_PATH.to_string();
    match blocking(app, |tool| client::status(tool)).await {
        Ok(status) => TimeTrackerInfo {
            path,
            default_path,
            state: TtState::Available,
            version: Some(status.version),
        },
        Err(e) => TimeTrackerInfo {
            path,
            default_path,
            state: state_of(&e),
            version: None,
        },
    }
}

/// The picker's projects: the cached list, or a new one when `fresh` or never listed.
pub async fn projects_for_page(app: &AppHandle, fresh: bool) -> TimeTrackerProjects {
    if !fresh && let Some(projects) = cached_projects(app) {
        return TimeTrackerProjects {
            state: tracking(app).tt,
            projects,
        };
    }
    match load_projects(app).await {
        Ok(projects) => TimeTrackerProjects {
            state: TtState::Available,
            projects,
        },
        Err(e) => TimeTrackerProjects {
            state: state_of(&e),
            projects: cached_projects(app).unwrap_or_default(),
        },
    }
}

struct AppSink<'a>(&'a AppHandle);

impl Sink for AppSink<'_> {
    fn connected(&self) {
        refresh_projects_soon(self.0);
    }

    fn tracking(&self, tracking: Tracking) {
        set_tracking(self.0, tracking);
    }

    fn data_changed(&self, change: DataChange) {
        match change.kind {
            DataKind::Projects => {
                // The tab reads the day once the new colours are in.
                let app = self.0.clone();
                tauri::async_runtime::spawn(async move {
                    load_projects_logged(&app).await;
                    day_changed(&app, DayChange::Data(change));
                });
                read_timer_soon(self.0);
            }
            DataKind::Recordings => {
                activity::recordings_changed(self.0);
                day_changed(self.0, DayChange::Data(change));
            }
            DataKind::Tags | DataKind::Rules => day_changed(self.0, DayChange::Data(change)),
            DataKind::Other => {}
        }
    }
}

fn watch_forever(app: &AppHandle, woken: &Receiver<()>) {
    let mut reconnect = Reconnect::default();
    loop {
        let (end, had_lines) = watch::follow(&cli(app), &AppSink(app));
        if let Some(gone) = state_after(end) {
            set_tracking(app, gone);
        }
        match reconnect.after(end, had_lines) {
            Next::Retry(delay) => {
                let _ = woken.recv_timeout(delay);
            }
            Next::Poll => poll_until_woken(app, woken),
        }
    }
}

/// An older TimeTracker without `watch`: its timer every `POLL`, until the path changes.
fn poll_until_woken(app: &AppHandle, woken: &Receiver<()>) {
    log::info!("This TimeTracker has no `watch`; reading its timer every 30 s");
    loop {
        set_tracking(app, poll(&cli(app)));
        if woken.recv_timeout(watch::POLL).is_ok() {
            return;
        }
    }
}

/// Starts following TimeTracker.
pub fn install(app: &AppHandle) {
    let (wake, woken) = mpsc::channel();
    *service(app).wake.lock_unpoisoned() = Some(wake);
    let handle = app.clone();
    let started = thread::Builder::new()
        .name("timetracker-watch".into())
        .spawn(move || watch_forever(&handle, &woken));
    if let Err(e) = started {
        log::warn!("Couldn't start following TimeTracker: {e}");
    }
    // A context switch can change which project counts.
    let handle = app.clone();
    app.listen(CONTEXT_CHANGED_EVENT, move |_| follow_project(&handle));
    drive::install(app);
}

/// A new path is tried at once; changed links move the timer's project.
pub fn settings_saved(app: &AppHandle, previous: &Settings, saved: &Settings) {
    if previous.timetracker.path != saved.timetracker.path {
        client::stop_watching(&service(app).watching);
        if let Some(wake) = service(app).wake.lock_unpoisoned().as_ref() {
            let _ = wake.send(());
        }
    }
    if previous.projects != saved.projects || previous.contexts != saved.contexts {
        follow_project(app);
    }
}

/// Daybook quits: the `watch` ends with it.
pub fn stop(app: &AppHandle) {
    client::stop_watching(&service(app).watching);
}
