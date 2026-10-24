//! Joining a call (Settings → Calls → Joining a Call): the apps to start first, then the link in the app
//! chosen for its service.

use std::path::Path;
use std::time::Duration;

use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;

use super::checks::CHROME_BUNDLE;
use super::detect::Probe;
use super::probe::MacProbe;
use crate::calendar::events::{CallApp, MeetingLink};
use crate::error::{AppError, AppResult};
use crate::process;
use crate::state::AppState;
use crate::store::settings::{AppRef, OpenWith};

/// Time for an app started just now (a virtual camera) to be ready before the call app looks for it.
const START_WAIT: Duration = Duration::from_millis(2500);
const OPEN_TIMEOUT: Duration = Duration::from_secs(10);

/// Arguments for `open`: the app macOS has for the link without a choice, else the chosen app. A Chrome
/// web app (the Meet app) can't take a link itself: Chrome opens it in that app's window, the way it
/// launches a web app's shortcut (a second Chrome hands the arguments to the running one).
pub fn open_args(url: &str, app: Option<&AppRef>) -> Vec<String> {
    let Some(app) = app else {
        return vec![url.to_string()];
    };
    let web_app = app
        .bundle_id
        .strip_prefix(CHROME_BUNDLE)
        .and_then(|rest| rest.strip_prefix(".app."));
    match web_app {
        Some(app_id) => vec![
            "-n".into(),
            "-b".into(),
            CHROME_BUNDLE.into(),
            "--args".into(),
            format!("--app-id={app_id}"),
            format!("--app-launch-url-for-shortcuts-menu-item={url}"),
        ],
        None => vec!["-b".into(), app.bundle_id.clone(), url.to_string()],
    }
}

fn open_with(open_with: &OpenWith, app: CallApp) -> Option<&AppRef> {
    match app {
        CallApp::Zoom => open_with.zoom.as_ref(),
        CallApp::Teams => open_with.teams.as_ref(),
        CallApp::Meet => open_with.meet.as_ref(),
    }
}

fn open(args: Vec<String>) -> Result<(), String> {
    let output =
        process::run(Path::new("/usr/bin/open"), &args, OPEN_TIMEOUT).map_err(|e| e.to_string())?;
    if output.success {
        Ok(())
    } else {
        Err(output.stderr.trim().to_string())
    }
}

/// Starts the apps that aren't running yet, in the background; true when one was started.
fn start_first(apps: &[AppRef]) -> bool {
    if apps.is_empty() {
        return false;
    }
    let running = MacProbe::default().running().unwrap_or_default();
    let mut started = false;
    for app in apps.iter().filter(|app| !running.contains(&app.bundle_id)) {
        match open(vec!["-g".into(), "-b".into(), app.bundle_id.clone()]) {
            Ok(()) => started = true,
            Err(e) => log::warn!("Couldn't start {} before the call: {e}", app.app),
        }
    }
    started
}

fn open_default(app: &AppHandle, url: &str) -> AppResult<()> {
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Io(format!("Couldn't open the meeting: {e}")))
}

pub async fn join(app: &AppHandle, link: MeetingLink) -> AppResult<()> {
    let joining = app.state::<AppState>().settings.get().joining;
    let apps = joining.start_first.clone();
    let started = tauri::async_runtime::spawn_blocking(move || start_first(&apps))
        .await
        .unwrap_or(false);
    if started {
        tokio::time::sleep(START_WAIT).await;
    }
    let Some(chosen) = open_with(&joining.open_with, link.app).cloned() else {
        return open_default(app, &link.url);
    };
    log::info!("Joining the {} call in {}", link.app.name(), chosen.app);
    if let Err(e) = open_blocking(open_args(&link.url, Some(&chosen))).await {
        log::warn!(
            "Couldn't open the call in {}, using the default app: {e}",
            chosen.app
        );
        return open_default(app, &link.url);
    }
    Ok(())
}

async fn open_blocking(args: Vec<String>) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || open(args))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::MEET_BUNDLE;

    fn app(name: &str, bundle_id: &str) -> AppRef {
        AppRef {
            app: name.into(),
            bundle_id: bundle_id.into(),
        }
    }

    #[test]
    fn a_join_link_opens_in_the_chosen_app_and_a_chrome_web_app_through_chrome() {
        let url = "https://meet.google.com/abc-defg-hij";
        assert_eq!(open_args(url, None), vec![url]);
        assert_eq!(
            open_args(url, Some(&app("Zoom", "us.zoom.xos"))),
            vec!["-b", "us.zoom.xos", url]
        );
        assert_eq!(
            open_args(url, Some(&app("Google Meet", MEET_BUNDLE))),
            vec![
                "-n".to_string(),
                "-b".into(),
                "com.google.Chrome".into(),
                "--args".into(),
                "--app-id=kjgfgldnnfoeklkmfkjfagphfepbbdan".into(),
                format!("--app-launch-url-for-shortcuts-menu-item={url}"),
            ]
        );
    }
}
