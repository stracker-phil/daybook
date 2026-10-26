//! The dated notes in the daily-notes folder (and below it) with their open todos, read once and kept up to
//! date by the vault's watcher (`changed`).

use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};

use chrono::NaiveDate;
use regex::Regex;

use super::scan::{ScannedTodo, scan_open_todos, todo_key};
use crate::lock::MutexExt;

static DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d{4}-\d{2}-\d{2})").unwrap());

/// A note whose file name holds a date.
#[derive(Clone, Debug, PartialEq)]
pub struct DatedNote {
    pub file: PathBuf,
    pub date: NaiveDate,
}

#[derive(Default)]
struct Inner {
    folder: Option<PathBuf>,
    notes: Option<Vec<DatedNote>>,
    todos: HashMap<PathBuf, Vec<ScannedTodo>>,
    /// The focus heading the kept todos were scanned with.
    skip_heading: String,
}

#[derive(Default)]
pub struct NotesIndex {
    inner: Mutex<Inner>,
}

fn date_of(file: &Path) -> Option<NaiveDate> {
    let name = file.file_name()?.to_str()?;
    NaiveDate::parse_from_str(&DATE.captures(name)?[1], "%Y-%m-%d").ok()
}

fn dated_notes(dir: &Path, found: &mut Vec<DatedNote>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if kind.is_dir() {
            dated_notes(&path, found);
        } else if kind.is_file()
            && path.extension().is_some_and(|ext| ext == "md")
            && let Some(date) = date_of(&path)
        {
            found.push(DatedNote { file: path, date });
        }
    }
}

impl Inner {
    fn notes(&mut self, folder: &Path) -> &[DatedNote] {
        if self.folder.as_deref() != Some(folder) {
            *self = Self {
                folder: Some(folder.to_path_buf()),
                ..Self::default()
            };
        }
        self.notes.get_or_insert_with(|| {
            let mut found = Vec::new();
            dated_notes(folder, &mut found);
            found
        })
    }
}

impl Inner {
    /// `file`'s open todos, scanned once until the note changes; `None` when it can't be read.
    fn scanned(&mut self, file: &Path, skip_heading: &str) -> Option<Vec<ScannedTodo>> {
        if let Some(scanned) = self.todos.get(file) {
            return Some(scanned.clone());
        }
        let text = fs::read_to_string(file)
            .map_err(|e| log::warn!("Couldn't read {}: {e}", file.display()))
            .ok()?;
        let scanned = scan_open_todos(&text, skip_heading);
        self.todos.insert(file.to_path_buf(), scanned.clone());
        Some(scanned)
    }
}

impl NotesIndex {
    /// Forgets the todos of `paths`; a note that was added or removed makes the folder be read again.
    pub fn changed(&self, paths: &[PathBuf]) {
        let mut inner = self.inner.lock_unpoisoned();
        for path in paths {
            inner.todos.remove(path);
            let known = inner
                .notes
                .as_ref()
                .is_some_and(|notes| notes.iter().any(|note| &note.file == path));
            if !known || !path.is_file() {
                inner.notes = None;
            }
        }
    }

    /// The dated notes in `folder` and below it.
    pub fn dated_notes(&self, folder: &Path) -> Vec<DatedNote> {
        self.inner.lock_unpoisoned().notes(folder).to_vec()
    }

