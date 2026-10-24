//! An app picked in Settings: its name and bundle id.

use std::path::Path;
use std::time::Duration;

use crate::error::{AppError, AppResult};
use crate::process;
use crate::store::settings::AppRef;

const PLUTIL_TIMEOUT: Duration = Duration::from_secs(5);

fn bundle_id(app_path: &Path) -> AppResult<String> {
    let plist = app_path.join("Contents/Info.plist");
    let args = vec![
        "-extract".into(),
        "CFBundleIdentifier".into(),
        "raw".into(),
        "-o".into(),
        "-".into(),
        plist.to_string_lossy().into_owned(),
    ];
    let output = process::run(Path::new("/usr/bin/plutil"), &args, PLUTIL_TIMEOUT)
        .map_err(|e| AppError::Io(e.to_string()))?;
    let id = output.stdout.trim();
    if !output.success || id.is_empty() {
        return Err(AppError::InvalidInput(format!(
            "“{}” has no bundle ID.",
            app_path.display()
        )));
    }
    Ok(id.to_string())
}

/// The app at `path`, named after its bundle (`Zoom.app` → Zoom). Blocking.
pub fn app_ref(path: &Path) -> AppResult<AppRef> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let Some(app) = name.strip_suffix(".app") else {
        return Err(AppError::InvalidInput(format!("“{name}” isn’t an app.")));
    };
    Ok(AppRef {
        app: app.to_string(),
        bundle_id: bundle_id(path)?,
    })
}
