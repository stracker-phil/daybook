//! The vault's notes and images, read once in the background and kept current from the watcher's paths.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Instant;

use super::files::{FileContent, is_hidden, read_file, vault_path, walk};
use super::note::IndexedNote;
use super::posix;
use super::preview::markdown::Links;
use super::rank::{self, Hit};
use super::uri::decode_uri;
use crate::lock::RwLockExt;

/// Notes a reader parses before it adds them to the index.
const BATCH: usize = 64;
const MAX_READERS: usize = 8;

/// The images are kept by lowercase file name.
fn asset_key(path: &str) -> String {
    posix::basename(path).to_lowercase()
}

#[derive(Default)]
struct Inner {
    root: Option<PathBuf>,
    notes: HashMap<String, IndexedNote>,
    /// `asset_key` → the images with that name.
    assets: HashMap<String, Vec<String>>,
}

impl Inner {
    fn for_root(root: Option<PathBuf>) -> Self {
        Self {
            root,
            ..Self::default()
        }
    }

    fn add_asset(&mut self, path: &str) {
        let bucket = self.assets.entry(asset_key(path)).or_default();
        if !bucket.iter().any(|known| known == path) {
            bucket.push(path.to_string());
        }
    }

    fn remove(&mut self, path: &str) {
        self.notes.remove(path);
        let key = asset_key(path);
        if let Some(bucket) = self.assets.get_mut(&key) {
            bucket.retain(|known| known != path);
            if bucket.is_empty() {
                self.assets.remove(&key);
            }
        }
    }

    /// Everything below the folder `path`, for a folder that was moved or deleted.
    fn remove_below(&mut self, path: &str) {
        let prefix = format!("{path}/");
        self.notes.retain(|known, _| !known.starts_with(&prefix));
        self.assets.retain(|_, bucket| {
            bucket.retain(|known| !known.starts_with(&prefix));
            !bucket.is_empty()
        });
    }

    fn resolve_by_path(&self, cleaned: &str, from: Option<&str>) -> Option<String> {
        let exact = format!("{cleaned}.md");
        if self.notes.contains_key(&exact) {
            return Some(exact);
        }
        if let Some(from) = from {
            let relative = posix::join_normalize(posix::dirname(from), &exact);
            if self.notes.contains_key(&relative) {
                return Some(relative);
            }
        }
        let wanted = exact.to_lowercase();
        let suffix = format!("/{wanted}");
        shortest(self.notes.keys().filter(|path| {
            let path = path.to_lowercase();
            path == wanted || path.ends_with(&suffix)
        }))
    }

    fn resolve_by_name(&self, cleaned: &str) -> Option<String> {
        let lower = cleaned.to_lowercase();
        let paths_where = |matches: &dyn Fn(&IndexedNote) -> bool| {
            shortest(
                self.notes
                    .values()
                    .filter(|note| matches(note))
                    .map(|note| &note.path),
            )
        };
        paths_where(&|note| note.name_lower == lower)
            .or_else(|| paths_where(&|note| note.aliases_lower.contains(&lower)))
    }
}

/// What the watcher's paths add to the index.
#[derive(Default)]
struct Changes {
    read: Vec<FileContent>,
    gone_folders: Vec<String>,
}

impl Changes {
    fn collect(root: &Path, paths: &[PathBuf]) -> Self {
        let mut changes = Self::default();
        for path in paths {
            if let Some(relative) = path
                .strip_prefix(root)
                .ok()
                .filter(|relative| !is_hidden(relative))
                .and_then(vault_path)
            {
                changes.add(root, path, relative);
            }
        }
        changes
    }

    fn add(&mut self, root: &Path, path: &Path, relative: String) {
        if path.is_dir() {
            let (mut notes, mut assets) = (Vec::new(), Vec::new());
            walk(root, Path::new(&relative), &mut notes, &mut assets);
            self.read
                .extend(notes.into_iter().map(|path| read_file(root, path)));
            self.read.extend(assets.into_iter().map(FileContent::Asset));
            return;
        }
        if !path.exists() {
            self.gone_folders.push(relative.clone());
        }
        self.read.push(read_file(root, relative));
    }
}

