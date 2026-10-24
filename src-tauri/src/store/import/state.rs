//! The prototype's state files in Daybook's shapes.

use serde_json::{Map, Value, json};

use crate::store::settings::Status;

type Convert = fn(&Value) -> Value;

/// Prototype file, Daybook file, conversion.
pub const STATE_FILES: &[(&str, &str, Convert)] = &[
    ("mode.json", "context.json", context),
    ("status-memory.json", "status-memory.json", status_memory),
    ("jira-tickets.json", "jira-tickets.json", jira_tickets),
    ("routines.json", "routines.json", routines),
    ("day-message.json", "day-message.json", day_message),
    ("slack-shown.json", "slack-shown.json", slack_shown),
    (
        "pending-entries.json",
        "pending-entries.json",
        pending_entries,
    ),
];

/// Note changes that only wrote the activity log.
const TRACKING_OPS: &[&str] = &["activity", "status", "meeting"];

fn text_or_null(value: &Value) -> Value {
    match value.as_str() {
        Some(text) if !text.is_empty() => json!(text),
        _ => Value::Null,
    }
}

fn text_or_empty(value: &Value) -> Value {
    json!(value.as_str().unwrap_or_default())
}

/// `"Sat Oct 24 2026"` (JavaScript's `toDateString`) → `"2026-10-24"`; empty when it isn't one.
pub fn iso_day(value: &Value) -> Value {
    let day = value
        .as_str()
        .and_then(|text| chrono::NaiveDate::parse_from_str(text, "%a %b %d %Y").ok())
        .map(|day| day.format("%Y-%m-%d").to_string());
    json!(day.unwrap_or_default())
}

fn context(p: &Value) -> Value {
    json!({ "id": text_or_null(&p["id"]), "setAt": text_or_null(&p["setAt"]) })
}

fn status_memory(p: &Value) -> Value {
    let status: Option<Status> = serde_json::from_value(p["status"].clone()).ok();
    json!({
        "date": iso_day(&p["date"]),
        "status": status,
        "since": text_or_null(&p["since"]),
        "workedToday": p["workedToday"].as_bool().unwrap_or(false),
    })
}

fn jira_tickets(p: &Value) -> Value {
    let days = p
        .as_object()
        .into_iter()
        .flatten()
        .filter_map(|(day, keys)| {
            let keys: Vec<&str> = keys.as_array()?.iter().filter_map(Value::as_str).collect();
            Some((day.clone(), json!(keys)))
        });
    Value::Object(days.collect::<Map<_, _>>())
}

fn routines(p: &Value) -> Value {
    let fired: Vec<&str> = p["fired"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    json!({ "date": text_or_empty(&p["date"]), "fired": fired })
}

fn day_message(p: &Value) -> Value {
    json!({
        "date": iso_day(&p["date"]),
        "channel": text_or_null(&p["channel"]),
        "ts": text_or_null(&p["ts"]),
        "greeting": text_or_empty(&p["greeting"]),
        "eodSent": p["eodSent"].as_bool().unwrap_or(false),
    })
}

fn slack_shown(p: &Value) -> Value {
    let presence = match p["presence"].as_str() {
        Some(presence @ ("auto" | "away")) => json!(presence),
        _ => Value::Null,
    };
    json!({
        "emoji": text_or_empty(&p["emoji"]),
        "text": text_or_empty(&p["text"]),
        "presence": presence,
        "expiresAt": p["expiresAt"].as_f64(),
    })
}

fn pending_entries(p: &Value) -> Value {
    let entries: Vec<&Value> = p["entries"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| {
            entry["file"].is_string()
                && entry["createdAt"].is_number()
                && entry["op"]["kind"]
                    .as_str()
                    .is_some_and(|kind| !TRACKING_OPS.contains(&kind))
        })
        .collect();
    json!({ "entries": entries })
}
