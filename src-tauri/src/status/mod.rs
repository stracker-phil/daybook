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
use crate::store::settings::Status;
use crate::{tray, window};

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
}

pub fn status_label(status: Option<Status>) -> &'static str {
    match status {
        Some(Status::Working) => "Working and present",
        Some(Status::Break) => "Taking a break",
        Some(Status::Off) => "Done for today",
        None => "No status yet",
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
) -> AppResult<()> {
    let state = app.state::<AppState>();
    let Some(change) = state.statuses.set(status, Local::now().naive_local())? else {
        return Ok(());
    };
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
    };
    if let Err(e) = app.emit(STATUS_CHANGED_EVENT, changed) {
        log::warn!("Couldn't announce the status change: {e}");
    }
    tray::refresh(app);
    Ok(())
}

pub fn today(app: &AppHandle) -> StatusMemory {
    app.state::<AppState>()
        .statuses
        .today(Local::now().naive_local())
}

/// Opens the status overlay with `status` picked; the same request again closes it.
async fn open(app: &AppHandle, status: Status) -> AppResult<()> {
    let settings = app.state::<AppState>().settings.get();
    let request = status_request(&settings, &today(app), status);
    window::panel::open(app, OVERLAY, serde_json::to_value(request)?).await;
    Ok(())
}

/// The overlay's pick: sets the status and closes the overlay, also when nothing changed.
#[tauri::command]
pub fn status_set(
    app: AppHandle,
    status: Status,
    message: String,
    end_of_day: Option<EndOfDayAnswer>,
) -> AppResult<()> {
    set_status(&app, status, &message, end_of_day)?;
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

    fn changed(previous: Option<Status>, end_of_day: Option<EndOfDayAnswer>) -> StatusChanged {
        StatusChanged {
            previous,
            next: Status::Off,
            first: false,
            since: "17:30".into(),
            message: "off for today".into(),
            end_of_day,
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
                "endOfDay": "done"
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
