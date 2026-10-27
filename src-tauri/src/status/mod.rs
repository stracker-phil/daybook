//! Working, Break and Off: Daybook's own status (docs/status.md). Today's status lives in status
//! memory; a change is announced as `status_changed`, which the features that react to it listen to.

mod memory;
mod request;

pub use memory::{StatusMemory, Statuses};
pub use request::{OfferOff, offers_off};

use request::{status_request, toggled};

use std::time::Duration;

use chrono::Local;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use ts_rs::TS;

use crate::bus::{Command, CommandBus};
use crate::error::AppResult;
use crate::events::STATUS_CHANGED_EVENT;
use crate::state::AppState;
use crate::store::settings::{Settings, Status};
use crate::{action_log, focus, slack, tray, window};

const OVERLAY: &str = "status";
/// The menu-bar title's break minutes move with the clock, and a new day starts without a status.
const TICK: Duration = Duration::from_secs(60);

/// The answer to the end-of-day check.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum EndOfDayAnswer {
    Done,
    Incomplete,
}

/// The payload of `status_changed`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct StatusChanged {
    /// `None`: the day's first status.
    pub previous: Option<Status>,
    pub next: Status,
    /// The day's first Working.
    pub first: bool,
    /// `HH:MM`
    pub since: String,
    /// The channel message as confirmed in the overlay; empty sends none.
    pub message: String,
    /// Going Off through the end-of-day check.
    pub end_of_day: Option<EndOfDayAnswer>,
    /// The message already went out as the end of the end-of-day reply.
    pub message_sent: bool,
    /// Picking an activity already starts its timer, so the status change must not start another.
    #[serde(default)]
    pub by_activity: bool,
}

/// How the surfaces name a status.
pub fn status_label(status: Option<Status>) -> &'static str {
    match status {
        Some(Status::Working) => "Working and present",
        Some(Status::Break) => "Taking a break",
        Some(Status::Off) => "Done for today",
        None => "No status yet",
    }
}

/// The action log's title of a status change: "Status changed: Working → Break", "Status set: Working".
pub fn status_trigger(previous: Option<Status>, next: Status) -> String {
    let name = |status| match status {
        Status::Working => "Working",
        Status::Break => "Break",
        Status::Off => "Off",
    };
    match previous {
        Some(previous) => format!("Status changed: {} → {}", name(previous), name(next)),
        None => format!("Status set: {}", name(next)),
    }
}

fn single_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Sets today's status and announces the change; picking today's status again does nothing.
pub fn set_status(
    app: &AppHandle,
    status: Status,
    message: &str,
    end_of_day: Option<EndOfDayAnswer>,
    message_sent: bool,
) -> AppResult<()> {
    change_status(app, status, message, end_of_day, message_sent, false)
}

/// Working for an activity picked during a break or after the day: no channel message.
pub fn set_working_by_activity(app: &AppHandle) -> AppResult<()> {
    change_status(app, Status::Working, "", None, false, true)
}

fn change_status(
    app: &AppHandle,
    status: Status,
    message: &str,
    end_of_day: Option<EndOfDayAnswer>,
    message_sent: bool,
    by_activity: bool,
) -> AppResult<()> {
    let state = app.state::<AppState>();
    let Some(change) = state.statuses.set(status, Local::now().naive_local())? else {
        return Ok(());
    };
    let trigger = action_log::begin(&status_trigger(change.previous, status));
    let _entered = trigger.enter();
    log::info!(
        "Status changed: {} → {}",
        status_label(change.previous),
        status_label(Some(status))
    );
    let changed = StatusChanged {
        previous: change.previous,
        next: status,
        first: change.first_working,
        since: change.since,
        message: single_line(message),
        end_of_day: end_of_day.filter(|_| status == Status::Off),
        message_sent,
        by_activity,
    };
    if let Err(e) = app.emit(STATUS_CHANGED_EVENT, changed) {
        log::warn!("Couldn't announce the status change: {e}");
    }
    tray::refresh(app);
    Ok(())
}

/// Today's status memory.
pub fn today(app: &AppHandle) -> StatusMemory {
    app.state::<AppState>()
        .statuses
        .today(Local::now().naive_local())
}

/// Going Off asks for the end-of-day list while the Off workflow is on and the list goes to Slack.
fn asks_for_end_of_day_list(app: &AppHandle, settings: &Settings) -> bool {
    settings.slack.manual_workflows.off && slack::day::asks_for_end_of_day_list(app)
}

/// Writes and posts the end-of-day list with the message as its last paragraph; `true`: the message went
/// out with it. Nothing when Off is today's status already or its workflow is off.
async fn send_end_of_day_list(
    app: &AppHandle,
    list: &str,
    message: &str,
    end_of_day: Option<EndOfDayAnswer>,
) -> AppResult<bool> {
    let settings = app.state::<AppState>().settings.get();
    if !settings.slack.manual_workflows.off
        || !memory::is_status_change(today(app).status, Status::Off)
    {
        return Ok(false);
    }
    let closing = slack::channel::message_text(&settings, message, end_of_day);
    action_log::traced(
        "End-of-day list sent",
        focus::send_end_of_day_list(app, list, &closing),
    )
    .await
}

