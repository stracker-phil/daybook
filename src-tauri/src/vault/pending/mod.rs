//! Changes for a note that can't take them yet (it doesn't exist, or it kept changing): kept in
//! `pending-entries.json` with no expiry, written in order once their note can take them.

mod report;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use super::ops::{NoteLayout, NoteOp};
use super::writer::{NoteWriter, WriteError};
use crate::action_log::{ActionState, Logged};
use crate::error::AppResult;
use crate::lock::MutexExt;
use crate::store::json_store::JsonStore;

pub const FILE_NAME: &str = "pending-entries.json";
/// The detail of a change the note already had.
pub const ALREADY: &str = "Already in the note";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PendingEntry {
    /// Milliseconds since the epoch.
    pub created_at: i64,
    /// The note that was "today" when the change was made.
    pub file: PathBuf,
    pub description: String,
    pub op: NoteOp,
    /// Its line in the action log; this session only.
    #[serde(skip)]
    pub logged: Logged,
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct PendingEntries {
    pub entries: Vec<PendingEntry>,
}

/// Entries that don't read as an entry (an op kind this version doesn't write) are dropped.
impl<'de> Deserialize<'de> for PendingEntries {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = Value::deserialize(deserializer)?;
        let entries = raw["entries"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|entry| match serde_json::from_value(entry.clone()) {
                Ok(entry) => Some(entry),
                Err(e) => {
                    log::warn!("Dropped a waiting note change that can't be read: {e}");
                    None
                }
            })
            .collect();
        Ok(Self { entries })
    }
}

#[derive(Debug, Default, PartialEq)]
pub struct Drained {
    pub written: usize,
    /// Entries that can never apply (e.g. their journal group is gone), with the reason.
    pub dropped: Vec<(String, String)>,
}

pub struct Pending {
    store: JsonStore<PendingEntries>,
    /// One drain at a time, so no entry is written twice.
    draining: Mutex<()>,
}

impl Pending {
    pub fn open(config_dir: &Path) -> Self {
        Self {
            store: JsonStore::open(config_dir.join(FILE_NAME)),
            draining: Mutex::new(()),
        }
    }

    pub fn count(&self) -> usize {
        self.store.read(|pending| pending.entries.len())
    }

    /// Changes for this note wait already; a new one must queue behind them to keep the order.
    pub fn has_for(&self, file: &Path) -> bool {
        self.store
            .read(|pending| pending.entries.iter().any(|entry| entry.file == file))
    }

    /// The notes that entries wait for, each once.
    pub fn files(&self) -> Vec<PathBuf> {
        let mut seen = HashSet::new();
        self.store.read(|pending| {
            pending
                .entries
                .iter()
                .filter(|entry| seen.insert(entry.file.clone()))
                .map(|entry| entry.file.clone())
                .collect()
        })
    }

    /// Queues `op` with its action-log line, which moves on once it is written.
    pub fn enqueue_logged(&self, file: &Path, op: NoteOp, logged: Logged) -> AppResult<()> {
        let entry = PendingEntry {
            created_at: chrono::Utc::now().timestamp_millis(),
            file: file.to_path_buf(),
            description: op.describe(),
            op,
            logged,
        };
        self.store.update(|pending| pending.entries.push(entry))
    }

