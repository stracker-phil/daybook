//! A note or an image of the vault for the search preview; paths from the page are checked first.

pub mod markdown;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

use self::markdown::{Links, preview_markdown};
use super::note::{body_start, split_lines};
use super::posix;
use crate::error::{AppError, AppResult};

/// Larger images aren't previewed.
const MAX_IMAGE_BYTES: u64 = 15 * 1024 * 1024;

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct NotePreview {
    pub path: String,
    pub name: String,
    /// `""` at the vault root.
    pub folder: String,
    /// The body without frontmatter, with links rewritten (`preview_markdown`).
    pub markdown: String,
}

/// A vault path from the page: relative, without `..` and NUL.
pub fn guard(path: &str) -> AppResult<()> {
    let invalid = path.is_empty()
        || path.starts_with('/')
        || path.contains('\0')
        || path.split('/').any(|segment| segment == "..");
    if invalid {
        return Err(AppError::InvalidInput("Invalid vault path".into()));
    }
    Ok(())
}

/// The existing file `relative` names, which has to be inside the vault after resolving symlinks.
pub fn vault_file(vault: &Path, relative: &str) -> AppResult<PathBuf> {
    guard(relative)?;
    let vault = vault
        .canonicalize()
        .map_err(|e| AppError::Io(format!("Couldn't open the vault: {e}")))?;
    let file = vault
        .join(relative)
        .canonicalize()
        .map_err(|_| AppError::NotFound(format!("{relative} doesn't exist.")))?;
    if file == vault || !file.starts_with(&vault) {
        return Err(AppError::InvalidInput(
            "That file is outside the vault.".into(),
        ));
    }
    Ok(file)
}

fn read_vault_file(file: &Path, relative: &str) -> AppResult<Vec<u8>> {
    fs::read(file).map_err(|e| AppError::Io(format!("Couldn't read {relative}: {e}")))
}

/// The note at `relative` without its frontmatter, rewritten for the preview.
pub fn note_preview(vault: &Path, relative: &str, links: &impl Links) -> AppResult<NotePreview> {
    let file = vault_file(vault, relative)?;
    let bytes = read_vault_file(&file, relative)?;
    let lines = split_lines(&String::from_utf8_lossy(&bytes));
    let body = &lines[body_start(&lines)..];
    let name = posix::basename(relative);
    Ok(NotePreview {
        path: relative.to_string(),
        name: name.strip_suffix(".md").unwrap_or(name).to_string(),
        folder: match posix::dirname(relative) {
            "." => String::new(),
            folder => folder.to_string(),
        },
        markdown: preview_markdown(body, relative, links),
    })
}

/// The image's type by its extension; `None` for anything but an image.
fn image_mime(path: &str) -> Option<&'static str> {
    let extension = path.rsplit_once('.')?.1.to_lowercase();
    Some(match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "bmp" => "image/bmp",
        _ => return None,
    })
}

