//! The narrator's state: the automation state in short values, plus whether a vault is set. No
//! secrets. Pure.

use serde_json::{Map, Value, json};

use super::protocol::flat_state;
use crate::automation::AutomationState;

pub fn stage_state(state: &AutomationState, vault: bool) -> Map<String, Value> {
    let call = state
        .call
        .as_ref()
        .map(|c| format!("{}: {}", c.app, c.title));
    let all = [
        ("status", json!(state.status)),
        ("since", json!(state.since)),
        ("context", json!(state.context_title)),
        ("contextId", json!(state.context)),
        ("vault", json!(vault)),
        ("call", json!(call)),
        ("event", json!(state.event.as_ref().map(|e| &e.title))),
        ("music", json!(state.music.as_ref().map(|m| m.player))),
        (
            "track",
            json!(state.music.as_ref().and_then(|m| m.track.as_ref())),
        ),
        ("waiting", json!(state.waiting)),
    ];
    flat_state(all.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::automation::state::{CallNow, EventNow, MusicNow};
    use crate::music::state::PlayerState;
    use crate::store::settings::Status;

    fn idle() -> AutomationState {
        AutomationState {
            status: None,
            since: None,
            context: None,
            context_title: None,
            call: None,
            event: None,
            music: None,
            waiting: 0,
        }
    }

    fn busy() -> AutomationState {
        AutomationState {
            status: Some(Status::Working),
            since: Some("08:45".into()),
            context: Some("work".into()),
            context_title: Some("[W] Work".into()),
            call: Some(CallNow {
                app: "Zoom".into(),
                title: "Standup".into(),
                since: "10:05".into(),
            }),
            event: Some(EventNow {
                title: "Planning".into(),
                start: "10:00".into(),
                end: "11:30".into(),
            }),
            music: Some(MusicNow {
                player: PlayerState::Playing,
                track: Some("Song".into()),
            }),
            waiting: 3,
        }
    }

    #[test]
    fn nothing_going_on_is_all_nulls_and_zero() {
        assert_eq!(
            Value::Object(stage_state(&idle(), false)),
            json!({
                "status": null,
                "since": null,
                "context": null,
                "contextId": null,
                "vault": false,
                "call": null,
                "event": null,
                "music": null,
                "track": null,
                "waiting": 0,
            })
        );
    }

    #[test]
    fn everything_going_on_is_told_in_short_values() {
        assert_eq!(
            Value::Object(stage_state(&busy(), true)),
            json!({
                "status": "working",
                "since": "08:45",
                "context": "[W] Work",
                "contextId": "work",
                "vault": true,
                "call": "Zoom: Standup",
                "event": "Planning",
                "music": "playing",
                "track": "Song",
                "waiting": 3,
            })
        );
    }

    #[test]
    fn a_call_is_the_app_and_its_title() {
        let state = stage_state(&busy(), true);

        assert_eq!(state["call"], json!("Zoom: Standup"));
    }

    #[test]
    fn the_state_holds_no_nested_values() {
        let state = stage_state(&busy(), true);

        for (key, value) in &state {
            assert!(!value.is_object() && !value.is_array(), "{key}");
        }
    }

    #[test]
    fn the_vault_flag_is_the_callers() {
        assert_eq!(stage_state(&idle(), true)["vault"], json!(true));
        assert_eq!(stage_state(&idle(), false)["vault"], json!(false));
    }
}
