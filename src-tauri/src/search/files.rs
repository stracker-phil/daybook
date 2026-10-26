//! Reading the vault's files for the index: which files count and what each adds.

use std::fs;
use std::path::{Component, Path};
use std::sync::LazyLock;
use std::time::UNIX_EPOCH;

use regex::Regex;

use super::note::{self, IndexedNote};

/// Larger notes aren't searched.
pub const MAX_NOTE_BYTES: u64 = 2 * 1024 * 1024;

static IMAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\.(png|jpe?g|gif|svg|webp|avif|bmp)$").unwrap());

pub fn is_image(path: &str) -> bool {
    IMAGE.is_match(path)
}

pub fn is_markdown(path: &str) -> bool {
    path.ends_with(".md")
}

/// Whether a path below the vault has a segment starting with `.` (`.obsidian/`, `.trash/`, `.DS_Store`).
pub fn is_hidden(relative: &Path) -> bool {
    relative
        .components()
        .any(|part| part.as_os_str().to_string_lossy().starts_with('.'))
}

/// `relative` as a vault path (`/`-separated); `None` for anything but plain segments.
pub fn vault_path(relative: &Path) -> Option<String> {
    relative
        .components()
        .map(|part| match part {
            Component::Normal(part) => part.to_str(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .filter(|parts| !parts.is_empty())
        .map(|parts| parts.join("/"))
}

/// What a file adds to the index.
pub enum FileContent {
    Note(Box<IndexedNote>),
    Asset(String),
    /// Missing, unreadable, too large or neither a note nor an image.
    Gone(String),
}

fn modified_ms(metadata: &fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .and_then(|since| i64::try_from(since.as_millis()).ok())
        .unwrap_or(0)
}

pub fn read_file(root: &Path, path: String) -> FileContent {
    let file = root.join(&path);
    let metadata = match fs::metadata(&file) {
        Ok(metadata) if metadata.is_file() => metadata,
        _ => return FileContent::Gone(path),
    };
    if is_image(&path) {
        return FileContent::Asset(path);
    }
    if !is_markdown(&path) || metadata.len() > MAX_NOTE_BYTES {
        return FileContent::Gone(path);
    }
    match fs::read(&file) {
        Ok(bytes) => {
            let text = String::from_utf8_lossy(&bytes);
            FileContent::Note(Box::new(note::parse(&path, &text, modified_ms(&metadata))))
        }
        Err(e) => {
            log::warn!("Couldn't read {}: {e}", file.display());
            FileContent::Gone(path)
        }
    }
}

/// The notes and images below `dir` (vault-relative), without dot entries and symlinks.
pub fn walk(root: &Path, dir: &Path, notes: &mut Vec<String>, assets: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(root.join(dir)) else {
        return;
    };
    for entry in entries.flatten() {
        add_entry(root, dir, &entry, notes, assets);
    }
}

fn add_entry(
    root: &Path,
    dir: &Path,
    entry: &fs::DirEntry,
    notes: &mut Vec<String>,
    assets: &mut Vec<String>,
) {
    let name = entry.file_name();
    if name.to_string_lossy().starts_with('.') {
        return;
    }
    let relative = dir.join(&name);
    let Ok(kind) = entry.file_type() else {
        return;
    };
    if kind.is_dir() {
        return walk(root, &relative, notes, assets);
    }
    let Some(path) = vault_path(&relative).filter(|_| kind.is_file()) else {
        return;
    };
    if is_image(&path) {
        assets.push(path);
    } else if is_markdown(&path) {
        notes.push(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, text: &str) {
        let file = root.join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, text).unwrap();
    }

    fn walked(root: &Path) -> (Vec<String>, Vec<String>) {
        let (mut notes, mut assets) = (Vec::new(), Vec::new());
        walk(root, Path::new(""), &mut notes, &mut assets);
        notes.sort();
        assets.sort();
        (notes, assets)
    }

    #[test]
    fn images_are_recognized_by_extension_in_any_case() {
        for (path, expected) in [
            ("a.png", true),
            ("a.PNG", true),
            ("a.jpg", true),
            ("a.jpeg", true),
            ("a.gif", true),
            ("a.svg", true),
            ("a.webp", true),
            ("a.avif", true),
            ("a.bmp", true),
            ("folder/a.Jpeg", true),
            ("a.pdf", false),
            ("a.md", false),
            ("png", false),
            ("a.png.md", false),
        ] {
            assert_eq!(is_image(path), expected, "{path}");
        }
    }

    #[test]
    fn a_path_with_a_dot_segment_is_hidden() {
        for (path, expected) in [
            (".obsidian/app.json", true),
            ("notes/.trash/a.md", true),
            (".DS_Store", true),
            ("notes/.hidden.md", true),
            ("notes/a.md", false),
            ("a.b/c.md", false),
        ] {
            assert_eq!(is_hidden(Path::new(path)), expected, "{path}");
        }
    }

    #[test]
    fn walking_lists_notes_and_images_and_skips_dot_entries_and_other_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "a.md", "a");
        write(root, "sub/b.md", "b");
        write(root, "sub/deep/c.md", "c");
        write(root, "pic.PNG", "x");
        write(root, "sub/photo.jpg", "x");
        write(root, "readme.txt", "x");
        write(root, ".obsidian/hidden.md", "x");
        write(root, ".hidden.md", "x");
        write(root, "sub/.secret.png", "x");

        let (notes, assets) = walked(root);

        assert_eq!(notes, ["a.md", "sub/b.md", "sub/deep/c.md"]);
        assert_eq!(assets, ["pic.PNG", "sub/photo.jpg"]);
    }

    #[test]
    fn walking_starts_below_the_given_folder() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "a.md", "a");
        write(root, "sub/b.md", "b");
        let (mut notes, mut assets) = (Vec::new(), Vec::new());

        walk(root, Path::new("sub"), &mut notes, &mut assets);

        assert_eq!(notes, ["sub/b.md"]);
        assert!(assets.is_empty());
    }

    #[test]
    fn walking_a_missing_folder_finds_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let (mut notes, mut assets) = (Vec::new(), Vec::new());

        walk(dir.path(), Path::new("nope"), &mut notes, &mut assets);

        assert!(notes.is_empty());
        assert!(assets.is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn walking_skips_symlinks_to_files_and_folders() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(root, "real.md", "a");
        write(root, "sub/inner.md", "b");
        write(root, "pic.png", "x");
        std::os::unix::fs::symlink(root.join("real.md"), root.join("link.md")).unwrap();
        std::os::unix::fs::symlink(root.join("pic.png"), root.join("link.png")).unwrap();
        std::os::unix::fs::symlink(root.join("sub"), root.join("linked-folder")).unwrap();

        let (notes, assets) = walked(root);

        assert_eq!(notes, ["real.md", "sub/inner.md"]);
        assert_eq!(assets, ["pic.png"]);
    }

    #[test]
    fn a_note_is_read_with_its_parsed_fields() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "Projects/Plan.md", "# Goals\nShip it\n");

        let FileContent::Note(note) = read_file(dir.path(), "Projects/Plan.md".into()) else {
            panic!("expected a note");
        };

        assert_eq!(note.path, "Projects/Plan.md");
        assert_eq!(note.name, "Plan");
        assert_eq!(note.folder, "Projects");
        assert_eq!(note.lines, ["# Goals", "Ship it"]);
    }

    #[test]
    fn a_note_of_exactly_the_size_limit_is_still_read() {
        let dir = tempfile::tempdir().unwrap();
        let file = fs::File::create(dir.path().join("edge.md")).unwrap();
        file.set_len(MAX_NOTE_BYTES).unwrap();

        assert!(matches!(
            read_file(dir.path(), "edge.md".into()),
            FileContent::Note(_)
        ));
    }

    #[test]
    fn a_note_over_the_size_limit_is_gone() {
        let dir = tempfile::tempdir().unwrap();
        let file = fs::File::create(dir.path().join("big.md")).unwrap();
        file.set_len(MAX_NOTE_BYTES + 1).unwrap();

        assert!(matches!(
            read_file(dir.path(), "big.md".into()),
            FileContent::Gone(path) if path == "big.md"
        ));
    }

    #[test]
    fn an_image_is_read_as_an_asset() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "img/cat.png", "x");

        assert!(matches!(
            read_file(dir.path(), "img/cat.png".into()),
            FileContent::Asset(path) if path == "img/cat.png"
        ));
    }

    #[test]
    fn a_large_image_is_still_an_asset() {
        let dir = tempfile::tempdir().unwrap();
        let file = fs::File::create(dir.path().join("big.png")).unwrap();
        file.set_len(MAX_NOTE_BYTES + 1).unwrap();

        assert!(matches!(
            read_file(dir.path(), "big.png".into()),
            FileContent::Asset(_)
        ));
    }

    #[test]
    fn a_missing_file_a_folder_and_other_file_types_are_gone() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "readme.txt", "x");
        write(dir.path(), "folder.md/inner.md", "x");

        for path in ["missing.md", "readme.txt", "folder.md"] {
            assert!(
                matches!(read_file(dir.path(), path.into()), FileContent::Gone(gone) if gone == path),
                "{path}"
            );
        }
    }
}
