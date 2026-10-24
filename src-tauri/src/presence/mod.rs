//! Whether the owner is at the Mac: asleep or locked is away (docs/presence-and-calls.md). macOS's own
//! events go missing at times, so every wake reads the lock state, and a locked screen is read again
//! every two seconds until it is unlocked.

pub mod checks;
mod native;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use tauri::{AppHandle, Manager};

use checks::{Away, after_wake, has_been_asleep_long_enough, is_coming_back};
use native::SystemEvent;

use crate::lock::MutexExt;
use crate::{calls, tray, window};

const LOCK_POLL: Duration = Duration::from_secs(2);
/// Lets macOS finish leaving the lock screen before the overlay takes the focus.
const REFOCUS_DELAY: Duration = Duration::from_millis(500);

#[derive(Default)]
struct Inner {
    away: Away,
    /// Wall-clock time, which runs on while the Mac sleeps.
    asleep_since: Option<SystemTime>,
}

#[derive(Default)]
pub struct Presence {
    inner: Mutex<Inner>,
    /// One check at a time; callers that arrive meanwhile wait for it.
    checking: tokio::sync::Mutex<()>,
    polling: AtomicBool,
}

fn presence(app: &AppHandle) -> &Presence {
    app.state::<Presence>().inner()
}

fn away(app: &AppHandle) -> Away {
    presence(app).inner.lock_unpoisoned().away
}

/// No automatic overlay may open while away.
pub fn is_away(app: &AppHandle) -> bool {
    away(app).is_away()
}

fn set_away(app: &AppHandle, next: Away) {
    let before = {
        let mut inner = presence(app).inner.lock_unpoisoned();
        let before = inner.away;
        inner.away = next;
        if next.asleep && !before.asleep {
            inner.asleep_since = Some(SystemTime::now());
        }
        before
    };
    if before == next {
        return;
    }
    log::info!("Presence: {}", label(next));
    if next.locked {
        poll_while_locked(app);
    }
    if is_coming_back(before, next) {
        came_back(app);
    }
}

fn label(away: Away) -> &'static str {
    match (away.asleep, away.locked) {
        (true, _) => "asleep",
        (false, true) => "locked",
        (false, false) => "back",
    }
}

async fn screen_locked() -> bool {
    tauri::async_runtime::spawn_blocking(native::screen_locked)
        .await
        .unwrap_or(true)
}

/// Awake again, announced or noticed: asleep ends, and the lock state decides the rest.
async fn wake(app: &AppHandle, how: &str) {
    let locked = screen_locked().await;
    if !away(app).asleep {
        return;
    }
    log::info!(
        "Awake again ({how}), screen {}",
        if locked { "locked" } else { "unlocked" }
    );
    set_away(app, after_wake(locked));
}

async fn check(app: &AppHandle) {
    let (state, asleep_since) = {
        let inner = presence(app).inner.lock_unpoisoned();
        (inner.away, inner.asleep_since)
    };
    if state.asleep {
        let asleep_for = asleep_since
            .and_then(|since| since.elapsed().ok())
            .unwrap_or_default();
        if has_been_asleep_long_enough(asleep_for) {
            wake(app, "no wake event").await;
        }
        return;
    }
    if state.locked && !screen_locked().await {
        clear_stale_lock(app);
    }
}

/// The screen was found unlocked; a sleep or lock meanwhile wins.
fn clear_stale_lock(app: &AppHandle) {
    let now = away(app);
    if now.locked && !now.asleep {
        log::info!("Unlocked without an unlock event");
        set_away(
            app,
            Away {
                locked: false,
                ..now
            },
        );
    }
}

/// Clears what no longer holds when macOS didn't say so; without it prompts would stay off for good.
/// Called on every call check while away, and every two seconds while locked.
pub async fn verify(app: &AppHandle) {
    if !is_away(app) {
        return;
    }
    let _one_at_a_time = presence(app).checking.lock().await;
    check(app).await;
}

fn poll_while_locked(app: &AppHandle) {
    if presence(app).polling.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while away(&app).locked {
            tokio::time::sleep(LOCK_POLL).await;
            verify(&app).await;
        }
        presence(&app).polling.store(false, Ordering::SeqCst);
    });
}

/// Back at the screen: look for a call, refresh the menu bar and give a waiting overlay the focus.
fn came_back(app: &AppHandle) {
    calls::check_soon(app);
    tray::refresh(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(REFOCUS_DELAY).await;
        window::panel::refocus(&app).await;
    });
}

fn on_event(app: &AppHandle, event: SystemEvent) {
    let now = away(app);
    match event {
        SystemEvent::Sleep => set_away(
            app,
            Away {
                asleep: true,
                ..now
            },
        ),
        SystemEvent::Wake => {
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                let _one_at_a_time = presence(&app).checking.lock().await;
                wake(&app, "wake event").await;
            });
        }
        SystemEvent::Lock => set_away(
            app,
            Away {
                locked: true,
                ..now
            },
        ),
        // While asleep the lock state is settled at the wake.
        SystemEvent::Unlock if !now.asleep => set_away(
            app,
            Away {
                locked: false,
                ..now
            },
        ),
        SystemEvent::Unlock => {}
    }
}

pub fn install(app: &AppHandle) {
    let handle = app.clone();
    native::observe(move |event| on_event(&handle, event));
}