    /// Every open todo of the dated notes, newest note first; the same text on the same day once. `ignore`
    /// holds `todo_key`s that aren't todos; `skip_heading` as in `scan_open_todos`.
    pub fn open_todos(
        &self,
        folder: &Path,
        skip_heading: &str,
        ignore: &HashSet<String>,
    ) -> Vec<(DatedNote, ScannedTodo)> {
        let mut inner = self.inner.lock_unpoisoned();
        if inner.skip_heading != skip_heading {
            inner.todos.clear();
            inner.skip_heading = skip_heading.to_string();
        }
        let mut notes = inner.notes(folder).to_vec();
        notes.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.file.cmp(&b.file)));
        let mut listed = HashSet::new();
        let mut todos = Vec::new();
        for note in notes {
            let Some(scanned) = inner.scanned(&note.file, skip_heading) else {
                continue;
            };
            for todo in scanned {
                let key = todo_key(&todo.text);
                if ignore.contains(&key) {
                    continue;
                }
                let first = listed.insert((note.date, key));
                if first {
                    todos.push((note.clone(), todo));
                }
            }
        }
        todos
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn write(dir: &TempDir, relative: &str, text: &str) -> PathBuf {
        let path = dir.path().join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }

    fn listed(index: &NotesIndex, dir: &TempDir) -> Vec<(String, String)> {
        listed_with(index, dir, "", &HashSet::new())
    }

    fn listed_with(
        index: &NotesIndex,
        dir: &TempDir,
        skip_heading: &str,
        ignore: &HashSet<String>,
    ) -> Vec<(String, String)> {
        index
            .open_todos(dir.path(), skip_heading, ignore)
            .into_iter()
            .map(|(note, todo)| (note.date.to_string(), todo.text))
            .collect()
    }

    fn pairs(items: &[(&str, &str)]) -> Vec<(String, String)> {
        items
            .iter()
            .map(|(date, text)| (date.to_string(), text.to_string()))
            .collect()
    }

    #[test]
    fn lists_todos_of_notes_in_subfolders_newest_date_first() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir, "2026-10-01.md", "- [ ] October\n");
        write(&dir, "2025/2025-12-01.md", "- [ ] Old\n");
        write(
            &dir,
            "2026-10-12.md",
            "- [ ] Newest one\n- [ ] Newest two\n",
        );

        assert_eq!(
            listed(&NotesIndex::default(), &dir),
            pairs(&[
                ("2026-10-12", "Newest one"),
                ("2026-10-12", "Newest two"),
                ("2026-10-01", "October"),
                ("2025-12-01", "Old"),
            ])
        );
    }

    #[test]
    fn dot_folders_other_extensions_and_undated_names_are_left_out() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir, "2026-10-01.md", "- [ ] Kept\n");
        write(&dir, ".trash/2026-10-02.md", "- [ ] Hidden\n");
        write(&dir, "2026-10-03.txt", "- [ ] Text file\n");
        write(&dir, "Ideas.md", "- [ ] Undated\n");
        write(&dir, "2026-13-45.md", "- [ ] Impossible date\n");

        assert_eq!(
            listed(&NotesIndex::default(), &dir),
            pairs(&[("2026-10-01", "Kept")])
        );
    }

    #[test]
    fn the_same_text_on_the_same_date_is_listed_once() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "2026-10-01.md",
            "- [ ] Call Anna\n## Later\n- [ ]  call   anna\n",
        );
        write(
            &dir,
            "archive/2026-10-01 copy.md",
            "- [ ] CALL ANNA\n- [ ] Other\n",
        );
        write(&dir, "2026-10-02.md", "- [ ] Call Anna\n");

        assert_eq!(
            listed(&NotesIndex::default(), &dir),
            pairs(&[
                ("2026-10-02", "Call Anna"),
                ("2026-10-01", "Call Anna"),
                ("2026-10-01", "Other"),
            ])
        );
    }

    #[test]
    fn ignored_keys_and_the_skip_heading_section_are_left_out() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "2026-10-01.md",
            "## Focus\n- [ ] Focus task\n## Plan\n- [ ] Book meeting rooms\n- [ ] Real\n",
        );
        let ignore = HashSet::from([todo_key("book  Meeting rooms")]);

        assert_eq!(
            listed_with(&NotesIndex::default(), &dir, "## focus", &ignore),
            pairs(&[("2026-10-01", "Real")])
        );
    }

    #[test]
    fn todos_carry_their_note_section_and_line() {
        let dir = tempfile::tempdir().unwrap();
        let file = write(&dir, "2026-10-01.md", "## Notes\n- [ ] Call Anna\n");

        let todos = NotesIndex::default().open_todos(dir.path(), "", &HashSet::new());

        assert_eq!(todos.len(), 1);
        assert_eq!(todos[0].0.file, file);
        assert_eq!(
            todos[0].1,
            ScannedTodo {
                text: "Call Anna".into(),
                section: "Notes".into(),
                line: 1,
            }
        );
    }

    #[test]
    fn a_changed_note_shows_its_new_todos_and_an_unreported_edit_stays_cached() {
        let dir = tempfile::tempdir().unwrap();
        let file = write(&dir, "2026-10-01.md", "- [ ] Before\n");
        let index = NotesIndex::default();
        assert_eq!(listed(&index, &dir), pairs(&[("2026-10-01", "Before")]));

        fs::write(&file, "- [ ] After\n").unwrap();
        assert_eq!(listed(&index, &dir), pairs(&[("2026-10-01", "Before")]));

        index.changed(&[file]);
        assert_eq!(listed(&index, &dir), pairs(&[("2026-10-01", "After")]));
    }

    #[test]
    fn a_deleted_note_disappears_once_reported() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir, "2026-10-01.md", "- [ ] Kept\n");
        let gone = write(&dir, "2026-10-02.md", "- [ ] Gone\n");
        let index = NotesIndex::default();
        assert_eq!(listed(&index, &dir).len(), 2);

        fs::remove_file(&gone).unwrap();
        index.changed(&[gone]);

        assert_eq!(listed(&index, &dir), pairs(&[("2026-10-01", "Kept")]));
        assert_eq!(index.dated_notes(dir.path()).len(), 1);
    }

    #[test]
    fn an_added_note_appears_once_reported() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir, "2026-10-01.md", "- [ ] First\n");
        let index = NotesIndex::default();
        assert_eq!(listed(&index, &dir).len(), 1);

        let added = write(&dir, "2026/2026-10-05.md", "- [ ] Added\n");
        index.changed(&[added]);

        assert_eq!(
            listed(&index, &dir),
            pairs(&[("2026-10-05", "Added"), ("2026-10-01", "First")])
        );
    }

    #[test]
    fn a_different_skip_heading_scans_the_notes_again() {
        let dir = tempfile::tempdir().unwrap();
        write(
            &dir,
            "2026-10-01.md",
            "## Focus\n- [ ] Focus task\n## Notes\n- [ ] Call\n",
        );
        let index = NotesIndex::default();

        let with_focus = listed_with(&index, &dir, "", &HashSet::new());
        let without_focus = listed_with(&index, &dir, "## Focus", &HashSet::new());

        assert_eq!(with_focus.len(), 2);
        assert_eq!(without_focus, pairs(&[("2026-10-01", "Call")]));
    }

    #[test]
    fn dated_notes_lists_files_with_their_dates() {
        let dir = tempfile::tempdir().unwrap();
        let file = write(&dir, "sub/2025-12-01.md", "- [ ] One\n");

        assert_eq!(
            NotesIndex::default().dated_notes(dir.path()),
            vec![DatedNote {
                file,
                date: NaiveDate::from_ymd_opt(2025, 12, 1).unwrap(),
            }]
        );
    }

    #[test]
    fn a_different_folder_resets_the_index() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        write(&first, "2026-10-01.md", "- [ ] One\n");
        let in_second = write(&second, "sub/2025-12-01.md", "- [ ] Two\n");
        let index = NotesIndex::default();
        assert_eq!(index.dated_notes(first.path()).len(), 1);

        assert_eq!(
            index.dated_notes(second.path()),
            vec![DatedNote {
                file: in_second,
                date: NaiveDate::from_ymd_opt(2025, 12, 1).unwrap(),
            }]
        );
        assert_eq!(listed(&index, &second), pairs(&[("2025-12-01", "Two")]));
    }
}
