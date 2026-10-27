//! The call check: every `TICK` while the Mac is awake (also at the lock screen), when the owner asks,
//! after a wake, and after a change of the settings or the context. One check at a time.

use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use chrono::{DateTime, Local, NaiveDateTime};
use tauri::{AppHandle, Emitter, Listener, Manager};
use tokio::sync::watch as signal;

use super::checks::{GENERIC_TITLE, call_title};
use super::detect::{Detection, Found, Missing, RuleCheck, detect, unknown_checks};
use super::probe::MacProbe;
use super::titles::EventTitles;
use super::tracker::{CallEvent, EndReason, MAX_FAILURES, Mode, Seen};
use super::{Announced, calls, detecting};
use crate::action_log::{self, ActionState, ActionTarget};
use crate::calendar::{self, events::running_event};
use crate::events::{CALL_CHANGED_EVENT, CONTEXT_CHANGED_EVENT};
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::store::settings::MeetingRule;
use crate::{notify, presence, tray};

const TICK: Duration = Duration::from_secs(15);

pub(super) struct Watch {
    /// A regular check skips while one runs; any other waits and runs after it.
    checking: tokio::sync::Mutex<()>,
    /// When the Mac fell asleep, until the first check after the wake.
    woke_from: Mutex<Option<NaiveDateTime>>,
    /// Whether a check has run since the start.
    checked: signal::Sender<bool>,
    pub status: Mutex<Status>,
    probe: MacProbe,
    titles: Mutex<EventTitles>,
}

impl Default for Watch {
    fn default() -> Self {
        Self {
            checking: tokio::sync::Mutex::default(),
            woke_from: Mutex::default(),
            checked: signal::channel(false).0,
            status: Mutex::default(),
            probe: MacProbe::default(),
            titles: Mutex::default(),
        }
    }
}

/// What the checks learned, for Settings.
#[derive(Default)]
pub(super) struct Status {
    /// Permissions a check was refused and hasn't used since.
    pub missing: Vec<Missing>,
    /// The first failing detector's error in the last check.
    pub error: Option<String>,
}

impl Status {
    fn record(&mut self, detection: &Detection) {
        self.missing.retain(|m| !detection.granted.contains(m));
        for m in &detection.missing {
            if !self.missing.contains(m) {
                self.missing.push(*m);
            }
        }
        let error = detection.error().map(String::from);
        if let Some(e) = &error
            && self.error.as_ref() != Some(e)
        {
            log::warn!("Checking for a call failed: {e}");
        }
        self.error = error;
    }
}

/// Why a check runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Why {
    /// The regular tick; skips while a check runs.
    Tick,
    /// Something changed that can change the answer.
    Soon,
    /// The owner asked: a missing call ends at once.
    Asked,
}

async fn detect_now(app: &AppHandle, rules: Vec<MeetingRule>, active: Option<String>) -> Detection {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        detect(&rules, &calls(&handle).watch.probe, active.as_deref())
    })
    .await
    .unwrap_or_else(|e| {
        // The rules moved into the failed task; a panic is rare enough to read them again.
        let rules = app.state::<AppState>().settings.get().meetings.rules;
        Detection {
            checks: unknown_checks(&rules, &e.to_string()),
            ..Detection::default()
        }
    })
}

/// The calendar event running now, asked once per event (`EventTitles`).
async fn event_title(app: &AppHandle, bundle_id: &str, now: NaiveDateTime) -> Option<String> {
    if let Some(cached) = calls(app)
        .watch
        .titles
        .lock_unpoisoned()
        .get(bundle_id, now)
    {
        return cached;
    }
    let events = calendar::events_on(app, now.date(), now.date()).await;
    let running = running_event(&events, now);
    calls(app).watch.titles.lock_unpoisoned().remember(
        bundle_id,
        running.map(|event| (event.title.as_str(), event.end)),
        now,
    );
    running.map(|event| event.title.clone())
}

/// A call its app calls just "Meeting" takes the title of the calendar event running now.
async fn with_event_titles(app: &AppHandle, mut checks: Vec<RuleCheck>) -> Vec<RuleCheck> {
    let now = Local::now().naive_local();
    for check in &mut checks {
        if let Found::Call(title) = &check.found
            && title == GENERIC_TITLE
        {
            let event = event_title(app, &check.bundle_id, now).await;
            check.found = Found::Call(call_title(title, event.as_deref()));
        }
    }
    checks
}

