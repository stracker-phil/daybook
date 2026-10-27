//! What the active context's music settings ask for when a call starts or ends, the owner goes on a
//! break or comes back (`steps`), and with the day's first Working (`first_working_steps`); and what
//! each step does to Music as it is now (`action`).

use serde::{Deserialize, Serialize};

use super::state::{Player, PlayerState};
use crate::store::settings::{ContextMusic, MusicEntry, OnCall};

/// The volume a call lowered, kept across a quit or crash mid-call.
pub const HELD_FILE: &str = "music-held.json";

/// Within this many points of the lowered volume, the volume counts as untouched since the call began.
const UNTOUCHED_TOLERANCE: u8 = 1;

/// The two things that hold the music back.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Moment {
    pub in_call: bool,
    /// On Break or Done for today.
    pub away: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Step {
    Pause,
    Resume,
    Lower { volume: u8 },
    Restore,
    Start { entry: String },
}

/// The volume before a call and the one it was lowered to.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lowered {
    pub from: u8,
    pub to: u8,
}

/// What Daybook did to the music and hasn't undone yet.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Held {
    pub paused: bool,
    pub lowered: Option<Lowered>,
}

/// Pausing on Break and Off and starting with the workday need the context's status feature; what
/// happens during calls doesn't.
pub fn effective(settings: &ContextMusic, has_status: bool) -> ContextMusic {
    if has_status {
        return settings.clone();
    }
    ContextMusic {
        pause_when_away: false,
        start_with: String::new(),
        ..settings.clone()
    }
}

/// The day's first Working only starts the context's entry; music paused the day before stays paused.
pub fn first_working_steps(now: Moment, settings: &ContextMusic) -> Vec<Step> {
    if settings.start_with.is_empty() || now.in_call {
        return Vec::new();
    }
    vec![Step::Start {
        entry: settings.start_with.clone(),
    }]
}