    /// Writes the waiting entries in order, each into its own note. A missing note, a conflict or a
    /// disk error keeps that note's entries (in order) for the next drain; other notes still get theirs.
    pub fn drain(&self, writer: &NoteWriter, vault: &Path, layout: &NoteLayout) -> Drained {
        let _turn = self.draining.lock_unpoisoned();
        let entries = self.store.read(|pending| pending.entries.clone());
        let mut blocked: HashSet<PathBuf> = HashSet::new();
        let mut done: Vec<PendingEntry> = Vec::new();
        let mut drained = Drained::default();
        for entry in entries {
            if blocked.contains(&entry.file) || !entry.file.is_file() {
                blocked.insert(entry.file.clone());
                continue;
            }
            let mut edit = |text: &str| entry.op.apply(text, layout);
            match writer.edit_note(vault, &entry.file, &mut edit) {
                Ok(changed) => {
                    drained.written += 1;
                    if changed {
                        entry.report(ActionState::Done, None);
                    } else {
                        entry.report(ActionState::Skipped, Some(ALREADY.into()));
                    }
                }
                Err(e @ (WriteError::Edit(_) | WriteError::OutsideVault(_))) => {
                    let reason = e.to_string();
                    log::warn!(
                        "Dropped the waiting change “{}”: {reason}",
                        entry.description
                    );
                    entry.report(ActionState::Failed, Some(reason.clone()));
                    drained.dropped.push((entry.description.clone(), reason));
                }
                Err(e @ (WriteError::Conflict(_) | WriteError::Io { .. })) => {
                    log::warn!("“{}” waits: {e}", entry.description);
                    entry.report(ActionState::Waiting, Some(format!("{e} Retried later.")));
                    blocked.insert(entry.file.clone());
                    continue;
                }
            }
            done.push(entry);
        }
        if !done.is_empty()
            && let Err(e) = self
                .store
                .update(|pending| remove_each(&mut pending.entries, done))
        {
            log::error!("Couldn't save the waiting note changes: {e}");
        }
        drained
    }
}