#[derive(Default)]
pub struct SearchIndex {
    inner: RwLock<Inner>,
    building: AtomicBool,
    /// Counts up with every build, so a build for a vault left meanwhile stops adding to the index.
    generation: AtomicU64,
}

impl SearchIndex {
    /// Forgets everything; nothing is indexed until the next `build`.
    pub fn clear(&self) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        *self.inner.write_unpoisoned() = Inner::for_root(None);
        self.building.store(false, Ordering::SeqCst);
    }

    pub fn root(&self) -> Option<PathBuf> {
        self.inner.read_unpoisoned().root.clone()
    }

    pub fn is_building(&self) -> bool {
        self.building.load(Ordering::SeqCst)
    }

    pub fn count(&self) -> usize {
        self.inner.read_unpoisoned().notes.len()
    }

    /// Reads every note and image below `root` (canonical). Notes join the index in batches, so search
    /// works while it builds.
    pub fn build(&self, root: &Path) {
        let started = Instant::now();
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        *self.inner.write_unpoisoned() = Inner::for_root(Some(root.to_path_buf()));
        self.building.store(true, Ordering::SeqCst);
        let (mut notes, mut assets) = (Vec::new(), Vec::new());
        walk(root, Path::new(""), &mut notes, &mut assets);
        {
            let mut inner = self.inner.write_unpoisoned();
            if !self.is_current(generation) {
                return;
            }
            assets.iter().for_each(|path| inner.add_asset(path));
        }
        self.spawn_readers(root, &notes, generation);
        if self.is_current(generation) {
            self.building.store(false, Ordering::SeqCst);
            log::info!(
                "Indexed {} notes in {} ms",
                self.count(),
                started.elapsed().as_millis()
            );
        }
    }

    /// Reads `notes` on a few threads and returns once all are done.
    fn spawn_readers(&self, root: &Path, notes: &[String], generation: u64) {
        let readers = thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(MAX_READERS);
        let share = notes.len().div_ceil(readers).max(1);
        thread::scope(|scope| {
            for chunk in notes.chunks(share) {
                scope.spawn(move || {
                    for batch in chunk.chunks(BATCH) {
                        let read = batch
                            .iter()
                            .map(|path| read_file(root, path.clone()))
                            .collect();
                        if !self.apply(generation, read) {
                            return;
                        }
                    }
                });
            }
        });
    }

    fn is_current(&self, generation: u64) -> bool {
        self.generation.load(Ordering::SeqCst) == generation
    }

    /// Adds `read` to the index; `false` once a newer build or `clear` took over.
    fn apply(&self, generation: u64, read: Vec<FileContent>) -> bool {
        let mut inner = self.inner.write_unpoisoned();
        if !self.is_current(generation) {
            return false;
        }
        for content in read {
            match content {
                FileContent::Note(note) => {
                    inner.notes.insert(note.path.clone(), *note);
                }
                FileContent::Asset(path) => inner.add_asset(&path),
                FileContent::Gone(path) => inner.remove(&path),
            }
        }
        true
    }

    /// Reads `paths` (absolute, as the watcher reports them) again: a file is (re)indexed or removed, a
    /// folder is read, and everything below a folder that is gone is removed.
    pub fn update(&self, paths: &[PathBuf]) {
        let generation = self.generation.load(Ordering::SeqCst);
        let Some(root) = self.root() else {
            return;
        };
        let changes = Changes::collect(&root, paths);
        let mut inner = self.inner.write_unpoisoned();
        if !self.is_current(generation) {
            return;
        }
        changes
            .gone_folders
            .iter()
            .for_each(|folder| inner.remove_below(folder));
        drop(inner);
        self.apply(generation, changes.read);
    }

    pub fn search(
        &self,
        terms: &[String],
        folders: &[String],
        exclude: &[String],
        now_ms: i64,
    ) -> Vec<Hit> {
        let inner = self.inner.read_unpoisoned();
        rank::search(inner.notes.values(), terms, folders, exclude, now_ms)
    }

    pub fn has_note(&self, path: &str) -> bool {
        self.inner.read_unpoisoned().notes.contains_key(path)
    }

    /// The note a link names: `Note`, `folder/Note` or a path relative to `from`, then by alias; `#heading`
    /// and `^block` are ignored, and an empty target is `from` itself.
    pub fn resolve_note(&self, target: &str, from: Option<&str>) -> Option<String> {
        let cleaned = target.split('#').next().unwrap_or_default();
        let cleaned = cleaned.split('^').next().unwrap_or_default().trim();
        let cleaned = strip_md(cleaned);
        if cleaned.is_empty() {
            return from.map(str::to_string);
        }
        let inner = self.inner.read_unpoisoned();
        if cleaned.contains('/') {
            inner.resolve_by_path(cleaned, from)
        } else {
            inner.resolve_by_name(cleaned)
        }
    }

    /// The image a link names: the path itself, else relative to `from`, else the shortest path with that
    /// file name.
    pub fn resolve_asset(&self, target: &str, from: &str) -> Option<String> {
        let raw = target.split('|').next().unwrap_or_default().trim();
        let cleaned = decode_uri(raw).unwrap_or_else(|| raw.to_string());
        let inner = self.inner.read_unpoisoned();
        let bucket = inner.assets.get(&asset_key(&cleaned))?;
        if bucket.contains(&cleaned) {
            return Some(cleaned);
        }
        let relative = posix::join_normalize(posix::dirname(from), &cleaned);
        if bucket.contains(&relative) {
            return Some(relative);
        }
        shortest(bucket.iter())
    }

    /// `[[Name]]`, or `[[folder/Name]]` when more than one note has that name.
    pub fn wikilink_for(&self, path: &str) -> String {
        let inner = self.inner.read_unpoisoned();
        let name = inner.notes.get(path).map_or_else(
            || strip_md(posix::basename(path)).to_string(),
            |note| note.name.clone(),
        );
        let lower = name.to_lowercase();
        let same_name = inner
            .notes
            .values()
            .filter(|note| note.name_lower == lower)
            .count();
        if same_name > 1 {
            format!("[[{}]]", strip_md(path))
        } else {
            format!("[[{name}]]")
        }
    }
}

