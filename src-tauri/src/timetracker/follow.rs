//! The thread that follows `timetracker watch --json`, or polls an older TimeTracker without it.

use std::sync::mpsc::{self, Receiver};
use std::thread;

use tauri::AppHandle;

use super::day::DayChange;
use super::reply::{DataChange, DataKind};
use super::watch::{self, Next, Reconnect, Sink, poll, state_after};
use super::{
    Tracking, WatchLink, blocking, cli, day_changed, load_projects_logged, service, set_link,
    set_tracking,
};
use crate::activity;
use crate::lock::MutexExt;

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

struct AppSink<'a>(&'a AppHandle);

impl Sink for AppSink<'_> {
    fn connected(&self) {
        set_link(self.0, WatchLink::Connected);
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
        set_link(app, WatchLink::Waiting);
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
    log::info!(
        "This TimeTracker has no `watch`; reading its timer every {} s",
        watch::POLL.as_secs()
    );
    set_link(app, WatchLink::Polling);
    loop {
        set_tracking(app, poll(&cli(app)));
        if woken.recv_timeout(watch::POLL).is_ok() {
            set_link(app, WatchLink::Waiting);
            return;
        }
    }
}

/// Starts the thread; a send on the service's `wake` ends a wait between connections.
pub(super) fn start(app: &AppHandle) {
    let (wake, woken) = mpsc::channel();
    *service(app).wake.lock_unpoisoned() = Some(wake);
    let handle = app.clone();
    let started = thread::Builder::new()
        .name("timetracker-watch".into())
        .spawn(move || watch_forever(&handle, &woken));
    if let Err(e) = started {
        log::warn!("Couldn't start following TimeTracker: {e}");
    }
}
