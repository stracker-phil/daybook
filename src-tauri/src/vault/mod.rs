//! The vault: where daily notes are, reading them, and the one write path into them. Changes for
//! today's note wait on disk while the note doesn't exist yet, and Obsidian is asked to create it.

pub mod commands;
mod create;
pub mod location;
pub mod note;
pub mod obsidian;
pub mod ops;
pub mod pending;
pub mod watch;
pub mod writer;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

use chrono::NaiveDate;
use serde::Serialize;
use ts_rs::TS;

pub use self::create::Timings;
use self::location::{DailyNoteFormat, FormatCache};
use self::obsidian::ObsidianCli;
use self::ops::{NoteLayout, NoteOp};
use self::pending::{ALREADY, Drained, Pending};
use self::watch::FolderWatch;
use self::writer::{NoteWriter, WriteError};
use crate::action_log::{self, ActionState, ActionTarget, Logged};
use crate::error::{AppError, AppResult};
use crate::lock::MutexExt;
use crate::store::settings::Settings;

const NO_VAULT: &str = "Choose your vault in Settings first.";
const WAITS_FOR_NOTE: &str = "Written once today’s note exists";

/// Today's note: where it is, whether it exists and what waits for it.
#[derive(Clone, Debug, PartialEq)]
pub struct DailyNoteState {
    /// `None` without a vault.
    pub path: Option<String>,
    pub exists: bool,
    /// Obsidian is creating it.
    pub creating: bool,
    /// Changes waiting for it (or for an earlier day's note).
    pub waiting: u32,
}

/// Where the daily notes are and whether today's note and the Obsidian CLI were found.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct VaultInfo {
    /// `None` without a vault.
    pub daily_notes: Option<DailyNoteFormat>,
    pub today_note: Option<String>,
    pub today_note_exists: bool,
    pub obsidian_cli: Option<String>,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum WriteOutcome {
    Written,
    /// The note already says this.
    Unchanged,
    /// Written once the note exists or stops changing.
    Queued,
}

/// What the listener hears.
#[derive(Debug, Clone, PartialEq)]
pub enum NoteEvent {
    /// Notes in the watched folder changed.
    Changed(Vec<PathBuf>),
    /// Waiting changes or the creating state changed.
    StateChanged,
}

type Listener = Box<dyn Fn(NoteEvent) + Send + Sync>;

pub struct DailyNotes {
    writer: NoteWriter,
    pending: Pending,
    formats: FormatCache,
    cli: ObsidianCli,
    creating: AtomicBool,
    watch: Mutex<Option<FolderWatch>>,
    listener: OnceLock<Listener>,
    today: fn() -> NaiveDate,
    timings: Timings,
}

fn local_today() -> NaiveDate {
    chrono::Local::now().date_naive()
}

pub(crate) fn vault(settings: &Settings) -> AppResult<PathBuf> {
    settings
        .vault
        .path
        .as_deref()
        .map(PathBuf::from)
        .ok_or_else(|| AppError::InvalidInput(NO_VAULT.into()))
}

impl From<WriteError> for AppError {
    fn from(e: WriteError) -> Self {
        match e {
            WriteError::Io { .. } => AppError::Io(e.to_string()),
            _ => AppError::InvalidInput(e.to_string()),
        }
    }
}

impl DailyNotes {
    pub fn open(config_dir: &Path) -> Self {
        Self::with_clock(config_dir, local_today, Timings::default())
    }

    /// `today` and `timings` replace the wall clock and the waits for Obsidian.
    pub fn with_clock(config_dir: &Path, today: fn() -> NaiveDate, timings: Timings) -> Self {
        Self {
            writer: NoteWriter::default(),
            pending: Pending::open(config_dir),
            formats: FormatCache::default(),
            cli: ObsidianCli::default(),
            creating: AtomicBool::new(false),
            watch: Mutex::new(None),
            listener: OnceLock::new(),
            today,
            timings,
        }
    }

    /// Set once; later calls are ignored.
    pub fn set_listener(&self, listener: impl Fn(NoteEvent) + Send + Sync + 'static) {
        let _ = self.listener.set(Box::new(listener));
    }

    fn notify(&self, event: NoteEvent) {
        if let Some(listener) = self.listener.get() {
            listener(event);
        }
    }