/// Without a trailing `.md` in any case.
fn strip_md(text: &str) -> &str {
    let cut = text.len().saturating_sub(3);
    match text.get(cut..) {
        Some(end) if end.eq_ignore_ascii_case(".md") => &text[..cut],
        _ => text,
    }
}

/// The shortest path, then the first by name.
fn shortest<'a>(paths: impl Iterator<Item = &'a String>) -> Option<String> {
    paths
        .min_by(|a, b| {
            a.len()
                .cmp(&b.len())
                .then_with(|| a.to_lowercase().cmp(&b.to_lowercase()))
                .then_with(|| a.cmp(b))
        })
        .cloned()
}

impl Links for SearchIndex {
    fn resolve_note(&self, target: &str, from: &str) -> Option<String> {
        SearchIndex::resolve_note(self, target, Some(from))
    }

    fn resolve_asset(&self, target: &str, from: &str) -> Option<String> {
        SearchIndex::resolve_asset(self, target, from)
    }

    fn has_note(&self, path: &str) -> bool {
        SearchIndex::has_note(self, path)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::search::files::MAX_NOTE_BYTES;

    struct Vault {
        _dir: tempfile::TempDir,
        root: PathBuf,
    }

    impl Vault {
        fn new(files: &[(&str, &str)]) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().canonicalize().unwrap();
            let vault = Self { _dir: dir, root };
            for (path, text) in files {
                vault.write(path, text);
            }
            vault
        }

        fn write(&self, relative: &str, text: &str) {
            let file = self.root.join(relative);
            fs::create_dir_all(file.parent().unwrap()).unwrap();
            fs::write(file, text).unwrap();
        }

        fn path(&self, relative: &str) -> PathBuf {
            self.root.join(relative)
        }

        fn index(&self) -> SearchIndex {
            let index = SearchIndex::default();
            index.build(&self.root);
            index
        }
    }

    fn found(index: &SearchIndex, term: &str) -> Vec<String> {
        found_excluding(index, term, &[])
    }

    fn found_excluding(index: &SearchIndex, term: &str, exclude: &[&str]) -> Vec<String> {
        let exclude: Vec<String> = exclude.iter().map(|prefix| prefix.to_string()).collect();
        index
            .search(&[term.to_string()], &[String::new()], &exclude, 0)
            .into_iter()
            .map(|hit| hit.path)
            .collect()
    }