/// The image at `relative` as a `data:` URL.
pub fn asset_data_url(vault: &Path, relative: &str) -> AppResult<String> {
    let mime =
        image_mime(relative).ok_or_else(|| AppError::InvalidInput("Not an image.".into()))?;
    let file = vault_file(vault, relative)?;
    let size = fs::metadata(&file)
        .map_err(|e| AppError::Io(format!("Couldn't read {relative}: {e}")))?
        .len();
    if size > MAX_IMAGE_BYTES {
        return Err(AppError::InvalidInput(
            "The image is too large to preview.".into(),
        ));
    }
    let bytes = read_vault_file(&file, relative)?;
    Ok(format!("data:{mime};base64,{}", STANDARD.encode(&bytes)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    struct NoLinks;

    impl Links for NoLinks {
        fn resolve_note(&self, _target: &str, _from: &str) -> Option<String> {
            None
        }

        fn resolve_asset(&self, _target: &str, _from: &str) -> Option<String> {
            None
        }

        fn has_note(&self, _path: &str) -> bool {
            false
        }
    }

    fn write(dir: &Path, relative: &str, bytes: &[u8]) {
        let file = dir.join(relative);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(file, bytes).unwrap();
    }

    fn vault_with(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, text) in files {
            write(dir.path(), path, text.as_bytes());
        }
        dir
    }

    fn invalid(message: &str) -> AppError {
        AppError::InvalidInput(message.into())
    }

    #[test]
    fn plain_relative_vault_paths_pass_the_guard() {
        for path in [
            "a.md",
            "folder/a.md",
            "a..b/c.md",
            "..hidden/a.md",
            "a/b/c.png",
        ] {
            assert_eq!(guard(path), Ok(()), "{path}");
        }
    }

    #[test]
    fn empty_absolute_parent_and_nul_paths_fail_the_guard() {
        for path in [
            "",
            "/",
            "/etc/passwd",
            "..",
            "../secret.md",
            "folder/../../secret.md",
            "folder/..",
            "a/../b.md",
            "a\0b.md",
        ] {
            assert_eq!(guard(path), Err(invalid("Invalid vault path")), "{path:?}");
        }
    }

    #[test]
    fn a_file_inside_the_vault_resolves_to_its_canonical_path() {
        let vault = vault_with(&[("folder/a.md", "x")]);

        let file = vault_file(vault.path(), "folder/a.md").unwrap();

        assert_eq!(
            file,
            vault.path().canonicalize().unwrap().join("folder/a.md")
        );
    }

    #[test]
    fn a_missing_file_is_not_found() {
        let vault = vault_with(&[]);

        let err = vault_file(vault.path(), "missing.md").unwrap_err();

        assert_eq!(err, AppError::NotFound("missing.md doesn't exist.".into()));
        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn a_guarded_path_is_refused_before_the_file_is_looked_up() {
        let vault = vault_with(&[("a.md", "x")]);

        for path in ["../a.md", "/a.md", ""] {
            assert_eq!(
                vault_file(vault.path(), path),
                Err(invalid("Invalid vault path")),
                "{path:?}"
            );
        }
    }

    #[test]
    fn the_vault_folder_itself_is_not_a_vault_file() {
        let vault = vault_with(&[("a.md", "x")]);

        assert_eq!(
            vault_file(vault.path(), "."),
            Err(invalid("That file is outside the vault."))
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_leading_outside_the_vault_is_refused() {
        let vault = vault_with(&[]);
        let outside = vault_with(&[("secret.md", "top secret"), ("dir/inner.png", "x")]);
        std::os::unix::fs::symlink(
            outside.path().join("secret.md"),
            vault.path().join("link.md"),
        )
        .unwrap();
        std::os::unix::fs::symlink(outside.path().join("dir"), vault.path().join("linked"))
            .unwrap();

        for path in ["link.md", "linked/inner.png"] {
            assert_eq!(
                vault_file(vault.path(), path),
                Err(invalid("That file is outside the vault.")),
                "{path}"
            );
        }
        assert_eq!(
            note_preview(vault.path(), "link.md", &NoLinks),
            Err(invalid("That file is outside the vault."))
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_staying_inside_the_vault_is_followed() {
        let vault = vault_with(&[("real/a.md", "hello")]);
        std::os::unix::fs::symlink(vault.path().join("real"), vault.path().join("alias")).unwrap();

        let preview = note_preview(vault.path(), "alias/a.md", &NoLinks).unwrap();

        assert_eq!(preview.markdown, "hello");
    }

    #[test]
    fn a_note_preview_drops_the_frontmatter_and_names_the_note() {
        let vault = vault_with(&[(
            "Projects/Plan.md",
            "---\ntitle: Plan\naliases: [Roadmap]\n---\n# Goals\nShip it\n",
        )]);

        let preview = note_preview(vault.path(), "Projects/Plan.md", &NoLinks).unwrap();

        assert_eq!(
            preview,
            NotePreview {
                path: "Projects/Plan.md".into(),
                name: "Plan".into(),
                folder: "Projects".into(),
                markdown: "# Goals\nShip it".into(),
            }
        );
    }

    #[test]
    fn a_note_at_the_vault_root_has_an_empty_folder() {
        let vault = vault_with(&[("Home.md", "no frontmatter\r\nsecond line\r\n")]);

        let preview = note_preview(vault.path(), "Home.md", &NoLinks).unwrap();

        assert_eq!(preview.folder, "");
        assert_eq!(preview.name, "Home");
        assert_eq!(preview.markdown, "no frontmatter\nsecond line");
    }

    #[test]
    fn a_nested_note_names_its_whole_folder_path() {
        let vault = vault_with(&[("a/b/c.md", "x")]);

        let preview = note_preview(vault.path(), "a/b/c.md", &NoLinks).unwrap();

        assert_eq!(preview.folder, "a/b");
    }

    #[test]
    fn previewing_a_missing_note_is_not_found() {
        let vault = vault_with(&[]);

        let err = note_preview(vault.path(), "gone.md", &NoLinks).unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn the_preview_serializes_with_the_field_names_the_page_reads() {
        let preview = NotePreview {
            path: "a/b.md".into(),
            name: "b".into(),
            folder: "a".into(),
            markdown: "text".into(),
        };

        assert_eq!(
            serde_json::to_value(&preview).unwrap(),
            serde_json::json!({
                "path": "a/b.md",
                "name": "b",
                "folder": "a",
                "markdown": "text",
            })
        );
    }

    #[test]
    fn an_image_type_follows_the_extension_in_any_case() {
        for (path, expected) in [
            ("a.png", Some("image/png")),
            ("a.PNG", Some("image/png")),
            ("a.jpg", Some("image/jpeg")),
            ("folder/a.JPEG", Some("image/jpeg")),
            ("a.gif", Some("image/gif")),
            ("a.svg", Some("image/svg+xml")),
            ("a.webp", Some("image/webp")),
            ("a.avif", Some("image/avif")),
            ("a.bmp", Some("image/bmp")),
            ("a.pdf", None),
            ("a.md", None),
            ("png", None),
        ] {
            assert_eq!(image_mime(path), expected, "{path}");
        }
    }

    #[test]
    fn an_image_becomes_a_data_url_with_its_exact_bytes() {
        let vault = vault_with(&[]);
        write(vault.path(), "img/pixel.png", &[0x89, b'P', b'N', b'G']);

        assert_eq!(
            asset_data_url(vault.path(), "img/pixel.png"),
            Ok("data:image/png;base64,iVBORw==".to_string())
        );
    }

    #[test]
    fn the_data_url_type_follows_the_extension() {
        let vault = vault_with(&[("a.svg", "<svg/>")]);

        assert_eq!(
            asset_data_url(vault.path(), "a.svg"),
            Ok("data:image/svg+xml;base64,PHN2Zy8+".to_string())
        );
    }

    #[test]
    fn a_file_that_is_not_an_image_is_refused() {
        let vault = vault_with(&[("note.md", "x"), ("doc.pdf", "x")]);

        for path in ["note.md", "doc.pdf", "missing.txt"] {
            assert_eq!(
                asset_data_url(vault.path(), path),
                Err(invalid("Not an image.")),
                "{path}"
            );
        }
    }

    #[test]
    fn a_missing_image_is_not_found() {
        let vault = vault_with(&[]);

        let err = asset_data_url(vault.path(), "gone.png").unwrap_err();

        assert_eq!(err.kind(), ErrorKind::NotFound);
    }

    #[test]
    fn an_image_path_leaving_the_vault_is_refused() {
        let vault = vault_with(&[]);

        assert_eq!(
            asset_data_url(vault.path(), "../outside.png"),
            Err(invalid("Invalid vault path"))
        );
    }

    #[test]
    fn an_image_above_the_size_limit_is_too_large_to_preview() {
        let vault = vault_with(&[]);
        let file = fs::File::create(vault.path().join("huge.png")).unwrap();
        file.set_len(MAX_IMAGE_BYTES + 1).unwrap();

        assert_eq!(
            asset_data_url(vault.path(), "huge.png"),
            Err(invalid("The image is too large to preview."))
        );
    }
}