/// Removes one entry per finished one; entries queued during the drain stay.
fn remove_each(entries: &mut Vec<PendingEntry>, done: Vec<PendingEntry>) {
    for finished in done {
        if let Some(index) = entries.iter().position(|entry| *entry == finished) {
            entries.remove(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::Settings;
    use crate::test_support::temp_config_dir;
    use crate::vault::ops::journal::JournalGroupAnswers;

    fn layout() -> NoteLayout {
        NoteLayout::from_settings(&Settings::default())
    }

    fn step(title: &str) -> NoteOp {
        NoteOp::PlanStep {
            title: title.into(),
            done: true,
        }
    }

    fn applied(text: &str, ops: &[NoteOp]) -> String {
        let layout = layout();
        let mut text = text.to_string();
        for op in ops {
            if let Some(next) = op.apply(&text, &layout).unwrap() {
                text = next;
            }
        }
        text
    }

    fn note(vault: &Path, name: &str) -> PathBuf {
        vault.join(name)
    }

    fn create(file: &Path) {
        std::fs::write(file, "# Day\n").unwrap();
    }

    fn read(file: &Path) -> String {
        std::fs::read_to_string(file).unwrap()
    }

    #[test]
    fn entries_survive_reopening_the_store_in_order() {
        let config = temp_config_dir();
        let vault = tempfile::tempdir().unwrap();
        let (a, b) = (note(vault.path(), "a.md"), note(vault.path(), "b.md"));
        let pending = Pending::open(config.path());
        pending
            .enqueue_logged(&a, step("One"), Logged::default())
            .unwrap();
        pending
            .enqueue_logged(&b, step("Other"), Logged::default())
            .unwrap();
        pending
            .enqueue_logged(&a, step("Two"), Logged::default())
            .unwrap();
        drop(pending);

        let reopened = Pending::open(config.path());
        create(&a);
        create(&b);
        let drained = reopened.drain(&NoteWriter::default(), vault.path(), &layout());

        assert_eq!(
            drained,
            Drained {
                written: 3,
                dropped: vec![]
            }
        );
        assert_eq!(read(&a), applied("# Day\n", &[step("One"), step("Two")]));
        assert_eq!(read(&b), applied("# Day\n", &[step("Other")]));
    }

    #[test]
    fn count_has_for_and_files_describe_the_queue() {
        let config = temp_config_dir();
        let vault = tempfile::tempdir().unwrap();
        let (a, b, c) = (
            note(vault.path(), "a.md"),
            note(vault.path(), "b.md"),
            note(vault.path(), "c.md"),
        );
        let pending = Pending::open(config.path());
        assert_eq!(pending.count(), 0);
        assert!(pending.files().is_empty());

        pending
            .enqueue_logged(&a, step("One"), Logged::default())
            .unwrap();
        pending
            .enqueue_logged(&b, step("Two"), Logged::default())
            .unwrap();
        pending
            .enqueue_logged(&a, step("Three"), Logged::default())
            .unwrap();

        assert_eq!(pending.count(), 3);
        assert!(pending.has_for(&a));
        assert!(pending.has_for(&b));
        assert!(!pending.has_for(&c));
        assert_eq!(pending.files(), [a, b]);
    }

    #[test]
    fn drain_writes_in_order_and_keeps_entries_whose_note_is_missing() {
        let config = temp_config_dir();
        let vault = tempfile::tempdir().unwrap();
        let (missing, present) = (
            note(vault.path(), "missing.md"),
            note(vault.path(), "present.md"),
        );
        create(&present);
        let writer = NoteWriter::default();
        let pending = Pending::open(config.path());
        pending
            .enqueue_logged(&missing, step("First"), Logged::default())
            .unwrap();
        pending
            .enqueue_logged(&present, step("Early"), Logged::default())
            .unwrap();
        pending
            .enqueue_logged(&missing, step("Second"), Logged::default())
            .unwrap();
        pending
            .enqueue_logged(&present, step("Late"), Logged::default())
            .unwrap();

        let first = pending.drain(&writer, vault.path(), &layout());

        assert_eq!(
            first,
            Drained {
                written: 2,
                dropped: vec![]
            }
        );
        assert_eq!(
            read(&present),
            applied("# Day\n", &[step("Early"), step("Late")])
        );
        assert_eq!(pending.count(), 2);
        assert_eq!(pending.files(), [missing.clone()]);
        assert!(!missing.exists());

        create(&missing);
        let second = pending.drain(&writer, vault.path(), &layout());

        assert_eq!(
            second,
            Drained {
                written: 2,
                dropped: vec![]
            }
        );
        assert_eq!(
            read(&missing),
            applied("# Day\n", &[step("First"), step("Second")])
        );
        assert_eq!(pending.count(), 0);
    }

    #[test]
    fn an_entry_that_cannot_apply_is_dropped_and_reported() {
        let config = temp_config_dir();
        let vault = tempfile::tempdir().unwrap();
        let file = note(vault.path(), "a.md");
        create(&file);
        let gone = NoteOp::JournalGroup {
            group: "No such group".into(),
            answers: JournalGroupAnswers::default(),
        };
        let pending = Pending::open(config.path());
        pending
            .enqueue_logged(&file, gone.clone(), Logged::default())
            .unwrap();
        pending
            .enqueue_logged(&file, step("Fine"), Logged::default())
            .unwrap();

        let drained = pending.drain(&NoteWriter::default(), vault.path(), &layout());

        assert_eq!(
            drained,
            Drained {
                written: 1,
                dropped: vec![(
                    gone.describe(),
                    "“No such group” isn't a journal group.".to_string()
                )],
            }
        );
        assert_eq!(read(&file), applied("# Day\n", &[step("Fine")]));
        assert_eq!(pending.count(), 0);
    }

    #[test]
    fn an_entry_of_an_unknown_kind_is_skipped_and_the_valid_ones_load() {
        let config = temp_config_dir();
        let vault = tempfile::tempdir().unwrap();
        let file = note(vault.path(), "a.md");
        create(&file);
        let stored = serde_json::json!({
            "entries": [
                { "kind": "activity", "time": "09:00", "label": "x" },
                {
                    "createdAt": 1729000000000_i64,
                    "file": file,
                    "description": "Plan for Today: - [x] Valid",
                    "op": { "kind": "planStep", "title": "Valid", "done": true }
                },
                { "createdAt": 1729000000001_i64, "file": file, "description": "x", "op": { "kind": "somethingNew" } }
            ]
        });
        std::fs::write(config.path().join(FILE_NAME), stored.to_string()).unwrap();

        let pending = Pending::open(config.path());

        assert_eq!(pending.count(), 1);
        assert_eq!(pending.files(), [file.clone()]);
        let drained = pending.drain(&NoteWriter::default(), vault.path(), &layout());
        assert_eq!(
            drained,
            Drained {
                written: 1,
                dropped: vec![]
            }
        );
        assert_eq!(read(&file), applied("# Day\n", &[step("Valid")]));
    }
}