    fn resolver_vault() -> Vault {
        Vault::new(&[
            ("Home.md", ""),
            ("Projects/Plan.md", "---\naliases: [Roadmap]\n---\n"),
            ("Archive/Old/Projects/Plan.md", ""),
            ("Projects/Sub/Detail.md", ""),
            ("Other/Sub/Detail.md", ""),
            ("Notes/Aliased.md", "---\nalias: Nickname\n---\n"),
        ])
    }

    #[test]
    fn building_indexes_every_note_and_finishes() {
        let vault = Vault::new(&[
            ("a.md", "a"),
            ("sub/b.md", "b"),
            ("pic.png", "x"),
            (".obsidian/c.md", "c"),
        ]);
        let index = SearchIndex::default();

        index.build(&vault.root);

        assert_eq!(index.count(), 2);
        assert!(!index.is_building());
        assert_eq!(index.root(), Some(vault.root.clone()));
        assert!(index.has_note("sub/b.md"));
        assert!(!index.has_note(".obsidian/c.md"));
        assert!(!index.has_note("pic.png"));
    }

    #[test]
    fn building_an_empty_vault_indexes_nothing() {
        let vault = Vault::new(&[]);

        let index = vault.index();

        assert_eq!(index.count(), 0);
        assert!(!index.is_building());
    }

    #[test]
    fn building_again_replaces_the_previous_vault() {
        let first = Vault::new(&[("one.md", "x")]);
        let second = Vault::new(&[("two.md", "x"), ("three.md", "x")]);
        let index = first.index();

        index.build(&second.root);

        assert_eq!(index.count(), 2);
        assert!(!index.has_note("one.md"));
        assert_eq!(index.root(), Some(second.root.clone()));
    }

    #[test]
    fn a_name_match_beats_an_alias_a_heading_and_a_body_match() {
        let vault = Vault::new(&[
            ("body.md", "some zebra in the text\n"),
            ("heading.md", "# Zebra crossing\n"),
            ("aliased.md", "---\naliases: [zebra]\n---\ntext\n"),
            ("zebra.md", "text\n"),
        ]);

        assert_eq!(
            found(&vault.index(), "zebra"),
            ["zebra.md", "aliased.md", "heading.md", "body.md"]
        );
    }

    #[test]
    fn searching_without_a_match_or_without_terms_finds_nothing() {
        let vault = Vault::new(&[("a.md", "alpha\n")]);
        let index = vault.index();

        assert!(found(&index, "missing").is_empty());
        assert!(index.search(&[], &[String::new()], &[], 0).is_empty());
    }

    #[test]
    fn an_excluded_folder_never_shows_in_results() {
        let vault = Vault::new(&[
            ("Private/secret.md", "zebra\n"),
            ("Public/open.md", "zebra\n"),
        ]);

        assert_eq!(
            found_excluding(&vault.index(), "zebra", &["Private/"]),
            ["Public/open.md"]
        );
    }

    #[test]
    fn clearing_forgets_the_vault() {
        let vault = Vault::new(&[("a.md", "zebra\n"), ("pic.png", "x")]);
        let index = vault.index();

        index.clear();

        assert_eq!(index.count(), 0);
        assert!(!index.is_building());
        assert_eq!(index.root(), None);
        assert!(found(&index, "zebra").is_empty());
        assert_eq!(index.resolve_asset("pic.png", "a.md"), None);
    }

    #[test]
    fn updating_before_a_build_does_nothing() {
        let vault = Vault::new(&[("a.md", "x")]);
        let index = SearchIndex::default();

        index.update(&[vault.path("a.md")]);

        assert_eq!(index.count(), 0);
    }

    #[test]
    fn a_created_note_shows_up_after_an_update() {
        let vault = Vault::new(&[("a.md", "alpha\n")]);
        let index = vault.index();
        vault.write("new.md", "zebra\n");

        index.update(&[vault.path("new.md")]);

        assert_eq!(found(&index, "zebra"), ["new.md"]);
        assert_eq!(index.count(), 2);
    }

