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
pub fn show_with_action(app: &AppHandle, title: &str, body: &str, button: (&str, &str)) {
    show_with_actions(app, title, body, &[button]);
}

/// A notification with buttons, each `(command, label)`.
pub fn show_with_actions(app: &AppHandle, title: &str, body: &str, buttons: &[(&str, &str)]) {
    let notification = app.notification();
    let type_id = buttons
        .iter()
        .map(|(command, _)| *command)
        .collect::<Vec<_>>()
        .join("+");
    let actions = buttons
        .iter()
        .map(|(command, label)| Action::builder(*command, *label).build())
        .collect();
    let kind = ActionType::builder(&type_id).actions(actions).build();
    if let Err(e) = notification.register_action_types(vec![kind]) {
        log::warn!("Couldn't offer the notification's buttons: {e}");
    }
    let shown = notification
        .builder()
        .title(title)
        .body(body)
        .action_type_id(&type_id)
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
