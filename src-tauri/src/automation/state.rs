//! What `state` reports: the status, the context, a call, the running event, music and the changes
//! waiting for today's note.

use chrono::{Local, NaiveDateTime};
use serde::Serialize;
use tauri::{AppHandle, Manager};

use crate::calendar::Event;
use crate::calendar::events::running_event;
use crate::calls::ActiveCall;
use crate::contexts::context_title;
use crate::music::state::{MusicState, PlayerState};
use crate::state::AppState;
use crate::status::StatusMemory;
use crate::store::settings::{Context, Status};
use crate::{calendar, calls, music, status};

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutomationState {
    /// `None`: no status yet today.
    pub status: Option<Status>,
    /// `HH:MM`
    pub since: Option<String>,
    /// The current context's id.
    pub context: Option<String>,
    pub context_title: Option<String>,
    pub call: Option<CallNow>,
    pub event: Option<EventNow>,
    /// `None` while Music isn't running.
    pub music: Option<MusicNow>,
    /// Changes waiting for today's note.
    pub waiting: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct CallNow {
    pub app: String,
    pub title: String,
    pub since: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct EventNow {
    pub title: String,
    pub start: String,
    pub end: String,
}

#[derive(Serialize, Clone, Debug, PartialEq)]
pub struct MusicNow {
    pub player: PlayerState,
    pub track: Option<String>,
}

fn clock(time: NaiveDateTime) -> String {
    time.format("%H:%M").to_string()
}

/// What `describe` needs, gathered by `now` (and by tests).
pub struct Parts<'a> {
    pub today: &'a StatusMemory,
    pub context: Option<&'a Context>,
    pub call: Option<&'a ActiveCall>,
    pub events: &'a [Event],
    pub now: NaiveDateTime,
    pub music: &'a MusicState,
    pub waiting: u32,
}

pub fn describe(parts: Parts) -> AutomationState {
    AutomationState {
        status: parts.today.status,
        since: parts.today.since.clone(),
        context: parts.context.map(|c| c.id.clone()),
        context_title: parts.context.map(|c| context_title(&c.label, &c.name)),
        call: parts.call.map(|call| CallNow {
            app: call.app.clone(),
            title: call.title.clone(),
            since: clock(call.since),
        }),
        event: running_event(parts.events, parts.now).map(|event| EventNow {
            title: event.title.clone(),
            start: clock(event.start),
            end: clock(event.end),
        }),
        music: (parts.music.player != PlayerState::Closed).then(|| MusicNow {
            player: parts.music.player,
            track: parts.music.track.clone(),
        }),
        waiting: parts.waiting,
    }
}

pub async fn now(app: &AppHandle) -> AutomationState {
    let now = Local::now().naive_local();
    let events = calendar::events_on(app, now.date(), now.date()).await;
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    describe(Parts {
        today: &status::today(app),
        context: state.contexts.current(&settings),
        call: calls::active(app).as_ref(),
        events: &events,
        now,
        music: &music::current(app),
        waiting: state.notes.state(&settings).waiting,
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use serde_json::json;

    use super::*;

    fn at(hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 12)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    fn event(title: &str, start: NaiveDateTime, end: NaiveDateTime) -> Event {
        Event {
            title: title.into(),
            calendar: "Work".into(),
            start,
            end,
            url: None,
            location: None,
            notes: None,
        }
    }

    fn music(player: PlayerState, track: Option<&str>) -> MusicState {
        MusicState {
            entries: Vec::new(),
            player,
            track: track.map(String::from),
            volume: Some(40),
            entry: None,
        }
    }

    fn context(label: &str, name: &str) -> Context {
        Context::read_all(&[json!({ "id": "work", "name": name, "label": label })]).remove(0)
    }

    fn describe_with(
        today: &StatusMemory,
        context: Option<&Context>,
        call: Option<&ActiveCall>,
        events: &[Event],
        music: &MusicState,
    ) -> AutomationState {
        describe(Parts {
            today,
            context,
            call,
            events,
            now: at(10, 30),
            music,
            waiting: 0,
        })
    }

    fn idle() -> AutomationState {
        describe_with(
            &StatusMemory::default(),
            None,
            None,
            &[],
            &music(PlayerState::Closed, None),
        )
    }

    #[test]
    fn nothing_going_on_describes_empty_fields() {
        assert_eq!(
            serde_json::to_value(idle()).unwrap(),
            json!({
                "status": null,
                "since": null,
                "context": null,
                "contextTitle": null,
                "call": null,
                "event": null,
                "music": null,
                "waiting": 0,
            })
        );
    }

    #[test]
    fn everything_going_on_is_described_in_camel_case() {
        let today = StatusMemory {
            date: "2026-10-12".into(),
            status: Some(Status::Working),
            since: Some("08:45".into()),
            worked_today: true,
        };
        let work = context("[W]", "Work");
        let call = ActiveCall {
            app: "Zoom".into(),
            title: "Standup".into(),
            since: at(10, 5),
        };
        let events = [event("Planning", at(10, 0), at(11, 30))];

        let state = describe(Parts {
            today: &today,
            context: Some(&work),
            call: Some(&call),
            events: &events,
            now: at(10, 30),
            music: &music(PlayerState::Playing, Some("Song")),
            waiting: 3,
        });

        assert_eq!(
            serde_json::to_value(state).unwrap(),
            json!({
                "status": "working",
                "since": "08:45",
                "context": "work",
                "contextTitle": "[W] Work",
                "call": { "app": "Zoom", "title": "Standup", "since": "10:05" },
                "event": { "title": "Planning", "start": "10:00", "end": "11:30" },
                "music": { "player": "playing", "track": "Song" },
                "waiting": 3,
            })
        );
    }

    #[test]
    fn the_context_title_is_the_name_alone_without_a_label() {
        let work = context("", "Work");

        let state = describe_with(
            &StatusMemory::default(),
            Some(&work),
            None,
            &[],
            &music(PlayerState::Closed, None),
        );

        assert_eq!(state.context.as_deref(), Some("work"));
        assert_eq!(state.context_title.as_deref(), Some("Work"));
    }

    #[test]
    fn the_event_is_the_one_running_now() {
        let events = [
            event("Earlier", at(8, 0), at(9, 0)),
            event("Running", at(10, 0), at(11, 0)),
            event("Later", at(12, 0), at(13, 0)),
        ];

        let state = describe_with(
            &StatusMemory::default(),
            None,
            None,
            &events,
            &music(PlayerState::Closed, None),
        );

        assert_eq!(
            state.event,
            Some(EventNow {
                title: "Running".into(),
                start: "10:00".into(),
                end: "11:00".into(),
            })
        );
    }

    #[test]
    fn no_event_is_described_while_none_is_running() {
        let events = [
            event("Earlier", at(8, 0), at(10, 30)),
            event("Later", at(12, 0), at(13, 0)),
        ];

        let state = describe_with(
            &StatusMemory::default(),
            None,
            None,
            &events,
            &music(PlayerState::Closed, None),
        );

        assert_eq!(state.event, None);
    }

    #[test]
    fn music_is_null_only_while_music_is_closed() {
        for (player, expected) in [
            (PlayerState::Closed, None),
            (
                PlayerState::Playing,
                Some(json!({ "player": "playing", "track": "Song" })),
            ),
            (
                PlayerState::Paused,
                Some(json!({ "player": "paused", "track": "Song" })),
            ),
            (
                PlayerState::Stopped,
                Some(json!({ "player": "stopped", "track": "Song" })),
            ),
        ] {
            let state = describe_with(
                &StatusMemory::default(),
                None,
                None,
                &[],
                &music(player, Some("Song")),
            );

            let value = serde_json::to_value(state).unwrap();
            assert_eq!(
                value["music"],
                expected.unwrap_or(json!(null)),
                "{player:?}"
            );
        }
    }

    #[test]
    fn a_track_that_is_unknown_is_null() {
        let state = describe_with(
            &StatusMemory::default(),
            None,
            None,
            &[],
            &music(PlayerState::Stopped, None),
        );

        assert_eq!(
            serde_json::to_value(state).unwrap()["music"],
            json!({ "player": "stopped", "track": null })
        );
    }
}