    pub fn today(&self) -> NaiveDate {
        (self.today)()
    }

    /// The vault and where its daily notes are.
    pub fn vault_and_format(&self, settings: &Settings) -> AppResult<(PathBuf, DailyNoteFormat)> {
        let vault = vault(settings)?;
        let format = self.formats.get(&vault, &settings.daily_note);
        Ok((vault, format))
    }

    /// Settings → Connections → Vault's view; looking up the Obsidian CLI can take seconds.
    pub fn info(&self, settings: &Settings) -> VaultInfo {
        let today = self.state(settings);
        VaultInfo {
            daily_notes: self
                .vault_and_format(settings)
                .ok()
                .map(|(_, format)| format),
            today_note: today.path,
            today_note_exists: today.exists,
            obsidian_cli: self
                .cli
                .find(&settings.vault.obsidian_cli_path)
                .map(|path| path.to_string_lossy().into_owned()),
        }
    }

    /// Where `day`'s note belongs, whether or not it exists.
    pub fn note_path(&self, settings: &Settings, day: NaiveDate) -> AppResult<PathBuf> {
        let (vault, format) = self.vault_and_format(settings)?;
        Ok(format.note_path(&vault, day))
    }

    pub fn today_path(&self, settings: &Settings) -> AppResult<PathBuf> {
        self.note_path(settings, self.today())
    }

    /// `day`'s note where it is, also in its year's archive folder; `None` when that day has none.
    pub fn find_note(&self, settings: &Settings, day: NaiveDate) -> AppResult<Option<PathBuf>> {
        let (vault, format) = self.vault_and_format(settings)?;
        let candidates = [
            format.note_path(&vault, day),
            format.archived_path(&vault, day),
        ];
        Ok(candidates.into_iter().find(|path| path.is_file()))
    }

    /// `day`'s note text; `None` when that day has none.
    pub fn read_note(&self, settings: &Settings, day: NaiveDate) -> AppResult<Option<String>> {
        let Some(file) = self.find_note(settings, day)? else {
            return Ok(None);
        };
        std::fs::read_to_string(&file)
            .map(Some)
            .map_err(|e| AppError::Io(format!("Couldn't read {}: {e}", file.display())))
    }

    pub fn state(&self, settings: &Settings) -> DailyNoteState {
        let path = self.today_path(settings).ok();
        DailyNoteState {
            exists: path.as_deref().is_some_and(Path::is_file),
            path: path.map(|path| path.to_string_lossy().into_owned()),
            creating: self.creating.load(Ordering::SeqCst),
            waiting: u32::try_from(self.pending.count()).unwrap_or(u32::MAX),
        }
    }

    /// Applies `op` to today's note, or queues it while the note doesn't exist yet.
    pub fn edit_today(
        self: &Arc<Self>,
        settings: &Settings,
        op: NoteOp,
    ) -> AppResult<WriteOutcome> {
        let vault = vault(settings)?;
        let file = self.today_path(settings)?;
        self.watch_daily_notes(settings);
        let exists = file.is_file();
        let logged = action_log::sending(ActionTarget::Obsidian, &op.describe());
        if self.pending.has_for(&file) || !exists {
            logged.update(ActionState::Waiting, Some(WAITS_FOR_NOTE.into()));
            self.queue(&file, op, logged)?;
            if exists {
                self.drain(settings);
            } else {
                self.spawn_create_today(settings);
            }
            return Ok(WriteOutcome::Queued);
        }
        match self.write_now(settings, &vault, &file, &op, &logged) {
            Ok(outcome) => Ok(outcome),
            Err(e @ WriteError::Conflict(_)) => {
                log::warn!("{e} The change waits.");
                logged.update(ActionState::Waiting, Some(format!("{e} Retried later.")));
                self.queue(&file, op, logged)?;
                Ok(WriteOutcome::Queued)
            }
            Err(e) => {
                logged.update(ActionState::Failed, Some(e.to_string()));
                Err(e.into())
            }
        }
    }

