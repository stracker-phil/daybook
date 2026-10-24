//! macOS notifications: failures of commands nobody waits for, and later reminders and automatic switches.

use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

pub fn show(app: &AppHandle, title: &str, body: &str) {
    if let Err(e) = app.notification().builder().title(title).body(body).show() {
        log::warn!("Couldn't show a notification ({title}: {body}): {e}");
    }
}
