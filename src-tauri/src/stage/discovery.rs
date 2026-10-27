//! The legacy discovery file (protocol §3.2): `~/Library/Application Support/glaze-stage/apps/<id>.json`
//! tells a presenter where the hook answers. It exists while the HTTP API runs with Presentations on.

use std::fs;
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use super::protocol::DiscoveryRecord;

/// The folder Stage watches, under `home`.
pub fn folder(home: &Path) -> PathBuf {
    home.join("Library/Application Support/glaze-stage/apps")
}

pub fn file_in(folder: &Path, id: &str) -> PathBuf {
    folder.join(format!("{id}.json"))
}

/// Writes the record readable by this user only (the token is in it), aside first and then renamed,
/// so a presenter never reads half a file.
pub fn write(file: &Path, record: &DiscoveryRecord) -> std::io::Result<()> {
    if let Some(folder) = file.parent() {
        fs::create_dir_all(folder)?;
    }
    let mut text = serde_json::to_string_pretty(record)?;
    text.push('\n');
    let temporary = file.with_extension(format!("json.{}.tmp", std::process::id()));
    let written = write_aside(&temporary, file, &text);
    if written.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    written
}

fn write_aside(temporary: &Path, file: &Path, text: &str) -> std::io::Result<()> {
    let mut out = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(temporary)?;
    out.write_all(text.as_bytes())?;
    out.sync_all()?;
    fs::set_permissions(temporary, fs::Permissions::from_mode(0o600))?;
    fs::rename(temporary, file)
}

/// Removes the file; a missing one is fine.
pub fn remove(file: &Path) -> std::io::Result<()> {
    match fs::remove_file(file) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::Value;

    use super::*;
    use crate::stage::protocol::discovery_record;

    fn record(token: &str) -> DiscoveryRecord {
        discovery_record(
            "Daybook",
            "app.example",
            27180,
            token,
            42,
            Utc.with_ymd_and_hms(2026, 10, 15, 10, 0, 0).unwrap(),
        )
    }

    fn names(folder: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(folder)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn mode(file: &Path) -> u32 {
        fs::metadata(file).unwrap().permissions().mode() & 0o777
    }

    #[test]
    fn the_folder_is_stages_apps_folder_under_application_support() {
        assert_eq!(
            folder(Path::new("/Users/me")),
            PathBuf::from("/Users/me/Library/Application Support/glaze-stage/apps")
        );
    }

    #[test]
    fn the_file_is_named_after_the_app_id() {
        assert_eq!(
            file_in(Path::new("/apps"), "com.philippstracker.daybook"),
            PathBuf::from("/apps/com.philippstracker.daybook.json")
        );
    }

    #[test]
    fn writing_creates_the_folders_and_a_private_file_ending_in_a_newline() {
        let home = tempfile::tempdir().unwrap();
        let file = file_in(&folder(home.path()), "app.example");

        write(&file, &record("t")).unwrap();

        let text = fs::read_to_string(&file).unwrap();
        assert!(text.ends_with("}\n"), "{text:?}");
        assert_eq!(mode(&file), 0o600);
    }

    #[test]
    fn the_file_holds_the_record_pretty_printed_with_the_documented_keys_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let file = file_in(dir.path(), "app.example");

        write(&file, &record("t")).unwrap();

        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "{\n  \"protocol\": 1,\n  \"name\": \"Daybook\",\n  \"id\": \"app.example\",\n  \"url\": \"http://127.0.0.1:27180/v1/stage\",\n  \"token\": \"t\",\n  \"pid\": 42,\n  \"updatedAt\": \"2026-10-15T10:00:00.000Z\"\n}\n"
        );
        let parsed: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(parsed["url"], "http://127.0.0.1:27180/v1/stage");
    }

    #[test]
    fn writing_leaves_no_temporary_file_behind() {
        let dir = tempfile::tempdir().unwrap();
        let file = file_in(dir.path(), "app.example");

        write(&file, &record("t")).unwrap();
        write(&file, &record("u")).unwrap();

        assert_eq!(names(dir.path()), ["app.example.json"]);
    }

    #[test]
    fn writing_again_replaces_the_content() {
        let dir = tempfile::tempdir().unwrap();
        let file = file_in(dir.path(), "app.example");
        write(&file, &record("old")).unwrap();

        write(&file, &record("new")).unwrap();

        let parsed: Value = serde_json::from_str(&fs::read_to_string(&file).unwrap()).unwrap();
        assert_eq!(parsed["token"], "new");
    }

    #[test]
    fn a_file_that_was_readable_by_others_becomes_private() {
        let dir = tempfile::tempdir().unwrap();
        let file = file_in(dir.path(), "app.example");
        fs::write(&file, "old").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o644)).unwrap();

        write(&file, &record("t")).unwrap();

        assert_eq!(mode(&file), 0o600);
    }

    #[test]
    fn a_write_that_fails_leaves_no_temporary_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = file_in(dir.path(), "app.example");
        fs::create_dir(&file).unwrap();

        assert!(write(&file, &record("t")).is_err());

        assert_eq!(names(dir.path()), ["app.example.json"]);
    }

    #[test]
    fn removing_deletes_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = file_in(dir.path(), "app.example");
        write(&file, &record("t")).unwrap();

        remove(&file).unwrap();

        assert!(names(dir.path()).is_empty());
    }

    #[test]
    fn removing_a_missing_file_is_fine() {
        let dir = tempfile::tempdir().unwrap();

        assert!(remove(&file_in(dir.path(), "app.example")).is_ok());
        assert!(remove(&dir.path().join("no/such/folder/app.json")).is_ok());
    }
}