    /// Applies `op` to `file`, an existing note under the canonical vault; never queues.
    pub fn edit_file(
        &self,
        settings: &Settings,
        file: &Path,
        op: &NoteOp,
    ) -> AppResult<WriteOutcome> {
        let vault = vault(settings)?;
        let vault = vault.canonicalize().unwrap_or(vault);
        let logged = action_log::sending(ActionTarget::Obsidian, &op.describe());
        self.write_now(settings, &vault, file, op, &logged)
            .map_err(|e| {
                logged.update(ActionState::Failed, Some(e.to_string()));
                e.into()
            })
    }

    /// Writes `op` into `file` and logs a write or no change; an error is the caller's to log.
    fn write_now(
        &self,
        settings: &Settings,
        vault: &Path,
        file: &Path,
        op: &NoteOp,
        logged: &Logged,
    ) -> Result<WriteOutcome, WriteError> {
        let layout = NoteLayout::from_settings(settings);
        let mut edit = |text: &str| op.apply(text, &layout);
        if self.writer.edit_note(vault, file, &mut edit)? {
            logged.update(ActionState::Done, None);
            Ok(WriteOutcome::Written)
        } else {
            logged.update(ActionState::Skipped, Some(ALREADY.into()));
            Ok(WriteOutcome::Unchanged)
        }
    }

    /// Obsidian creates the note, because its template needs Templater, which only runs inside
    /// Obsidian.
    fn spawn_create_today(self: &Arc<Self>, settings: &Settings) {
        let this = Arc::clone(self);
        let settings = settings.clone();
        thread::spawn(action_log::carry_sync(move || {
            if let Err(e) = this.create_today(&settings) {
                log::warn!("Couldn't create today's note: {e}");
            }
        }));
    }

    fn queue(&self, file: &Path, op: NoteOp, logged: Logged) -> AppResult<()> {
        self.pending.enqueue_logged(file, op, logged)?;
        self.notify(NoteEvent::StateChanged);
        Ok(())
    }

    /// Writes the waiting changes whose note can take them.
    pub fn drain(&self, settings: &Settings) -> Drained {
        let Ok(vault) = vault(settings) else {
            return Drained::default();
        };
        let drained =
            self.pending
                .drain(&self.writer, &vault, &NoteLayout::from_settings(settings));
        if drained.written > 0 || !drained.dropped.is_empty() {
            self.notify(NoteEvent::StateChanged);
        }
        drained
    }

    /// Writes the waiting changes once their note exists and Templater is done with it: when a note
    /// appears, and on the scheduler's tick.
    pub fn note_appeared(&self, settings: &Settings) {
        if self.pending.count() == 0 || self.creating.load(Ordering::SeqCst) {
            return;
        }
        for file in self.pending.files() {
            if file.is_file() {
                self.wait_until_quiet(&file);
            }
        }
        self.drain(settings);
    }

