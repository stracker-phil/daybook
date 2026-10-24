//! Fixtures for unit tests and integration tests.

use crate::store::json_file::create_private_dir;

/// A fresh private directory standing in for the app's config dir; removed on drop.
pub fn temp_config_dir() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("create temp dir");
    create_private_dir(dir.path()).expect("make temp dir private");
    dir
}