/// Opens the status overlay with `status` picked; the same request again closes it.
async fn open(app: &AppHandle, status: Status) -> AppResult<()> {
    let settings = app.state::<AppState>().settings.get();
    let mut request = status_request(&settings, &today(app), status);
    request.end_of_day_list = asks_for_end_of_day_list(app, &settings);
    window::panel::open(app, OVERLAY, serde_json::to_value(request)?).await;
    Ok(())
}

/// The overlay's pick: sets the status and closes the overlay, also when nothing changed. Going Off
/// with the end-of-day list writes it to the note and posts it, with the message, as one reply first;
/// when that fails the status stays.
#[tauri::command]
pub async fn status_set(
    app: AppHandle,
    status: Status,
    message: String,
    end_of_day: Option<EndOfDayAnswer>,
    end_of_day_list: Option<String>,
) -> AppResult<()> {
    let message_sent = match end_of_day_list.filter(|list| !list.trim().is_empty()) {
        Some(list) if status == Status::Off => {
            send_end_of_day_list(&app, &list, &message, end_of_day).await?
        }
        _ => false,
    };
    set_status(&app, status, &message, end_of_day, message_sent)?;
    if window::panel::view(&app).as_deref() == Some(OVERLAY) {
        window::panel::close(&app, false);
    }
    Ok(())
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    let picks = [
        (
            "status:working",
            "Change Status to Working…",
            "Change Status… with Working picked",
            Status::Working,
        ),
        (
            "status:break",
            "Change Status to Break…",
            "Change Status… with Break picked",
            Status::Break,
        ),
        (
            "status:off",
            "Change Status to Done for Today…",
            "Change Status… with Done for Today picked",
            Status::Off,
        ),
    ];
    for (id, title, help, status) in picks {
        bus.add(
            Command::new(id, title)
                .help(help)
                .needs_vault()
                .run(move |app| async move { open(&app, status).await }),
        );
    }
    bus.add(
        Command::new("status:toggle", "Change Status…")
            .help("Change Status… (Working ↔ Break)")
            .needs_vault()
            .run(|app| async move { open(&app, toggled(today(&app).status)).await }),
    );
}

/// Starts the `TICK` that refreshes the menu-bar title.
pub fn start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticks = tokio::time::interval(TICK);
        loop {
            ticks.tick().await;
            tray::refresh(&app);
        }
    });
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn each_status_has_its_label() {
        for (status, label) in [
            (Some(Status::Working), "Working and present"),
            (Some(Status::Break), "Taking a break"),
            (Some(Status::Off), "Done for today"),
            (None, "No status yet"),
        ] {
            assert_eq!(status_label(status), label, "{status:?}");
        }
    }

    #[test]
    fn a_status_change_names_both_statuses() {
        for (previous, next, title) in [
            (
                Some(Status::Working),
                Status::Break,
                "Status changed: Working → Break",
            ),
            (
                Some(Status::Break),
                Status::Off,
                "Status changed: Break → Off",
            ),
            (
                Some(Status::Off),
                Status::Working,
                "Status changed: Off → Working",
            ),
        ] {
            assert_eq!(
                status_trigger(previous, next),
                title,
                "{previous:?} {next:?}"
            );
        }
    }

    #[test]
    fn the_days_first_status_reads_as_set() {
        for (next, title) in [
            (Status::Working, "Status set: Working"),
            (Status::Break, "Status set: Break"),
            (Status::Off, "Status set: Off"),
        ] {
            assert_eq!(status_trigger(None, next), title, "{next:?}");
        }
    }

    fn changed(previous: Option<Status>, end_of_day: Option<EndOfDayAnswer>) -> StatusChanged {
        StatusChanged {
            previous,
            next: Status::Off,
            first: false,
            since: "17:30".into(),
            message: "off for today".into(),
            end_of_day,
            message_sent: false,
            by_activity: false,
        }
    }

    #[test]
    fn a_status_change_serializes_with_camel_case_fields() {
        assert_eq!(
            serde_json::to_value(changed(Some(Status::Working), Some(EndOfDayAnswer::Done)))
                .unwrap(),
            json!({
                "previous": "working",
                "next": "off",
                "first": false,
                "since": "17:30",
                "message": "off for today",
                "endOfDay": "done",
                "messageSent": false,
                "byActivity": false
            })
        );
    }

    #[test]
    fn the_end_of_day_answer_is_done_incomplete_or_null() {
        for (answer, expected) in [
            (Some(EndOfDayAnswer::Done), json!("done")),
            (Some(EndOfDayAnswer::Incomplete), json!("incomplete")),
            (None, json!(null)),
        ] {
            assert_eq!(
                serde_json::to_value(changed(None, answer)).unwrap()["endOfDay"],
                expected,
                "{answer:?}"
            );
        }
    }

    #[test]
    fn a_status_change_round_trips_through_serde() {
        for change in [
            changed(Some(Status::Working), Some(EndOfDayAnswer::Incomplete)),
            changed(None, None),
        ] {
            let value = serde_json::to_value(&change).unwrap();
            assert_eq!(
                serde_json::from_value::<StatusChanged>(value).unwrap(),
                change
            );
        }
    }
}