/// What the detectors see now, recorded for Settings.
async fn observe_detection(app: &AppHandle) -> Seen {
    let settings = app.state::<AppState>().settings.get();
    if !detecting(&settings) {
        return Seen::Off;
    }
    let state = calls(app);
    let active = state
        .tracker
        .lock_unpoisoned()
        .active()
        .map(|call| call.bundle_id.clone());
    let detection = detect_now(app, settings.meetings.rules, active).await;
    state.watch.status.lock_unpoisoned().record(&detection);
    Seen::Checked(with_event_titles(app, detection.checks).await)
}

/// The first check after a wake counts as one, whatever started it.
fn mode_for(watch: &Watch, why: Why) -> Mode {
    match (watch.woke_from.lock_unpoisoned().take(), why) {
        (Some(asleep_since), _) => Mode::Woke { asleep_since },
        (None, Why::Asked) => Mode::Asked,
        (None, _) => Mode::Regular,
    }
}

async fn check(app: &AppHandle, why: Why) {
    let state = calls(app);
    let _one_at_a_time = if why == Why::Tick {
        let Ok(guard) = state.watch.checking.try_lock() else {
            return;
        };
        guard
    } else {
        state.watch.checking.lock().await
    };
    let seen = observe_detection(app).await;
    let mode = mode_for(&state.watch, why);
    let now = Local::now().naive_local();
    let changes = state
        .tracker
        .lock_unpoisoned()
        .observe(seen.clone(), now, mode);
    if !changes.is_empty() {
        announce(app, changes, &seen);
    }
    state.watch.checked.send_replace(true);
}

/// Returns once the first check since the start has run; `active` then tells whether a call runs.
pub async fn first_check(app: &AppHandle) {
    let mut checked = calls(app).watch.checked.subscribe();
    let _ = checked.wait_for(|done| *done).await;
}

/// The action-log trigger of a change, with its log line.
fn traced(change: &CallEvent) -> Option<action_log::Trigger> {
    match change {
        CallEvent::Started(call) => {
            log::info!("In a {} call: {}", call.app, call.title);
            Some(action_log::begin(&format!("Call started: {}", call.app)))
        }
        CallEvent::Changed(call) => {
            log::info!("The {} call is now: {}", call.app, call.title);
            None
        }
        CallEvent::Switched { from, to, end } => {
            log::info!(
                "The call moved from {} (ended {}) to {}: {}",
                from.app,
                end.format("%H:%M:%S"),
                to.app,
                to.title
            );
            Some(action_log::begin(&format!(
                "Call moved: {} → {}",
                from.app, to.app
            )))
        }
        CallEvent::Ended { call, end, .. } => {
            log::info!("The {} call ended at {}", call.app, end.format("%H:%M:%S"));
            Some(action_log::begin(&format!("Call ended: {}", call.app)))
        }
    }
}

/// Why the check of `bundle_id` failed in this round.
fn check_error<'a>(seen: &'a Seen, bundle_id: &str) -> Option<&'a str> {
    let Seen::Checked(checks) = seen else {
        return None;
    };
    checks.iter().find_map(|check| match &check.found {
        Found::Unknown(error) if check.bundle_id == bundle_id => Some(error.as_str()),
        _ => None,
    })
}

/// The active call's detector kept failing: logged, in the action log and once as a notification.
fn failing(app: &AppHandle, name: &str, error: &str, trigger: &action_log::Trigger) {
    let text = format!("{name}'s call check failed {MAX_FAILURES} times in a row");
    log::warn!("{text}; the call counts as ended: {error}");
    let _entered = trigger.enter();
    action_log::log(
        ActionTarget::Daybook,
        &text,
        ActionState::Failed,
        Some(error.to_string()),
    );
    notify::show(
        app,
        "Call check failing",
        &format!("{text}, so the call counts as ended. {error}"),
    );
}

