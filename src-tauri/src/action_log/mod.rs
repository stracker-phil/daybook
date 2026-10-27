//! The action log (docs/backend.md#action-log): what Daybook sent to Obsidian, Slack and TimeTracker,
//! grouped by what triggered it, kept in `action-log.json`. A trigger opens an event (`traced`,
//! `begin`); lines logged while it is current (`scope`) join it for two minutes. Writers log through
//! the free functions, which do nothing until `install` ran (tests, startup).

pub mod model;
mod scope;
pub mod view;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use chrono::{DateTime, Local};
use tauri::{AppHandle, Emitter, Manager};

pub use model::{ActionState, ActionTarget, LineRef};
pub use scope::{Trigger, carry, carry_sync, current, within};
pub use view::ActionLog as ActionLogView;

use self::model::{Log, OTHER};
use crate::error::AppResult;
use crate::events::ACTION_LOG_CHANGED_EVENT;
use crate::locale::{self, DateLocale};
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::store::json_file::JsonFile;

pub const FILE_NAME: &str = "action-log.json";
const SAVE_DELAY: Duration = Duration::from_secs(1);

type Listener = Box<dyn Fn() + Send + Sync>;

/// The log in memory, saved a moment after it changed.
pub struct ActionLog {
    log: Mutex<Log>,
    file: JsonFile,
    saving: AtomicBool,
    save_delay: Duration,
    listener: OnceLock<Listener>,
}

impl ActionLog {
    pub fn open(config_dir: &Path) -> Arc<Self> {
        Self::with_delay(config_dir, SAVE_DELAY)
    }

    pub fn with_delay(config_dir: &Path, save_delay: Duration) -> Arc<Self> {
        let file = JsonFile::new(config_dir.join(FILE_NAME));
        let mut log: Log = file.load();
        log.prune(Local::now());
        Arc::new(Self {
            log: Mutex::new(log),
            file,
            saving: AtomicBool::new(false),
            save_delay,
            listener: OnceLock::new(),
        })
    }

    /// Hears every change; set once.
    pub fn set_listener(&self, listener: impl Fn() + Send + Sync + 'static) {
        let _ = self.listener.set(Box::new(listener));
    }

    fn change<R>(self: &Arc<Self>, edit: impl FnOnce(&mut Log) -> R) -> R {
        let result = edit(&mut self.log.lock_unpoisoned());
        if let Some(listener) = self.listener.get() {
            listener();
        }
        self.save_soon();
        result
    }

    fn save_soon(self: &Arc<Self>) {
        if self.saving.swap(true, Ordering::SeqCst) {
            return;
        }
        let this = Arc::clone(self);
        thread::spawn(move || {
            thread::sleep(this.save_delay);
            this.saving.store(false, Ordering::SeqCst);
            this.flush();
        });
    }

    /// Writes the file now.
    pub fn flush(&self) {
        let saved = {
            let mut log = self.log.lock_unpoisoned();
            log.prune(Local::now());
            log.saved(Local::now())
        };
        if let Err(e) = self.file.save(&saved) {
            log::warn!("Couldn't save the action log: {e}");
        }
    }

    /// Opens an event at `now`, or a "Daybook" line in `parent`'s event while `parent` is open.
    pub fn begin(
        self: &Arc<Self>,
        title: &str,
        merge: bool,
        parent: Option<&Trigger>,
        now: DateTime<Local>,
    ) -> Trigger {
        self.change(|log| {
            if let Some(parent) = parent.filter(|parent| parent.open_at(now))
                && let Some(line) = log.add(
                    &parent.event,
                    ActionTarget::Daybook,
                    title,
                    ActionState::Done,
                    None,
                    now,
                )
            {
                return Trigger {
                    event: parent.event.clone(),
                    started: parent.started,
                    nested: Some(line),
                };
            }
            Trigger {
                event: log.open(title, merge, now),
                started: now,
                nested: None,
            }
        })
    }

