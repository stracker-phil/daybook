//! Creating today's note through the Obsidian CLI (`obsidian vault=<name> daily`), so Obsidian's
//! template and Templater fill it, then writing the changes that waited for it.

use std::path::Path;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use super::{DailyNotes, NoteEvent};
use crate::action_log::{self, ActionState, ActionTarget};
use crate::error::{AppError, AppResult};
use crate::store::settings::Settings;

/// How long to wait for Obsidian.
#[derive(Clone, Copy, Debug)]
pub struct Timings {
    /// For the note to appear after the CLI returned.
    pub appear: Duration,
    pub poll: Duration,
    /// A new note is filled by Templater right after Obsidian creates it; it counts as done once its
    /// size and modification time stay the same for this long.
    pub settle: Duration,
    pub max_settle_checks: u32,
}

impl Default for Timings {
    fn default() -> Self {
        Self {
            appear: Duration::from_secs(15),
            poll: Duration::from_millis(500),
            settle: Duration::from_millis(1500),
            max_settle_checks: 10,
        }
    }
}

/// Clears the creating flag however creation ends.
struct Creating<'a>(&'a DailyNotes);

impl Drop for Creating<'_> {
    fn drop(&mut self) {
        self.0.creating.store(false, Ordering::SeqCst);
        self.0.notify(NoteEvent::StateChanged);
    }
}

impl DailyNotes {
    /// Creates today's note if it is missing, writes the waiting changes and returns whether the note
    /// exists. A call while another runs waits for it.
    pub fn create_today(&self, settings: &Settings) -> AppResult<bool> {
        let file = self.today_path(settings)?;
        if self.creating.swap(true, Ordering::SeqCst) {
            while self.creating.load(Ordering::SeqCst) {
                thread::sleep(self.timings.poll);
            }
            return Ok(file.is_file());
        }
        let _creating = Creating(self);
        self.notify(NoteEvent::StateChanged);
        if !file.is_file() {
            let name = file.file_name().unwrap_or_default().to_string_lossy();
            log::info!("Creating today's note ({name}) with the Obsidian CLI");
            let logged = action_log::sending(
                ActionTarget::Obsidian,
                &format!("Create today’s note ({name}) with the Obsidian CLI"),
            );
            let failed = |message: String| {
                logged.update(ActionState::Failed, Some(message.clone()));
                AppError::Io(message)
            };
            self.cli
                .run(
                    &settings.vault.obsidian_cli_path,
                    &settings.vault.name,
                    &["daily"],
                )
                .map_err(failed)?;
            if !self.wait_for(&file) {
                let appear = self.timings.appear.as_secs();
                return Err(failed(format!(
                    "Obsidian didn't create {name} within {appear} seconds."
                )));
            }
            logged.update(ActionState::Done, None);
            self.wait_until_quiet(&file);
        }
        self.drain(settings);
        Ok(true)
    }

    fn wait_for(&self, file: &Path) -> bool {
        let deadline = Instant::now() + self.timings.appear;
        while !file.is_file() {
            if Instant::now() >= deadline {
                return false;
            }
            thread::sleep(self.timings.poll);
        }
        true
    }

    /// Returns once the file stops changing, or at once when it is gone meanwhile.
    pub(super) fn wait_until_quiet(&self, file: &Path) {
        let mut previous = None;
        for _ in 0..self.timings.max_settle_checks {
            let Ok(meta) = std::fs::metadata(file) else {
                return;
            };
            let stamp = Some((meta.modified().ok(), meta.len()));
            if stamp == previous {
                return;
            }
            previous = stamp;
            thread::sleep(self.timings.settle);
        }
    }
}
