//! macOS notifications: failures of commands nobody waits for, automatic switches and later reminders.
//! A notification's button runs a bus command: the button's id is its command id.

use tauri::{AppHandle, Manager};
use tauri_plugin_notification::{Action, ActionType, NotificationExt};

use crate::bus;

pub fn show(app: &AppHandle, title: &str, body: &str) {
    report(
        title,
        body,
        app.notification().builder().title(title).body(body).show(),
    );
}

/// A notification with a button that runs `command`.
pub fn show_with_action(app: &AppHandle, title: &str, body: &str, (command, label): (&str, &str)) {
    let notification = app.notification();
    let button = ActionType::builder(command)
        .actions(vec![Action::builder(command, label).build()])
        .build();
    if let Err(e) = notification.register_action_types(vec![button]) {
        log::warn!("Couldn't offer the notification's button: {e}");
    }
    let shown = notification
        .builder()
        .title(title)
        .body(body)
        .action_type_id(command)
        .show();
    report(title, body, shown);
}

fn report(title: &str, body: &str, shown: tauri_plugin_notification::Result<()>) {
    if let Err(e) = shown {
        log::warn!("Couldn't show a notification ({title}: {body}): {e}");
    }
}

/// Runs the command of a clicked button; a click on the notification itself runs nothing.
pub fn install(app: &AppHandle) {
    let handle = app.clone();
    let installed = app.notification().on_action(move |performed| {
        let command = performed.action_id();
        if handle.state::<bus::Commands>().knows(command) {
            bus::spawn(&handle, command);
        }
    });
    if let Err(e) = installed {
        log::warn!("Notification buttons won't work: {e}");
    }
}