    /// Adds a line to `trigger`'s event while it is open, else to a new event titled `title` (or
    /// "Other").
    #[allow(clippy::too_many_arguments)]
    pub fn line(
        self: &Arc<Self>,
        trigger: Option<&Trigger>,
        title: Option<&str>,
        target: ActionTarget,
        text: &str,
        state: ActionState,
        detail: Option<String>,
        now: DateTime<Local>,
    ) -> Option<LineRef> {
        self.change(|log| {
            let joined = trigger
                .filter(|trigger| trigger.open_at(now))
                .and_then(|trigger| {
                    log.add(&trigger.event, target, text, state, detail.clone(), now)
                });
            joined.or_else(|| {
                let event = log.open(title.unwrap_or(OTHER), false, now);
                log.add(&event, target, text, state, detail, now)
            })
        })
    }

    pub fn update(
        self: &Arc<Self>,
        line: &LineRef,
        state: ActionState,
        detail: Option<String>,
        now: DateTime<Local>,
    ) {
        self.change(|log| log.update(line, state, detail, now));
    }

    /// Renames the event, or the line of a nested trigger.
    pub fn rename(self: &Arc<Self>, trigger: &Trigger, title: &str) {
        self.change(|log| match &trigger.nested {
            Some(line) => log.rename_line(line, title),
            None => log.rename_event(&trigger.event, title),
        });
    }

    /// The event shows even without lines; nothing for a nested trigger.
    pub fn keep(self: &Arc<Self>, trigger: &Trigger) {
        if trigger.nested.is_none() {
            self.change(|log| log.keep(&trigger.event));
        }
    }

    pub fn view(&self, now: DateTime<Local>, locale: DateLocale) -> ActionLogView {
        view::action_log(&self.log.lock_unpoisoned(), now, locale)
    }

    pub fn clear(self: &Arc<Self>) -> AppResult<()> {
        self.change(|log| log.clear());
        // Saved now so the command reports a failed write.
        self.file.save(&Log::default())
    }
}

static LOG: OnceLock<Arc<ActionLog>> = OnceLock::new();

/// Logging starts: changes reach every window as `action_log_changed`.
pub fn install(app: &AppHandle) {
    let log = Arc::clone(&app.state::<AppState>().action_log);
    let handle = app.clone();
    log.set_listener(move || {
        if let Err(e) = handle.emit(ACTION_LOG_CHANGED_EVENT, ()) {
            log::warn!("Couldn't announce an action log change: {e}");
        }
    });
    let _ = LOG.set(log);
}

/// Saves what waits for the save delay (Daybook quits).
pub fn flush() {
    if let Some(log) = LOG.get() {
        log.flush();
    }
}

/// A logged line; `update` moves it on (sending → done, waiting → done when written).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Logged(Option<LineRef>);

impl Logged {
    pub fn update(&self, state: ActionState, detail: Option<String>) {
        if let (Some(log), Some(line)) = (LOG.get(), &self.0) {
            log.update(line, state, detail, Local::now());
        }
    }

    pub fn is_logged(&self) -> bool {
        self.0.is_some()
    }
}

fn open_trigger(title: &str, merge: bool) -> Trigger {
    let now = Local::now();
    match LOG.get() {
        Some(log) => log.begin(title, merge, current().as_ref(), now),
        None => Trigger {
            event: String::new(),
            started: now,
            nested: None,
        },
    }
}

/// Opens a trigger; lines join it while it is current (`Trigger::scope`, `Trigger::enter`).
pub fn begin(title: &str) -> Trigger {
    open_trigger(title, false)
}

/// Opens a trigger that counts as a repeat of the last event with its title (autosaves).
pub fn begin_merged(title: &str) -> Trigger {
    open_trigger(title, true)
}

impl Trigger {
    pub fn rename(&self, title: &str) {
        if let Some(log) = LOG.get() {
            log.rename(self, title);
        }
    }

    pub fn keep(&self) {
        if let Some(log) = LOG.get() {
            log.keep(self);
        }
    }
}

/// Runs `work` as the trigger `title`.
pub async fn traced<F: Future>(title: &str, work: F) -> F::Output {
    begin(title).scope(work).await
}

/// Runs `work` as a merged trigger `title` (autosaves).
pub async fn traced_merged<F: Future>(title: &str, work: F) -> F::Output {
    begin_merged(title).scope(work).await
}