    #[test]
    fn a_modified_notes_new_text_is_found_and_the_old_text_is_not() {
        let vault = Vault::new(&[("a.md", "alpha\n")]);
        let index = vault.index();
        vault.write("a.md", "zebra\n");

        index.update(&[vault.path("a.md")]);

        assert_eq!(found(&index, "zebra"), ["a.md"]);
        assert!(found(&index, "alpha").is_empty());
    }

    #[test]
    fn a_deleted_note_disappears_after_an_update() {
        let vault = Vault::new(&[("a.md", "zebra\n"), ("b.md", "zebra\n")]);
        let index = vault.index();
        fs::remove_file(vault.path("a.md")).unwrap();

        index.update(&[vault.path("a.md")]);

        assert_eq!(found(&index, "zebra"), ["b.md"]);
        assert!(!index.has_note("a.md"));
    }

    #[test]
    fn a_note_grown_past_the_size_limit_is_removed_by_an_update() {
        let vault = Vault::new(&[("a.md", "zebra\n")]);
        let index = vault.index();
        let file = fs::OpenOptions::new()
            .write(true)
            .open(vault.path("a.md"))
            .unwrap();
        file.set_len(MAX_NOTE_BYTES + 1).unwrap();

        index.update(&[vault.path("a.md")]);

        assert!(!index.has_note("a.md"));
        assert_eq!(index.count(), 0);
    }

    #[test]
    fn dot_paths_and_paths_outside_the_vault_are_ignored_by_an_update() {
        let vault = Vault::new(&[("a.md", "x")]);
        let outside = Vault::new(&[("elsewhere.md", "x")]);
        let index = vault.index();
        vault.write(".obsidian/plugin.md", "x");
        vault.write("sub/.draft.md", "x");

        index.update(&[
            vault.path(".obsidian/plugin.md"),
            vault.path("sub/.draft.md"),
            vault.path(".obsidian"),
            outside.path("elsewhere.md"),
        ]);

        assert_eq!(index.count(), 1);
        assert!(!index.has_note(".obsidian/plugin.md"));
        assert!(!index.has_note("sub/.draft.md"));
    }

    #[test]
    fn a_new_folder_is_read_by_an_update() {
        let vault = Vault::new(&[("a.md", "x")]);
        let index = vault.index();
        vault.write("fresh/one.md", "x");
        vault.write("fresh/deeper/two.md", "x");
        vault.write("fresh/pic.png", "x");

        index.update(&[vault.path("fresh")]);

        assert!(index.has_note("fresh/one.md"));
        assert!(index.has_note("fresh/deeper/two.md"));
        assert_eq!(
            index.resolve_asset("pic.png", "a.md"),
            Some("fresh/pic.png".into())
        );
    }

    #[test]
    fn a_renamed_folder_is_read_again_and_the_old_prefix_removed() {
        let vault = Vault::new(&[
            ("old/a.md", "zebra\n"),
            ("old/deep/b.md", "x"),
            ("c.md", "x"),
        ]);
        let index = vault.index();
        fs::rename(vault.path("old"), vault.path("new")).unwrap();

        index.update(&[vault.path("old"), vault.path("new")]);

        assert_eq!(found(&index, "zebra"), ["new/a.md"]);
        assert!(index.has_note("new/deep/b.md"));
        assert!(!index.has_note("old/a.md"));
        assert!(!index.has_note("old/deep/b.md"));
        assert_eq!(index.count(), 3);
    }

    #[test]
    fn a_deleted_folder_removes_everything_below_it() {
        let vault = Vault::new(&[("old/a.md", "x"), ("old/pic.png", "x"), ("keep.md", "x")]);
        let index = vault.index();
        fs::remove_dir_all(vault.path("old")).unwrap();

        index.update(&[vault.path("old")]);

        assert_eq!(index.count(), 1);
        assert_eq!(index.resolve_asset("pic.png", "keep.md"), None);
    }

    #[test]
    fn a_renamed_file_is_found_under_its_new_path_only() {
        let vault = Vault::new(&[("a.md", "zebra\n")]);
        let index = vault.index();
        fs::rename(vault.path("a.md"), vault.path("b.md")).unwrap();

        index.update(&[vault.path("a.md"), vault.path("b.md")]);

        assert_eq!(found(&index, "zebra"), ["b.md"]);
    }