    /// Follows the daily-notes folder and everything below it; a missing folder is watched once it exists.
    pub fn watch_daily_notes(self: &Arc<Self>, settings: &Settings) {
        let folder = self
            .vault_and_format(settings)
            .ok()
            .map(|(vault, format)| format.folder_in(&vault));
        let mut watch = self.watch.lock_unpoisoned();
        if watch.as_ref().map(FolderWatch::folder) == folder.as_deref() {
            return;
        }
        *watch = None;
        let Some(folder) = folder.filter(|folder| folder.is_dir()) else {
            return;
        };
        let this = Arc::downgrade(self);
        let started = FolderWatch::start(&folder, true, watch::is_note, move |paths| {
            if let Some(this) = this.upgrade() {
                this.notify(NoteEvent::Changed(paths));
            }
        });
        match started {
            Ok(started) => *watch = Some(started),
            Err(e) => log::warn!("Can't watch {}: {e}", folder.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::time::{Duration, Instant};

    use tempfile::TempDir;

    use super::*;
    use crate::test_support::temp_config_dir;

    fn fixed_today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 24).unwrap()
    }

    fn fast() -> Timings {
        Timings {
            appear: Duration::from_secs(2),
            poll: Duration::from_millis(5),
            settle: Duration::from_millis(5),
            max_settle_checks: 10,
        }
    }

    fn step(title: &str) -> NoteOp {
        NoteOp::PlanStep {
            title: title.into(),
            done: true,
        }
    }

    struct Fixture {
        config: TempDir,
        vault: TempDir,
        notes: Arc<DailyNotes>,
        settings: Settings,
    }

    impl Fixture {
        /// `queued` entries wait for today's note before the service opens.
        fn new(queued: &[NoteOp], timings: Timings) -> Self {
            let config = temp_config_dir();
            let vault = tempfile::tempdir().unwrap();
            let mut settings = Settings::default();
            settings.vault.path = Some(vault.path().to_string_lossy().into_owned());
            settings.vault.name = "Test Vault".into();
            settings.vault.obsidian_cli_path = "/nonexistent/obsidian".into();
            let note = vault.path().join("1-Journal").join("2026-10-24.md");
            let seed = Pending::open(config.path());
            for op in queued {
                seed.enqueue_logged(&note, op.clone(), Logged::default())
                    .unwrap();
            }
            drop(seed);
            let notes = Arc::new(DailyNotes::with_clock(config.path(), fixed_today, timings));
            Self {
                config,
                vault,
                notes,
                settings,
            }
        }

        fn note(&self) -> PathBuf {
            self.vault.path().join("1-Journal").join("2026-10-24.md")
        }

        fn write_note(&self, text: &str) {
            fs::create_dir_all(self.note().parent().unwrap()).unwrap();
            fs::write(self.note(), text).unwrap();
        }

        /// A CLI that runs `body`; it is the vault's configured Obsidian CLI from now on.
        fn install_cli(&mut self, body: &str) {
            let script = self.config.path().join("obsidian");
            fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
            self.settings.vault.obsidian_cli_path = script.to_string_lossy().into_owned();
        }

        /// A CLI that creates today's note like Obsidian does, and insists on its arguments.
        fn install_creating_cli(&mut self) {
            let note = self.note();
            let folder = note.parent().unwrap().display().to_string();
            self.install_cli(&format!(
                "[ \"$1\" = \"vault=Test Vault\" ] && [ \"$2\" = \"daily\" ] || exit 2\n\
                 mkdir -p '{folder}' && printf '# Day\\n' > '{}'",
                note.display()
            ));
        }

        fn expected(&self, ops: &[NoteOp]) -> String {
            let layout = NoteLayout::from_settings(&self.settings);
            let mut text = "# Day\n".to_string();
            for op in ops {
                if let Some(next) = op.apply(&text, &layout).unwrap() {
                    text = next;
                }
            }
            text
        }

        fn note_text(&self) -> Option<String> {
            fs::read_to_string(self.note()).ok()
        }
    }

    fn eventually(what: &str, mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(5);
        while !condition() {
            assert!(Instant::now() < deadline, "timed out waiting for {what}");
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn without_a_vault_path_editing_and_locating_today_fail_with_invalid_input() {
        let config = temp_config_dir();
        let notes = Arc::new(DailyNotes::with_clock(config.path(), fixed_today, fast()));
        let settings = Settings::default();

        let edit = notes.edit_today(&settings, step("Stretch"));
        let path = notes.today_path(&settings);

        assert!(
            matches!(edit, Err(AppError::InvalidInput(m)) if m == "Choose your vault in Settings first.")
        );
        assert!(
            matches!(path, Err(AppError::InvalidInput(m)) if m == "Choose your vault in Settings first.")
        );
    }

    #[test]
    fn info_reports_the_fallback_format_and_whether_today_has_a_note() {
        let mut fx = Fixture::new(&[], fast());
        fx.install_cli("exit 0");
        let cli = fx.settings.vault.obsidian_cli_path.clone();
        let note = Some(fx.note().to_string_lossy().into_owned());
        let format = DailyNoteFormat {
            folder: "1-Journal".into(),
            format: "YYYY-MM-DD".into(),
            from_obsidian: false,
        };

        let before = fx.notes.info(&fx.settings);
        fx.write_note("# Day\n");
        let after = fx.notes.info(&fx.settings);

        assert_eq!(
            before,
            VaultInfo {
                daily_notes: Some(format.clone()),
                today_note: note.clone(),
                today_note_exists: false,
                obsidian_cli: Some(cli.clone()),
            }
        );
        assert_eq!(
            after,
            VaultInfo {
                daily_notes: Some(format),
                today_note: note,
                today_note_exists: true,
                obsidian_cli: Some(cli),
            }
        );
    }

    #[test]
    fn info_without_a_vault_has_no_daily_notes_and_no_today_note() {
        let config = temp_config_dir();
        let notes = DailyNotes::with_clock(config.path(), fixed_today, fast());

        let info = notes.info(&Settings::default());

        assert_eq!(info.daily_notes, None);
        assert_eq!(info.today_note, None);
        assert!(!info.today_note_exists);
    }

    #[test]
    fn the_default_note_is_in_the_journal_folder_named_by_date() {
        let fx = Fixture::new(&[], fast());

        assert_eq!(fx.notes.today_path(&fx.settings).unwrap(), fx.note());
    }

    #[test]
    fn editing_an_existing_note_writes_and_a_repeat_changes_nothing() {
        let fx = Fixture::new(&[], fast());
        fx.write_note("# Day\n");

        let first = fx.notes.edit_today(&fx.settings, step("Stretch")).unwrap();
        let second = fx.notes.edit_today(&fx.settings, step("Stretch")).unwrap();

        assert_eq!(first, WriteOutcome::Written);
        assert_eq!(second, WriteOutcome::Unchanged);
        assert_eq!(fx.note_text().unwrap(), fx.expected(&[step("Stretch")]));
        assert_eq!(fx.notes.state(&fx.settings).waiting, 0);
    }

    #[test]
    fn editing_a_missing_note_queues_the_change() {
        let fx = Fixture::new(&[], fast());

        let outcome = fx.notes.edit_today(&fx.settings, step("Stretch")).unwrap();

        assert_eq!(outcome, WriteOutcome::Queued);
        let state = fx.notes.state(&fx.settings);
        assert_eq!(state.waiting, 1);
        assert!(!state.exists);
        assert_eq!(state.path, Some(fx.note().to_string_lossy().into_owned()));
        assert!(!fx.note().exists());
    }

    #[test]
    fn queued_changes_are_written_in_order_once_the_cli_has_created_the_note() {
        let mut fx = Fixture::new(&[], fast());
        fx.install_creating_cli();

        let first = fx.notes.edit_today(&fx.settings, step("One")).unwrap();
        let second = fx.notes.edit_today(&fx.settings, step("Two")).unwrap();

        assert_eq!(first, WriteOutcome::Queued);
        assert!(matches!(
            second,
            WriteOutcome::Queued | WriteOutcome::Written
        ));
        let expected = fx.expected(&[step("One"), step("Two")]);
        eventually("the queued lines in today's note", || {
            fx.note_text().as_deref() == Some(expected.as_str())
                && fx.notes.state(&fx.settings).waiting == 0
        });
    }

    #[test]
    fn creating_today_with_the_cli_creates_the_note_and_writes_the_waiting_changes() {
        let mut fx = Fixture::new(&[step("One"), step("Two")], fast());
        fx.install_creating_cli();
        assert_eq!(fx.notes.state(&fx.settings).waiting, 2);

        let exists = fx.notes.create_today(&fx.settings).unwrap();

        assert!(exists);
        assert_eq!(
            fx.note_text().unwrap(),
            fx.expected(&[step("One"), step("Two")])
        );
        let state = fx.notes.state(&fx.settings);
        assert_eq!(state.waiting, 0);
        assert!(state.exists);
        assert!(!state.creating);
    }

    #[test]
    fn a_failing_cli_is_an_error_and_the_changes_keep_waiting() {
        let mut fx = Fixture::new(&[step("One")], fast());
        fx.install_cli("echo 'vault not open' >&2\nexit 3");

        let result = fx.notes.create_today(&fx.settings);

        assert!(matches!(result, Err(AppError::Io(m)) if m == "vault not open"));
        let state = fx.notes.state(&fx.settings);
        assert_eq!(state.waiting, 1);
        assert!(!state.exists);
        assert!(!state.creating);
    }

    #[test]
    fn a_missing_cli_is_an_error_and_the_changes_keep_waiting() {
        let fx = Fixture::new(&[step("One")], fast());

        let result = fx.notes.create_today(&fx.settings);

        assert!(matches!(result, Err(AppError::Io(_))));
        assert_eq!(fx.notes.state(&fx.settings).waiting, 1);
    }

    #[test]
    fn a_cli_that_creates_nothing_is_an_error_once_the_wait_for_the_note_is_over() {
        let timings = Timings {
            appear: Duration::from_millis(50),
            ..fast()
        };
        let mut fx = Fixture::new(&[step("One")], timings);
        fx.install_cli("exit 0");

        let result = fx.notes.create_today(&fx.settings);

        assert!(matches!(result, Err(AppError::Io(_))));
        let state = fx.notes.state(&fx.settings);
        assert_eq!(state.waiting, 1);
        assert!(!state.exists);
        assert!(!state.creating);
    }

    #[test]
    fn a_note_created_by_hand_gets_the_waiting_changes() {
        let fx = Fixture::new(&[step("One"), step("Two")], fast());
        fx.write_note("# Day\n");

        fx.notes.note_appeared(&fx.settings);

        assert_eq!(
            fx.note_text().unwrap(),
            fx.expected(&[step("One"), step("Two")])
        );
        assert_eq!(fx.notes.state(&fx.settings).waiting, 0);
    }

    #[test]
    fn find_note_looks_in_the_year_archive_and_a_day_without_a_note_has_none() {
        let fx = Fixture::new(&[], fast());
        let day = fixed_today();
        let archived = fx
            .vault
            .path()
            .join("1-Journal")
            .join("2026")
            .join("2026-10-24.md");
        fs::create_dir_all(archived.parent().unwrap()).unwrap();
        fs::write(&archived, "archived text\n").unwrap();
        let other_day = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();

        assert_eq!(
            fx.notes.find_note(&fx.settings, day).unwrap(),
            Some(archived)
        );
        assert_eq!(
            fx.notes.read_note(&fx.settings, day).unwrap().as_deref(),
            Some("archived text\n")
        );
        assert_eq!(fx.notes.find_note(&fx.settings, other_day).unwrap(), None);
        assert_eq!(fx.notes.read_note(&fx.settings, other_day).unwrap(), None);
    }

    #[test]
    fn find_note_prefers_the_note_in_the_daily_folder() {
        let fx = Fixture::new(&[], fast());
        fx.write_note("current\n");

        assert_eq!(
            fx.notes.find_note(&fx.settings, fixed_today()).unwrap(),
            Some(fx.note())
        );
        assert_eq!(
            fx.notes
                .read_note(&fx.settings, fixed_today())
                .unwrap()
                .as_deref(),
            Some("current\n")
        );
    }

    fn tick(line: u32, text: &str) -> NoteOp {
        NoteOp::TodoDone {
            line,
            text: text.into(),
        }
    }

    #[test]
    fn ticking_a_todo_in_a_note_of_the_vault_writes_the_tick() {
        let fx = Fixture::new(&[], fast());
        fx.write_note("# Day\n\n- [ ] Call Anna\n");
        let file = fx.note().canonicalize().unwrap();

        let outcome = fx
            .notes
            .edit_file(&fx.settings, &file, &tick(2, "Call Anna"))
            .unwrap();

        assert_eq!(outcome, WriteOutcome::Written);
        assert_eq!(fx.note_text().unwrap(), "# Day\n\n- [x] Call Anna\n");
    }

    #[test]
    fn ticking_a_todo_whose_line_changed_is_refused_and_writes_nothing() {
        let fx = Fixture::new(&[], fast());
        fx.write_note("# Day\n\n- [ ] Call Bob\n");
        let file = fx.note().canonicalize().unwrap();

        let result = fx
            .notes
            .edit_file(&fx.settings, &file, &tick(2, "Call Anna"));

        assert_eq!(
            result.unwrap_err(),
            AppError::InvalidInput(crate::vault::ops::todo::TODO_CHANGED.into())
        );
        assert_eq!(fx.note_text().unwrap(), "# Day\n\n- [ ] Call Bob\n");
    }

    #[test]
    fn ticking_a_todo_of_a_file_outside_the_vault_is_refused() {
        let fx = Fixture::new(&[], fast());
        fx.write_note("# Day\n");
        let elsewhere = tempfile::tempdir().unwrap();
        let outside = elsewhere.path().canonicalize().unwrap().join("a.md");
        fs::write(&outside, "- [ ] Call Anna\n").unwrap();

        let result = fx
            .notes
            .edit_file(&fx.settings, &outside, &tick(0, "Call Anna"));

        assert!(matches!(result, Err(AppError::InvalidInput(_))));
        assert_eq!(fs::read_to_string(&outside).unwrap(), "- [ ] Call Anna\n");
    }
}