/// The steps from `before` to `now`, in order.
pub fn steps(before: Moment, now: Moment, held: &Held, settings: &ContextMusic) -> Vec<Step> {
    let mut steps = Vec::new();
    let call_started = !before.in_call && now.in_call;
    let call_ended = before.in_call && !now.in_call;
    if call_started {
        match settings.on_call {
            OnCall::Pause => steps.push(Step::Pause),
            OnCall::Lower => steps.push(Step::Lower {
                volume: settings.call_volume,
            }),
            OnCall::Nothing => {}
        }
    }
    if call_ended && held.lowered.is_some() {
        steps.push(Step::Restore);
    }
    if !before.away && now.away && settings.pause_when_away {
        steps.push(Step::Pause);
    }
    let back = call_ended || (before.away && !now.away);
    if back && settings.resume && held.paused && !now.in_call && !now.away {
        steps.push(Step::Resume);
    }
    steps
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Action {
    Pause,
    Resume,
    Volume(u8),
    Play(String),
}

/// A volume changed by hand during the call stays.
fn untouched_since(change: Lowered, volume: u8) -> bool {
    volume.abs_diff(change.to) <= UNTOUCHED_TOLERANCE
}

/// What `step` does to Music as `player` shows it, updating `held`; `None` when Music isn't in the
/// state for it or the entry to start is no longer in `entries`.
pub fn action(
    step: &Step,
    player: &Player,
    held: &mut Held,
    entries: &[MusicEntry],
) -> Option<Action> {
    match step {
        Step::Pause => {
            if !player.playing() {
                return None;
            }
            held.paused = true;
            Some(Action::Pause)
        }
        Step::Resume => {
            held.paused = false;
            // Still as Daybook left it: not closed, stopped or started again by hand.
            (player.state == PlayerState::Paused).then_some(Action::Resume)
        }
        Step::Lower { volume } => {
            let from = player
                .volume
                .filter(|from| player.playing() && from > volume)?;
            held.lowered = Some(Lowered { from, to: *volume });
            Some(Action::Volume(*volume))
        }
        Step::Restore => {
            let change = held.lowered.take()?;
            untouched_since(change, player.volume?).then_some(Action::Volume(change.from))
        }
        Step::Start { entry } => {
            let known = entries.iter().any(|item| &item.id == entry);
            (known && !player.playing()).then(|| Action::Play(entry.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::MusicKind;

    const WORKING: Moment = Moment {
        in_call: false,
        away: false,
    };
    const IN_CALL: Moment = Moment {
        in_call: true,
        away: false,
    };
    const ON_BREAK: Moment = Moment {
        in_call: false,
        away: true,
    };
    const UNTOUCHED: Held = Held {
        paused: false,
        lowered: None,
    };
    const PAUSED: Held = Held {
        paused: true,
        lowered: None,
    };

    fn settings() -> ContextMusic {
        ContextMusic {
            on_call: OnCall::Lower,
            call_volume: 2,
            pause_when_away: true,
            resume: true,
            start_with: "focus".into(),
        }
    }

    fn off() -> ContextMusic {
        ContextMusic {
            on_call: OnCall::Nothing,
            call_volume: 2,
            pause_when_away: false,
            resume: false,
            start_with: String::new(),
        }
    }

    fn player(state: PlayerState, volume: Option<u8>) -> Player {
        Player {
            state,
            track: None,
            volume,
            playlist: None,
        }
    }

    fn entry(id: &str) -> MusicEntry {
        MusicEntry {
            id: id.into(),
            name: id.into(),
            kind: MusicKind::Playlist,
            source: id.into(),
            artist_id: String::new(),
            storefront: String::new(),
            shuffle: true,
        }
    }

    #[test]
    fn lowers_the_volume_for_a_call_and_restores_it_afterwards() {
        assert_eq!(
            steps(WORKING, IN_CALL, &UNTOUCHED, &settings()),
            vec![Step::Lower { volume: 2 }]
        );
        let lowered = Held {
            paused: false,
            lowered: Some(Lowered { from: 60, to: 2 }),
        };
        assert_eq!(
            steps(IN_CALL, WORKING, &lowered, &settings()),
            vec![Step::Restore]
        );
    }

    #[test]
    fn pauses_for_a_call_and_resumes_what_it_paused() {
        let pausing = ContextMusic {
            on_call: OnCall::Pause,
            ..settings()
        };
        assert_eq!(
            steps(WORKING, IN_CALL, &UNTOUCHED, &pausing),
            vec![Step::Pause]
        );
        assert_eq!(
            steps(IN_CALL, WORKING, &PAUSED, &pausing),
            vec![Step::Resume]
        );
        assert_eq!(
            steps(IN_CALL, WORKING, &UNTOUCHED, &pausing),
            Vec::<Step>::new()
        );
    }

    #[test]
    fn pauses_on_a_break_and_resumes_when_back() {
        assert_eq!(
            steps(WORKING, ON_BREAK, &UNTOUCHED, &settings()),
            vec![Step::Pause]
        );
        assert_eq!(
            steps(ON_BREAK, WORKING, &PAUSED, &settings()),
            vec![Step::Resume]
        );
    }

    #[test]
    fn doesnt_resume_while_something_still_holds_the_music_back() {
        let both = Moment {
            in_call: true,
            away: true,
        };
        assert_eq!(
            steps(both, ON_BREAK, &PAUSED, &settings()),
            Vec::<Step>::new()
        );
        assert_eq!(
            steps(both, IN_CALL, &PAUSED, &settings()),
            Vec::<Step>::new()
        );
    }

    #[test]
    fn starts_the_contexts_entry_with_the_days_first_working_and_only_then() {
        assert_eq!(
            first_working_steps(WORKING, &settings()),
            vec![Step::Start {
                entry: "focus".into()
            }]
        );
        assert_eq!(
            first_working_steps(IN_CALL, &settings()),
            Vec::<Step>::new()
        );
        let none = ContextMusic {
            start_with: String::new(),
            ..settings()
        };
        assert_eq!(first_working_steps(WORKING, &none), Vec::<Step>::new());
    }

    #[test]
    fn leaves_the_music_alone_when_everything_is_off() {
        let nothing = Vec::<Step>::new();
        assert_eq!(steps(WORKING, IN_CALL, &UNTOUCHED, &off()), nothing);
        assert_eq!(steps(WORKING, ON_BREAK, &UNTOUCHED, &off()), nothing);
        assert_eq!(steps(ON_BREAK, WORKING, &PAUSED, &off()), nothing);
    }

    #[test]
    fn without_the_status_feature_only_the_call_settings_remain() {
        assert_eq!(
            effective(&settings(), false),
            ContextMusic {
                pause_when_away: false,
                start_with: String::new(),
                ..settings()
            }
        );
        assert_eq!(effective(&settings(), true), settings());
    }

    #[test]
    fn pause_acts_only_on_playing_music_and_remembers_it() {
        let mut held = Held::default();
        let paused = player(PlayerState::Paused, Some(50));
        assert_eq!(action(&Step::Pause, &paused, &mut held, &[]), None);
        assert_eq!(held, Held::default());

        let playing = player(PlayerState::Playing, Some(50));
        assert_eq!(
            action(&Step::Pause, &playing, &mut held, &[]),
            Some(Action::Pause)
        );
        assert!(held.paused);
    }

    #[test]
    fn resume_forgets_the_pause_and_only_resumes_paused_music() {
        for (state, expected) in [
            (PlayerState::Paused, Some(Action::Resume)),
            (PlayerState::Playing, None),
            (PlayerState::Stopped, None),
            (PlayerState::Closed, None),
        ] {
            let mut held = PAUSED;
            assert_eq!(
                action(&Step::Resume, &player(state, Some(50)), &mut held, &[]),
                expected,
                "{state:?}"
            );
            assert!(!held.paused, "{state:?}");
        }
    }

    #[test]
    fn lower_needs_playing_music_with_a_volume_above_the_target() {
        let lower = Step::Lower { volume: 2 };
        for (state, volume) in [
            (PlayerState::Paused, Some(50)),
            (PlayerState::Playing, None),
            (PlayerState::Playing, Some(2)),
            (PlayerState::Playing, Some(1)),
        ] {
            let mut held = Held::default();
            assert_eq!(
                action(&lower, &player(state, volume), &mut held, &[]),
                None,
                "{state:?} {volume:?}"
            );
            assert_eq!(held, Held::default(), "{state:?} {volume:?}");
        }

        let mut held = Held::default();
        assert_eq!(
            action(
                &lower,
                &player(PlayerState::Playing, Some(60)),
                &mut held,
                &[]
            ),
            Some(Action::Volume(2))
        );
        assert_eq!(held.lowered, Some(Lowered { from: 60, to: 2 }));
    }

    #[test]
    fn restore_returns_the_volume_unless_it_was_changed_by_hand() {
        for (now, expected) in [
            (Some(2), Some(Action::Volume(60))),
            (Some(3), Some(Action::Volume(60))),
            (Some(1), Some(Action::Volume(60))),
            (Some(30), None),
            (None, None),
        ] {
            let mut held = Held {
                paused: false,
                lowered: Some(Lowered { from: 60, to: 2 }),
            };
            assert_eq!(
                action(
                    &Step::Restore,
                    &player(PlayerState::Playing, now),
                    &mut held,
                    &[]
                ),
                expected,
                "{now:?}"
            );
            assert_eq!(held.lowered, None, "{now:?}");
        }
    }

    #[test]
    fn staying_in_a_call_changes_nothing_even_with_a_lowered_volume() {
        let lowered = Held {
            paused: true,
            lowered: Some(Lowered { from: 60, to: 2 }),
        };
        for held in [&UNTOUCHED, &PAUSED, &lowered] {
            assert_eq!(
                steps(IN_CALL, IN_CALL, held, &settings()),
                Vec::<Step>::new(),
                "{held:?}"
            );
        }
    }

    #[test]
    fn a_volume_lowered_before_a_restart_is_restored_unless_changed_by_hand() {
        let persisted: Lowered =
            serde_json::from_value(serde_json::json!({ "from": 60, "to": 2 })).unwrap();
        assert_eq!(persisted, Lowered { from: 60, to: 2 });
        let restored = Held {
            paused: false,
            lowered: Some(persisted),
        };

        for (volume, expected) in [(2, Some(Action::Volume(60))), (40, None)] {
            let mut held = restored;
            assert_eq!(
                action(
                    &Step::Restore,
                    &player(PlayerState::Playing, Some(volume)),
                    &mut held,
                    &[]
                ),
                expected,
                "{volume}"
            );
            assert_eq!(held.lowered, None, "{volume}");
        }
    }

    #[test]
    fn the_lowered_volume_is_stored_as_from_and_to() {
        assert_eq!(
            serde_json::to_value(Lowered { from: 60, to: 2 }).unwrap(),
            serde_json::json!({ "from": 60, "to": 2 })
        );
    }

    #[test]
    fn restore_without_a_lowered_volume_does_nothing() {
        let mut held = Held::default();
        assert_eq!(
            action(
                &Step::Restore,
                &player(PlayerState::Playing, Some(2)),
                &mut held,
                &[]
            ),
            None
        );
    }

    #[test]
    fn start_needs_an_existing_entry_and_music_that_isnt_playing() {
        let start = Step::Start {
            entry: "focus".into(),
        };
        let mut held = Held::default();
        let idle = player(PlayerState::Closed, None);
        let playing = player(PlayerState::Playing, Some(50));
        let entries = [entry("focus")];
        assert_eq!(
            action(&start, &idle, &mut held, &entries),
            Some(Action::Play("focus".into()))
        );
        assert_eq!(action(&start, &idle, &mut held, &[]), None);
        assert_eq!(action(&start, &playing, &mut held, &entries), None);
    }
}