    #[test]
    fn a_created_image_can_be_resolved_after_an_update() {
        let vault = Vault::new(&[("a.md", "x")]);
        let index = vault.index();
        vault.write("img/cat.png", "x");

        index.update(&[vault.path("img/cat.png")]);

        assert_eq!(
            index.resolve_asset("cat.png", "a.md"),
            Some("img/cat.png".into())
        );
    }

    #[test]
    fn a_link_names_a_note_by_name_ignoring_case_and_the_md_extension() {
        let index = resolver_vault().index();

        for target in ["Home", "home", "HOME", "Home.md", "home.MD", "Home.Md"] {
            assert_eq!(
                index.resolve_note(target, None),
                Some("Home.md".into()),
                "{target}"
            );
        }
    }

    #[test]
    fn heading_and_block_references_are_ignored_when_resolving_a_note() {
        let index = resolver_vault().index();

        for target in ["Home#Section", "Home^abc123", "Home.md#Section", "Home#a^b"] {
            assert_eq!(
                index.resolve_note(target, None),
                Some("Home.md".into()),
                "{target}"
            );
        }
    }

    #[test]
    fn an_empty_target_resolves_to_the_note_it_is_written_in() {
        let index = resolver_vault().index();

        for target in ["", "#Heading", "^block", "  "] {
            assert_eq!(
                index.resolve_note(target, Some("Home.md")),
                Some("Home.md".into()),
                "{target:?}"
            );
            assert_eq!(index.resolve_note(target, None), None, "{target:?}");
        }
    }

    #[test]
    fn a_name_shared_by_several_notes_resolves_to_the_shortest_path() {
        let index = resolver_vault().index();

        assert_eq!(
            index.resolve_note("Plan", None),
            Some("Projects/Plan.md".into())
        );
    }

    #[test]
    fn an_alias_resolves_when_no_note_has_that_name() {
        let index = resolver_vault().index();

        assert_eq!(
            index.resolve_note("roadmap", None),
            Some("Projects/Plan.md".into())
        );
        assert_eq!(
            index.resolve_note("Nickname", None),
            Some("Notes/Aliased.md".into())
        );
    }

    #[test]
    fn a_note_name_beats_an_alias_of_another_note() {
        let vault = Vault::new(&[
            ("deep/er/Roadmap.md", ""),
            ("Plan.md", "---\naliases: [Roadmap]\n---\n"),
        ]);

        assert_eq!(
            vault.index().resolve_note("Roadmap", None),
            Some("deep/er/Roadmap.md".into())
        );
    }

    #[test]
    fn a_link_with_a_folder_resolves_the_exact_path_first() {
        let index = resolver_vault().index();

        assert_eq!(
            index.resolve_note("Projects/Plan", Some("Other/Sub/Detail.md")),
            Some("Projects/Plan.md".into())
        );
        assert_eq!(
            index.resolve_note("Projects/Plan.md", None),
            Some("Projects/Plan.md".into())
        );
    }

    #[test]
    fn a_link_with_a_folder_resolves_relative_to_the_note_before_any_suffix_match() {
        let index = resolver_vault().index();

        assert_eq!(
            index.resolve_note("Sub/Detail", Some("Projects/Plan.md")),
            Some("Projects/Sub/Detail.md".into())
        );
        assert_eq!(
            index.resolve_note("Sub/Detail", Some("Other/Home.md")),
            Some("Other/Sub/Detail.md".into())
        );
        assert_eq!(
            index.resolve_note("../Other/Sub/Detail", Some("Projects/Plan.md")),
            Some("Other/Sub/Detail.md".into())
        );
    }

    #[test]
    fn a_link_with_a_folder_falls_back_to_the_shortest_suffix_match() {
        let index = resolver_vault().index();

        assert_eq!(
            index.resolve_note("Sub/Detail", None),
            Some("Other/Sub/Detail.md".into())
        );
        assert_eq!(
            index.resolve_note("Old/Projects/Plan", None),
            Some("Archive/Old/Projects/Plan.md".into())
        );
    }