/// Runs sync `work` as the trigger `title`.
pub fn traced_sync<R>(title: &str, work: impl FnOnce(&Trigger) -> R) -> R {
    let trigger = begin(title);
    let _entered = trigger.enter();
    work(&trigger)
}

/// Logs a line in the current trigger, else in an event titled `title` (or "Other").
pub fn log_titled(
    title: Option<&str>,
    target: ActionTarget,
    text: &str,
    state: ActionState,
    detail: Option<String>,
) -> Logged {
    Logged(LOG.get().and_then(|log| {
        log.line(
            current().as_ref(),
            title,
            target,
            text,
            state,
            detail,
            Local::now(),
        )
    }))
}

pub fn log(target: ActionTarget, text: &str, state: ActionState, detail: Option<String>) -> Logged {
    log_titled(None, target, text, state, detail)
}

pub fn done(target: ActionTarget, text: &str) -> Logged {
    log(target, text, ActionState::Done, None)
}

/// A line whose outcome `Logged::update` reports later.
pub fn sending(target: ActionTarget, text: &str) -> Logged {
    log(target, text, ActionState::Sending, None)
}

/// Spawns `work` on the async runtime with the current trigger.
pub fn spawn<F>(work: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tauri::async_runtime::spawn(carry(work));
}

#[tauri::command]
pub async fn action_log_get(app: AppHandle) -> ActionLogView {
    let locale = locale::system_async().await;
    app.state::<AppState>()
        .action_log
        .view(Local::now(), locale)
}

