//! The only code that writes into the vault. Writes to one file run one at a time, every edit starts
//! from a fresh read, and the result replaces the file atomically (temp file and rename) unless the
//! file changed meanwhile.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::lock::MutexExt;

/// Reads before giving up on a file that keeps changing between the read and the write.
const ATTEMPTS: usize = 2;

#[derive(Debug, thiserror::Error)]
pub enum WriteError {
    #[error("Refusing to write outside the vault: {0}")]
    OutsideVault(PathBuf),
    /// The file kept changing between the read and the write (Obsidian or a sync tool).
    #[error("{0} kept changing while Daybook tried to write to it.")]
    Conflict(String),
    /// The edit can't apply to this note.
    #[error("{0}")]
    Edit(String),
    #[error("Couldn't write {path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

/// An edit of a note's text: the new text, `None` for no change.
pub type NoteEdit<'a> = dyn FnMut(&str) -> Result<Option<String>, String> + 'a;

#[derive(Default)]
pub struct NoteWriter {
    files: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

impl NoteWriter {
    /// Applies `edit` to `file` inside `vault`; whether the file changed.
    pub fn edit_note(
        &self,
        vault: &Path,
        file: &Path,
        edit: &mut NoteEdit<'_>,
    ) -> Result<bool, WriteError> {
        if !is_inside(vault, file) {
            return Err(WriteError::OutsideVault(file.to_path_buf()));
        }
        let gate = Arc::clone(
            self.files
                .lock_unpoisoned()
                .entry(file.to_path_buf())
                .or_default(),
        );
        let _turn = gate.lock_unpoisoned();
        apply_edit(file, edit)
    }
}

/// Lexically inside: `..` is refused outright, so no path climbs out of the vault.
fn is_inside(vault: &Path, file: &Path) -> bool {
    let has_parent_dir = |path: &Path| path.components().any(|c| c == Component::ParentDir);
    vault.is_absolute()
        && !has_parent_dir(vault)
        && !has_parent_dir(file)
        && file.starts_with(vault)
        && file != vault
}

/// What tells two versions of a file apart without reading it.
#[derive(PartialEq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

impl Stamp {
    fn of(meta: &fs::Metadata) -> Self {
        Self {
            modified: meta.modified().ok(),
            len: meta.len(),
        }
    }
}

fn apply_edit(file: &Path, edit: &mut NoteEdit<'_>) -> Result<bool, WriteError> {
    let io_error = |source| WriteError::Io {
        path: file.to_path_buf(),
        source,
    };
    for _ in 0..ATTEMPTS {
        let meta = fs::metadata(file).map_err(io_error)?;
        let text = fs::read_to_string(file).map_err(io_error)?;
        let next = match edit(&text).map_err(WriteError::Edit)? {
            Some(next) if next != text => next,
            _ => return Ok(false),
        };
        if Stamp::of(&fs::metadata(file).map_err(io_error)?) != Stamp::of(&meta) {
            continue;
        }
        write_atomically(file, &next, meta.permissions().mode() & 0o7777).map_err(io_error)?;
        return Ok(true);
    }
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    Err(WriteError::Conflict(name.into_owned()))
}

/// A crash before the rename leaves the original as it was; a failed rename removes the temp file.
fn write_atomically(file: &Path, text: &str, mode: u32) -> io::Result<()> {
    let dir = file.parent().unwrap_or(Path::new("."));
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = file.file_name().unwrap_or_default().to_string_lossy();
    let tmp = dir.join(format!(".{name}.{}.{nanos}.tmp", std::process::id()));
    let written = (|| {
        let mut out = fs::File::create(&tmp)?;
        out.set_permissions(fs::Permissions::from_mode(mode))?;
        out.write_all(text.as_bytes())?;
        out.sync_all()?;
        fs::rename(&tmp, file)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;

    fn note_in(dir: &Path, name: &str, text: &str) -> PathBuf {
        let file = dir.join(name);
        fs::write(&file, text).unwrap();
        file
    }

    fn append(line: &'static str) -> impl FnMut(&str) -> Result<Option<String>, String> {
        move |text: &str| Ok(Some(format!("{text}{line}\n")))
    }

    fn names_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn an_edit_lands_and_reports_a_change() {
        let vault = tempfile::tempdir().unwrap();
        let file = note_in(vault.path(), "a.md", "start\n");

        let changed = NoteWriter::default()
            .edit_note(vault.path(), &file, &mut append("added"))
            .unwrap();

        assert!(changed);
        assert_eq!(fs::read_to_string(&file).unwrap(), "start\nadded\n");
    }

    #[test]
    fn an_edit_without_a_change_leaves_the_file_untouched() {
        let vault = tempfile::tempdir().unwrap();
        let file = note_in(vault.path(), "a.md", "start\n");
        let modified = fs::metadata(&file).unwrap().modified().unwrap();
        let writer = NoteWriter::default();

        let nothing = writer
            .edit_note(vault.path(), &file, &mut |_: &str| Ok(None))
            .unwrap();
        let same = writer
            .edit_note(vault.path(), &file, &mut |text: &str| {
                Ok(Some(text.to_string()))
            })
            .unwrap();

        assert!(!nothing);
        assert!(!same);
        assert_eq!(fs::read_to_string(&file).unwrap(), "start\n");
        assert_eq!(fs::metadata(&file).unwrap().modified().unwrap(), modified);
        assert_eq!(names_in(vault.path()), ["a.md"]);
    }

    #[test]
    fn the_file_mode_is_kept() {
        let vault = tempfile::tempdir().unwrap();
        let file = note_in(vault.path(), "a.md", "start\n");
        fs::set_permissions(&file, fs::Permissions::from_mode(0o640)).unwrap();

        NoteWriter::default()
            .edit_note(vault.path(), &file, &mut append("added"))
            .unwrap();

        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
    }

    #[test]
    fn concurrent_edits_of_one_file_all_land() {
        let vault = tempfile::tempdir().unwrap();
        let file = note_in(vault.path(), "a.md", "start\n");
        let writer = NoteWriter::default();

        std::thread::scope(|scope| {
            for i in 0..8 {
                let (writer, file, vault) = (&writer, &file, vault.path());
                scope.spawn(move || {
                    let mut edit = |text: &str| -> Result<Option<String>, String> {
                        Ok(Some(format!("{text}line {i}\n")))
                    };
                    writer.edit_note(vault, file, &mut edit).unwrap();
                });
            }
        });

        let text = fs::read_to_string(&file).unwrap();
        let mut lines: Vec<&str> = text.lines().collect();
        lines.sort_unstable();
        assert_eq!(
            lines,
            [
                "line 0", "line 1", "line 2", "line 3", "line 4", "line 5", "line 6", "line 7",
                "start"
            ]
        );
    }

    #[test]
    fn a_file_changed_between_read_and_write_is_read_again() {
        let vault = tempfile::tempdir().unwrap();
        let file = note_in(vault.path(), "a.md", "start\n");
        let mut calls = 0;
        let mut edit = |text: &str| -> Result<Option<String>, String> {
            calls += 1;
            if calls == 1 {
                fs::write(&file, format!("{text}from obsidian\n")).unwrap();
            }
            Ok(Some(format!("{text}mine\n")))
        };

        let changed = NoteWriter::default()
            .edit_note(vault.path(), &file, &mut edit)
            .unwrap();

        assert!(changed);
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "start\nfrom obsidian\nmine\n"
        );
    }

    #[test]
    fn a_file_that_changes_on_every_attempt_is_a_conflict_and_keeps_the_outside_version() {
        let vault = tempfile::tempdir().unwrap();
        let file = note_in(vault.path(), "a.md", "start\n");
        let mut edit = |text: &str| -> Result<Option<String>, String> {
            fs::write(&file, format!("{text}outside\n")).unwrap();
            Ok(Some(format!("{text}mine\n")))
        };

        let result = NoteWriter::default().edit_note(vault.path(), &file, &mut edit);

        assert!(matches!(result, Err(WriteError::Conflict(name)) if name == "a.md"));
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "start\noutside\noutside\n"
        );
        assert_eq!(names_in(vault.path()), ["a.md"]);
    }

    #[test]
    fn an_edit_error_is_reported_and_changes_nothing() {
        let vault = tempfile::tempdir().unwrap();
        let file = note_in(vault.path(), "a.md", "start\n");

        let result = NoteWriter::default().edit_note(vault.path(), &file, &mut |_: &str| {
            Err("can't apply".to_string())
        });

        assert!(matches!(result, Err(WriteError::Edit(message)) if message == "can't apply"));
        assert_eq!(fs::read_to_string(&file).unwrap(), "start\n");
    }

    #[test]
    fn paths_outside_the_vault_are_refused_and_nothing_is_written() {
        let root = tempfile::tempdir().unwrap();
        let vault = root.path().join("vault");
        let other = root.path().join("other");
        fs::create_dir_all(&vault).unwrap();
        fs::create_dir_all(&other).unwrap();
        let outside = note_in(&other, "a.md", "start\n");
        let climbing = vault.join("..").join("other").join("a.md");
        let writer = NoteWriter::default();

        for (file, label) in [
            (outside.clone(), "other folder"),
            (climbing, "parent components"),
            (vault.clone(), "the vault itself"),
        ] {
            let result = writer.edit_note(&vault, &file, &mut append("added"));
            assert!(
                matches!(result, Err(WriteError::OutsideVault(_))),
                "{label}"
            );
        }

        assert_eq!(fs::read_to_string(&outside).unwrap(), "start\n");
        assert_eq!(names_in(&other), ["a.md"]);
        assert_eq!(names_in(&vault), Vec::<String>::new());
    }

    #[test]
    fn a_failed_write_keeps_the_original_and_leaves_no_temp_file() {
        let vault = tempfile::tempdir().unwrap();
        let folder = vault.path().join("locked");
        fs::create_dir(&folder).unwrap();
        let file = note_in(&folder, "a.md", "start\n");
        fs::set_permissions(&folder, fs::Permissions::from_mode(0o500)).unwrap();

        let result = NoteWriter::default().edit_note(vault.path(), &file, &mut append("added"));

        fs::set_permissions(&folder, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(matches!(result, Err(WriteError::Io { .. })));
        assert_eq!(fs::read_to_string(&file).unwrap(), "start\n");
        assert_eq!(names_in(&folder), ["a.md"]);
    }
}