fn announce(app: &AppHandle, changes: Vec<CallEvent>, seen: &Seen) {
    for change in changes {
        let trigger = traced(&change);
        if let (
            CallEvent::Ended {
                call,
                reason: EndReason::Failing,
                ..
            },
            Some(trigger),
        ) = (&change, &trigger)
        {
            let error = check_error(seen, &call.bundle_id).unwrap_or_default();
            failing(app, &call.app, error, trigger);
        }
        // Nobody may be listening yet.
        let _ = calls(app).events.send(Announced {
            event: change,
            trigger,
        });
    }
    tray::refresh(app);
    if let Err(e) = app.emit(CALL_CHANGED_EVENT, ()) {
        log::warn!("Couldn't announce the call change: {e}");
    }
}

fn spawn_check(app: &AppHandle, why: Why) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { check(&app, why).await });
}

/// A check after something changed that can change the answer; it waits for a running one.
pub fn check_soon(app: &AppHandle) {
    spawn_check(app, Why::Soon);
}

/// Runs a check as if the owner asked.
pub fn check_now(app: &AppHandle) {
    spawn_check(app, Why::Asked);
}

/// Asked for by `meetings:check`; returns once it ran.
pub async fn check_asked(app: &AppHandle) {
    check(app, Why::Asked).await;
}

/// The Mac woke from a sleep that began at `asleep_since`: a call that is gone now ended then.
pub fn woke(app: &AppHandle, asleep_since: SystemTime) {
    let asleep_since = DateTime::<Local>::from(asleep_since).naive_local();
    *calls(app).watch.woke_from.lock_unpoisoned() = Some(asleep_since);
    spawn_check(app, Why::Soon);
}

/// Checks every `TICK` while the Mac is awake, at the lock screen too (no UI is involved); while
/// asleep it only checks whether the Mac is awake again. A context change checks at once.
pub fn start(app: &AppHandle) {
    let handle = app.clone();
    app.listen(CONTEXT_CHANGED_EVENT, move |_| check_soon(&handle));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticks = tokio::time::interval(TICK);
        loop {
            ticks.tick().await;
            tick(&app).await;
        }
    });
}

async fn tick(app: &AppHandle) {
    if presence::is_away(app) {
        presence::verify(app).await;
    }
    if !presence::is_asleep(app) {
        check(app, Why::Tick).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule_check(bundle_id: &str, found: Found) -> RuleCheck {
        RuleCheck {
            bundle_id: bundle_id.into(),
            app: bundle_id.into(),
            found,
        }
    }

    fn unknown(bundle_id: &str, error: &str) -> RuleCheck {
        rule_check(bundle_id, Found::Unknown(error.into()))
    }

    #[test]
    fn the_active_call_gets_its_own_check_error_when_two_rules_fail() {
        let seen = Seen::Checked(vec![
            unknown("us.zoom.xos", "zoom: no permission"),
            unknown("com.microsoft.teams2", "teams: no answer"),
        ]);
        for (bundle_id, expected) in [
            ("us.zoom.xos", "zoom: no permission"),
            ("com.microsoft.teams2", "teams: no answer"),
        ] {
            assert_eq!(check_error(&seen, bundle_id), Some(expected), "{bundle_id}");
        }
    }

    #[test]
    fn an_active_rule_that_answered_has_no_check_error() {
        let seen = Seen::Checked(vec![
            unknown("us.zoom.xos", "zoom: no permission"),
            rule_check("com.microsoft.teams2", Found::Call("Standup".into())),
            rule_check("com.apple.FaceTime", Found::NotInCall),
        ]);
        for bundle_id in ["com.microsoft.teams2", "com.apple.FaceTime"] {
            assert_eq!(check_error(&seen, bundle_id), None, "{bundle_id}");
        }
    }

    #[test]
    fn an_active_rule_absent_from_the_round_has_no_check_error() {
        let seen = Seen::Checked(vec![unknown("us.zoom.xos", "zoom: no permission")]);
        assert_eq!(check_error(&seen, "com.microsoft.teams2"), None);
    }

    #[test]
    fn a_round_without_checks_has_no_check_error() {
        assert_eq!(check_error(&Seen::Off, "us.zoom.xos"), None);
    }
}