#[tauri::command]
pub fn action_log_clear(state: tauri::State<'_, AppState>) -> AppResult<()> {
    state.action_log.clear()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_config_dir;
    use chrono::Duration as ChronoDuration;
    use tempfile::TempDir;

    const NEVER: Duration = Duration::from_secs(3600);

    fn open_log(save_delay: Duration) -> (TempDir, Arc<ActionLog>) {
        let dir = temp_config_dir();
        let log = ActionLog::with_delay(dir.path(), save_delay);
        (dir, log)
    }

    fn minutes(n: i64) -> ChronoDuration {
        ChronoDuration::minutes(n)
    }

    fn done(
        log: &Arc<ActionLog>,
        trigger: Option<&Trigger>,
        text: &str,
        now: DateTime<Local>,
    ) -> Option<LineRef> {
        log.line(
            trigger,
            None,
            ActionTarget::Obsidian,
            text,
            ActionState::Done,
            None,
            now,
        )
    }

    fn en_gb() -> DateLocale {
        DateLocale::parse("en_GB")
    }

    fn lines_of(log: &ActionLog, now: DateTime<Local>) -> Vec<Vec<(ActionTarget, String)>> {
        log.view(now, en_gb())
            .events
            .into_iter()
            .map(|event| {
                event
                    .lines
                    .into_iter()
                    .map(|line| (line.target, line.text))
                    .collect()
            })
            .collect()
    }

    fn triggers_of(log: &ActionLog, now: DateTime<Local>) -> Vec<String> {
        log.view(now, en_gb())
            .events
            .into_iter()
            .map(|event| event.trigger)
            .collect()
    }

    #[test]
    fn begin_without_a_parent_opens_an_event_of_its_own() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let trigger = log.begin("Focus saved", false, None, now);
        assert_eq!(trigger.nested, None);
        assert_eq!(trigger.started, now);
        let line = done(&log, Some(&trigger), "Focus: 1 lines", now);
        assert_eq!(line.map(|line| line.index), Some(0));
        assert_eq!(triggers_of(&log, now), ["Focus saved"]);
    }

    #[test]
    fn begin_inside_an_open_parent_adds_a_daybook_line_to_the_parents_event() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let parent = log.begin("Context change", false, None, now);
        let child = log.begin("Status set: Off", false, Some(&parent), now + minutes(1));
        assert_eq!(child.event, parent.event);
        assert_eq!(child.started, parent.started);
        assert!(child.nested.is_some());
        assert_eq!(triggers_of(&log, now + minutes(1)), ["Context change"]);
        assert_eq!(
            lines_of(&log, now + minutes(1)),
            [[(ActionTarget::Daybook, "Status set: Off".to_string())]]
        );
    }

    #[test]
    fn lines_logged_in_a_nested_trigger_join_the_parents_event() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let parent = log.begin("Context change", false, None, now);
        let child = log.begin("Status set: Off", false, Some(&parent), now);
        done(&log, Some(&child), "Plan: 1 item", now + minutes(1));
        assert_eq!(triggers_of(&log, now + minutes(1)), ["Context change"]);
        assert_eq!(lines_of(&log, now + minutes(1))[0].len(), 2);
    }

    #[test]
    fn begin_after_the_parent_is_over_two_minutes_old_starts_a_new_event() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let parent = log.begin("Context change", false, None, now);
        let later = now + minutes(3);
        let child = log.begin("Status set: Off", false, Some(&parent), later);
        assert_ne!(child.event, parent.event);
        assert_eq!(child.nested, None);
        assert_eq!(child.started, later);
        done(&log, Some(&child), "Plan: 1 item", later);
        assert_eq!(triggers_of(&log, later), ["Status set: Off"]);
    }

    #[test]
    fn a_merged_begin_repeats_the_last_event_and_shows_its_count() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let first = log.begin("Journal saved: Morning", true, None, now);
        done(&log, Some(&first), "Journal: Morning", now);
        let second = log.begin("Journal saved: Morning", true, None, now + minutes(5));
        assert_eq!(second.event, first.event);
        let view = log.view(now + minutes(5), en_gb());
        assert_eq!(view.events.len(), 1);
        assert_eq!(view.events[0].lines[0].count, 2);
    }

    #[test]
    fn a_line_in_an_open_trigger_joins_its_event() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let trigger = log.begin("Focus saved", false, None, now);
        done(&log, Some(&trigger), "Focus: 2 lines", now + minutes(1));
        done(&log, Some(&trigger), "#team: focus list", now + minutes(1));
        let lines = lines_of(&log, now + minutes(1));
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), 2);
        assert_eq!(triggers_of(&log, now + minutes(1)), ["Focus saved"]);
    }

    #[test]
    fn a_line_without_a_trigger_goes_to_a_new_event_with_the_given_title() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let line = log.line(
            None,
            Some("Waiting entries"),
            ActionTarget::Obsidian,
            "Focus: 2 lines",
            ActionState::Done,
            None,
            now,
        );
        assert!(line.is_some());
        assert_eq!(triggers_of(&log, now), ["Waiting entries"]);
    }

    #[test]
    fn a_line_without_a_trigger_or_title_goes_to_an_event_called_other() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        done(&log, None, "Focus: 2 lines", now);
        assert_eq!(triggers_of(&log, now), ["Other"]);
    }

    #[test]
    fn a_line_after_its_trigger_closed_goes_to_a_new_event() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let trigger = log.begin("Focus saved", false, None, now);
        done(&log, Some(&trigger), "Focus: 2 lines", now);
        let later = now + minutes(3);
        log.line(
            Some(&trigger),
            Some("Retry"),
            ActionTarget::SlackMessage,
            "#team: focus list",
            ActionState::Done,
            None,
            later,
        );
        let mut titles = triggers_of(&log, later);
        titles.sort();
        assert_eq!(titles, ["Focus saved", "Retry"]);
    }

    #[test]
    fn the_same_line_logged_again_updates_instead_of_duplicating() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let trigger = log.begin("Focus saved", false, None, now);
        let first = done(&log, Some(&trigger), "Focus: 2 lines", now);
        let again = done(&log, Some(&trigger), "Focus: 2 lines", now);
        assert_eq!(first, again);
        assert_eq!(lines_of(&log, now)[0].len(), 1);
    }

    #[test]
    fn update_moves_a_logged_line_on() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let line = log
            .line(
                None,
                None,
                ActionTarget::SlackStatus,
                "Off",
                ActionState::Sending,
                None,
                now,
            )
            .unwrap();
        log.update(
            &line,
            ActionState::Failed,
            Some("Not connected".into()),
            now,
        );
        let logged = &log.view(now, en_gb()).events[0].lines[0];
        assert_eq!(logged.state, ActionState::Failed);
        assert_eq!(logged.detail.as_deref(), Some("Not connected"));
    }

    #[test]
    fn rename_on_an_event_trigger_renames_the_event() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let trigger = log.begin("Context change", false, None, now);
        done(&log, Some(&trigger), "Plan: 1 item", now);
        log.rename(&trigger, "Context: Work → Home");
        assert_eq!(triggers_of(&log, now), ["Context: Work → Home"]);
    }

    #[test]
    fn rename_on_a_nested_trigger_renames_its_line_only() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let parent = log.begin("Context change", false, None, now);
        let child = log.begin("Status set", false, Some(&parent), now);
        log.rename(&child, "Status set: Off");
        assert_eq!(triggers_of(&log, now), ["Context change"]);
        assert_eq!(
            lines_of(&log, now),
            [[(ActionTarget::Daybook, "Status set: Off".to_string())]]
        );
    }

    #[test]
    fn keep_makes_an_empty_event_visible() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let trigger = log.begin("Context change", false, None, now);
        assert!(log.view(now, en_gb()).events.is_empty());
        log.keep(&trigger);
        assert_eq!(triggers_of(&log, now), ["Context change"]);
        assert!(lines_of(&log, now)[0].is_empty());
    }

    #[test]
    fn keep_on_a_nested_trigger_leaves_the_parents_event_unkept() {
        let (_dir, log) = open_log(NEVER);
        let now = Local::now();
        let parent = log.begin("Context change", false, None, now);
        let child = log.begin("Status set", false, Some(&parent), now);
        log.keep(&child);
        assert!(!log.log.lock().unwrap().events[0].always);
    }

    #[test]
    fn clear_empties_memory_and_the_file() {
        let (dir, log) = open_log(NEVER);
        let now = Local::now();
        done(&log, None, "Focus: 2 lines", now);
        log.flush();
        let file = JsonFile::new(dir.path().join(FILE_NAME));
        assert_eq!(file.load::<Log>().events.len(), 1);

        log.clear().unwrap();

        assert!(log.view(Local::now(), en_gb()).events.is_empty());
        assert_eq!(std::fs::read_to_string(file.path()).unwrap().trim(), "[]");
    }

    #[test]
    fn the_debounced_save_writes_a_file_that_loads_back_equal() {
        let (dir, log) = open_log(Duration::from_millis(5));
        let now = Local::now();
        let trigger = log.begin("Focus saved", false, None, now);
        log.line(
            Some(&trigger),
            None,
            ActionTarget::SlackMessage,
            "#team: focus list",
            ActionState::Failed,
            Some("Not connected".into()),
            now,
        );
        let expected = log.log.lock().unwrap().saved(Local::now());
        let file = JsonFile::new(dir.path().join(FILE_NAME));
        let mut loaded = Log::default();
        for _ in 0..600 {
            loaded = file.load();
            if loaded == expected {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(expected.events.len(), 1);
        assert_eq!(loaded, expected);
    }

    #[test]
    fn a_saved_log_is_there_again_when_the_app_reopens() {
        let (dir, log) = open_log(NEVER);
        let now = Local::now();
        let trigger = log.begin("Focus saved", false, None, now);
        done(&log, Some(&trigger), "Focus: 2 lines", now);
        done(&log, None, "Plan: 1 item", now);
        log.flush();

        let reopened = ActionLog::with_delay(dir.path(), NEVER);

        assert_eq!(
            reopened.view(Local::now(), en_gb()),
            log.view(Local::now(), en_gb())
        );
        assert_eq!(reopened.view(Local::now(), en_gb()).events.len(), 2);
    }

    #[test]
    fn a_change_is_announced_to_the_listener() {
        let (_dir, log) = open_log(NEVER);
        let heard = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&heard);
        log.set_listener(move || flag.store(true, Ordering::SeqCst));
        done(&log, None, "Focus: 2 lines", Local::now());
        assert!(heard.load(Ordering::SeqCst));
    }
}