    #[test]
    fn a_link_with_a_folder_matches_a_root_level_path_ignoring_case() {
        let index = resolver_vault().index();

        for (target, expected) in [
            ("projects/plan", "Projects/Plan.md"),
            ("PROJECTS/PLAN.md", "Projects/Plan.md"),
            ("other/sub/detail", "Other/Sub/Detail.md"),
        ] {
            assert_eq!(
                index.resolve_note(target, None),
                Some(expected.into()),
                "{target}"
            );
        }
    }

    #[test]
    fn an_unknown_note_does_not_resolve() {
        let index = resolver_vault().index();

        for target in ["Nope", "Projects/Nope", "Nope.md#Heading"] {
            assert_eq!(
                index.resolve_note(target, Some("Home.md")),
                None,
                "{target}"
            );
        }
    }

    fn asset_vault() -> Vault {
        Vault::new(&[
            ("Photo.png", "x"),
            ("img/Photo.png", "x"),
            ("My Pic.png", "x"),
            ("Projects/img/cat.png", "x"),
            ("Other/img/cat.png", "x"),
            ("Home.md", ""),
        ])
    }

    #[test]
    fn an_image_link_resolves_an_exact_path_first() {
        let index = asset_vault().index();

        assert_eq!(
            index.resolve_asset("img/Photo.png", "Projects/Note.md"),
            Some("img/Photo.png".into())
        );
        assert_eq!(
            index.resolve_asset("Photo.png", "Projects/Note.md"),
            Some("Photo.png".into())
        );
    }

    #[test]
    fn an_image_link_drops_the_size_alias_and_decodes_the_path() {
        let index = asset_vault().index();

        assert_eq!(
            index.resolve_asset("Photo.png|200", "Home.md"),
            Some("Photo.png".into())
        );
        assert_eq!(
            index.resolve_asset("My%20Pic.png", "Home.md"),
            Some("My Pic.png".into())
        );
        assert_eq!(
            index.resolve_asset(" My%20Pic.png | 300x200 ", "Home.md"),
            Some("My Pic.png".into())
        );
    }

    #[test]
    fn an_image_link_resolves_relative_to_the_note_before_the_shortest_name_match() {
        let index = asset_vault().index();

        assert_eq!(
            index.resolve_asset("img/cat.png", "Projects/Note.md"),
            Some("Projects/img/cat.png".into())
        );
        assert_eq!(
            index.resolve_asset("img/cat.png", "Other/Note.md"),
            Some("Other/img/cat.png".into())
        );
    }

    #[test]
    fn an_image_link_falls_back_to_the_shortest_path_with_that_file_name() {
        let index = asset_vault().index();

        assert_eq!(
            index.resolve_asset("cat.png", "Home.md"),
            Some("Other/img/cat.png".into())
        );
        assert_eq!(
            index.resolve_asset("somewhere/CAT.PNG", "Home.md"),
            Some("Other/img/cat.png".into())
        );
    }

    #[test]
    fn an_unknown_image_does_not_resolve() {
        let index = asset_vault().index();

        assert_eq!(index.resolve_asset("dog.png", "Home.md"), None);
        assert_eq!(index.resolve_asset("Home.md", "Home.md"), None);
    }

    #[test]
    fn a_wikilink_is_the_note_name() {
        let index = resolver_vault().index();

        assert_eq!(index.wikilink_for("Home.md"), "[[Home]]");
        assert_eq!(index.wikilink_for("Notes/Aliased.md"), "[[Aliased]]");
    }

    #[test]
    fn a_wikilink_includes_the_path_when_two_notes_share_the_name() {
        let index = resolver_vault().index();

        assert_eq!(index.wikilink_for("Projects/Plan.md"), "[[Projects/Plan]]");
        assert_eq!(
            index.wikilink_for("Archive/Old/Projects/Plan.md"),
            "[[Archive/Old/Projects/Plan]]"
        );
    }

    #[test]
    fn a_wikilink_for_an_unindexed_note_uses_the_file_name() {
        let index = resolver_vault().index();

        assert_eq!(index.wikilink_for("Elsewhere/Missing.md"), "[[Missing]]");
    }

    #[test]
    fn the_index_tells_which_notes_it_holds() {
        let index = resolver_vault().index();

        assert!(index.has_note("Home.md"));
        assert!(!index.has_note("Home"));
        assert!(!index.has_note("Nope.md"));
    }
}
