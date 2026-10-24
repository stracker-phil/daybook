//! The prototype's `settings.json` in Daybook's shape: `clients` became projects, `modes` contexts,
//! the `tracking` feature `status`, and every key that only served time tracking is left behind.

use serde_json::{Map, Value, json};

use crate::store::settings::Settings;

/// What of the prototype's file Daybook leaves behind, as the import report names it.
pub const DROPPED: &[&str] = &[
    "dailyNote.activity (the activity section)",
    "config.activityFormat.activity, .working, .afk (activity log lines)",
    "config.statusLogLines (status lines in the activity log)",
    "config.clients[].color (day recap colours)",
];

pub fn from_prototype(prototype: &Value) -> Settings {
    Settings::from_json(&daybook_shape(prototype))
}

fn daybook_shape(p: &Value) -> Value {
    let c = &p["config"];
    let format = &c["activityFormat"];
    let mut shape = Map::new();
    // A missing part stays out, so it gets its default.
    let mut put = |key: &str, value: Value| {
        if !value.is_null() {
            shape.insert(key.into(), value);
        }
    };
    put(
        "vault",
        present(json!({
            "path": p["vaultPath"],
            "name": c["vaultName"],
            "obsidianCliPath": p["obsidianCliPath"],
        })),
    );
    put("dailyNote", daily_note(p, c));
    put(
        "statusMessages",
        present(json!({
            "working": format["workingNote"],
            "firstWorking": format["firstWorkingNote"],
            "break": format["breakNote"],
            "off": format["offNote"],
        })),
    );
    put(
        "slack",
        present(json!({
            "channel": p["slackChannel"],
            "statuses": c["slack"],
            "manualWorkflows": c["manualWorkflows"],
        })),
    );
    put(
        "meetings",
        present(json!({ "detection": p["meetingDetection"], "rules": c["meetings"] })),
    );
    put(
        "search",
        present(json!({ "scopes": c["searchScopes"], "exclude": c["exclude"] })),
    );
    put("projects", each(&c["clients"], project));
    put("contexts", each(&c["modes"], context));
    put("links", each(&c["links"], link));
    put("routines", each(&c["routines"], routine));
    for key in [
        "endOfDay",
        "focus",
        "statusRules",
        "timeline",
        "joining",
        "music",
        "journal",
        "reminders",
    ] {
        put(key, c[key].clone());
    }
    for key in ["rail", "tools", "hotkeys", "api", "jira"] {
        put(key, p[key].clone());
    }
    Value::Object(shape)
}

fn daily_note(p: &Value, c: &Value) -> Value {
    let note = &p["dailyNote"];
    let config = &c["dailyNote"];
    let order = note["order"].as_array().map(|order| {
        order
            .iter()
            .filter(|section| section.as_str() != Some("activity"))
            .cloned()
            .collect::<Vec<_>>()
    });
    present(json!({
        "fallbackFolder": config["fallbackFolder"],
        "fallbackFormat": config["fallbackFormat"],
        "notes": config["sections"]["notes"],
        "journal": note["journal"],
        "focus": note["focus"],
        "plan": note["plan"],
        "order": order,
    }))
}

fn project(item: &mut Map<String, Value>) {
    item.remove("color");
    rename(item, "modes", "contexts");
    rename(item, "projects", "launcherProjects");
}

fn context(item: &mut Map<String, Value>) {
    if let Some(Value::Array(features)) = item.get_mut("features") {
        for feature in features {
            if feature.as_str() == Some("tracking") {
                *feature = json!("status");
            }
        }
    }
}

fn link(item: &mut Map<String, Value>) {
    rename(item, "modes", "contexts");
    rename(item, "clients", "projects");
}

fn routine(item: &mut Map<String, Value>) {
    rename(item, "modes", "contexts");
}

/// A list with `edit` applied to each object in it; anything else stays as it is.
fn each(list: &Value, edit: fn(&mut Map<String, Value>)) -> Value {
    let mut list = list.clone();
    if let Value::Array(items) = &mut list {
        items
            .iter_mut()
            .filter_map(Value::as_object_mut)
            .for_each(edit);
    }
    list
}

fn rename(item: &mut Map<String, Value>, from: &str, to: &str) {
    if let Some(value) = item.remove(from) {
        item.insert(to.into(), value);
    }
}

/// The object without its missing (`null`) keys, so each one gets its default.
fn present(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            Value::Object(map.into_iter().filter(|(_, v)| !v.is_null()).collect())
        }
        other => other,
    }
}
