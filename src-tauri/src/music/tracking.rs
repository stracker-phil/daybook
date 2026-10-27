//! Which entry plays: a playlist is known by its name; a station Daybook started counts only while
//! nothing interrupted it (a skip, a stop or another track before the last one ended). Times in ms.

use super::state::{Player, PlayerState};
use crate::store::settings::{MusicEntry, MusicKind};

/// A station changes tracks while it starts.
pub const START_GRACE_MS: i64 = 20_000;
/// A track counts as played to its end within this many seconds of it.
pub const END_SLACK_S: f64 = 8.0;

/// Where the current track stands; 0 when Music doesn't say.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Playback {
    pub position: f64,
    pub duration: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Seen {
    pub playback: Playback,
    pub track: Option<String>,
    pub at: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracking {
    /// The entry Daybook started last.
    pub started: Option<String>,
    pub started_at: i64,
    /// Nothing interrupted it since.
    pub unbroken: bool,
    pub seen: Option<Seen>,
}

pub fn started_entry(tracking: &Tracking, id: &str, now: i64) -> Tracking {
    Tracking {
        started: Some(id.to_string()),
        started_at: now,
        unbroken: true,
        seen: tracking.seen.clone(),
    }
}

/// Follows one read of Music.
pub fn follow_player(tracking: &Tracking, next: &Player, playback: Playback, now: i64) -> Tracking {
    let past_grace = now - tracking.started_at > START_GRACE_MS;
    let unbroken =
        tracking.unbroken && !(past_grace && interrupted(tracking.seen.as_ref(), next, now));
    Tracking {
        unbroken,
        seen: Some(Seen {
            playback,
            track: next.track.clone(),
            at: now,
        }),
        ..tracking.clone()
    }
}

/// Music stopped, or another track came before the one seen last ended (or its length was unknown).
fn interrupted(seen: Option<&Seen>, next: &Player, now: i64) -> bool {
    if matches!(next.state, PlayerState::Closed | PlayerState::Stopped) {
        return true;
    }
    let Some(seen) = seen.filter(|seen| next.track != seen.track) else {
        return false;
    };
    let elapsed_s = (now - seen.at) as f64 / 1000.0;
    let left_s = seen.playback.duration - seen.playback.position;
    seen.playback.duration == 0.0 || elapsed_s < left_s - END_SLACK_S
}

/// The playlist Music names, whoever started it; else the station Daybook started, while unbroken.
pub fn current_entry(
    entries: &[MusicEntry],
    player: &Player,
    tracking: &Tracking,
) -> Option<String> {
    if matches!(player.state, PlayerState::Closed | PlayerState::Stopped) {
        return None;
    }
    if let Some(entry) = entries.iter().find(|entry| {
        entry.kind == MusicKind::Playlist && Some(&entry.source) == player.playlist.as_ref()
    }) {
        return Some(entry.id.clone());
    }
    let last = entries
        .iter()
        .find(|entry| Some(&entry.id) == tracking.started.as_ref())?;
    (last.kind != MusicKind::Playlist && tracking.unbroken && player.playlist.is_none())
        .then(|| last.id.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    const AFTER_GRACE: i64 = 60_000;

    fn entry(id: &str, name: &str, kind: MusicKind, source: &str) -> MusicEntry {
        MusicEntry {
            id: id.into(),
            name: name.into(),
            kind,
            source: source.into(),
            artist_id: String::new(),
            storefront: String::new(),
            shuffle: true,
        }
    }

    fn entries() -> Vec<MusicEntry> {
        vec![
            entry("focus", "Focus", MusicKind::Playlist, "Deep Focus"),
            entry("cell", "Cell", MusicKind::Artist, "Cell"),
        ]
    }

    fn playing(track: &str, playlist: Option<&str>) -> Player {
        Player {
            state: PlayerState::Playing,
            track: Some(track.into()),
            volume: Some(50),
            playlist: playlist.map(String::from),
        }
    }

    fn at(position: f64, duration: f64) -> Playback {
        Playback { position, duration }
    }

    fn started() -> Tracking {
        started_entry(&Tracking::default(), "cell", 0)
    }

    fn heard() -> Tracking {
        follow_player(
            &started(),
            &playing("First", None),
            at(10.0, 200.0),
            AFTER_GRACE,
        )
    }

    #[test]
    fn the_current_entry_is_the_playlist_music_names_whoever_started_it() {
        let none = Tracking::default();
        assert_eq!(
            current_entry(&entries(), &playing("Song", Some("Deep Focus")), &none),
            Some("focus".to_string())
        );
        assert_eq!(
            current_entry(&entries(), &playing("Song", Some("Other")), &none),
            None
        );
    }

    #[test]
    fn the_current_entry_is_nothing_while_music_is_closed_or_stopped() {
        let stopped = Player {
            state: PlayerState::Stopped,
            ..playing("Song", None)
        };
        assert_eq!(current_entry(&entries(), &Player::CLOSED, &started()), None);
        assert_eq!(current_entry(&entries(), &stopped, &started()), None);
    }

    #[test]
    fn the_current_entry_is_the_station_daybook_started_only_while_nothing_interrupted_it() {
        let started = started();
        assert_eq!(
            current_entry(&entries(), &playing("Song", None), &started),
            Some("cell".to_string())
        );
        assert_eq!(
            current_entry(&entries(), &playing("Song", Some("Deep Focus")), &started),
            Some("focus".to_string())
        );
        let broken = Tracking {
            unbroken: false,
            ..started
        };
        assert_eq!(
            current_entry(&entries(), &playing("Song", None), &broken),
            None
        );
    }

    #[test]
    fn track_changes_right_after_the_start_are_ignored() {
        let soon = follow_player(&started(), &playing("First", None), at(0.0, 200.0), 1_000);
        let next = follow_player(&soon, &playing("Second", None), at(0.0, 200.0), 5_000);
        assert!(next.unbroken);
    }

    #[test]
    fn a_track_that_ran_to_its_end_keeps_the_station_unbroken() {
        let next = follow_player(
            &heard(),
            &playing("Second", None),
            at(0.0, 180.0),
            AFTER_GRACE + 195_000,
        );
        assert!(next.unbroken);
    }

    #[test]
    fn a_skip_a_stop_or_a_track_of_unknown_length_break_the_station() {
        let skipped = follow_player(
            &heard(),
            &playing("Second", None),
            at(0.0, 180.0),
            AFTER_GRACE + 30_000,
        );
        assert!(!skipped.unbroken);

        let stopped = Player {
            state: PlayerState::Stopped,
            ..playing("First", None)
        };
        let after_stop = follow_player(&heard(), &stopped, at(0.0, 0.0), AFTER_GRACE + 1);
        assert!(!after_stop.unbroken);

        let unknown = follow_player(
            &started(),
            &playing("First", None),
            at(0.0, 0.0),
            AFTER_GRACE,
        );
        let changed = follow_player(
            &unknown,
            &playing("Second", None),
            at(0.0, 0.0),
            AFTER_GRACE + 300_000,
        );
        assert!(!changed.unbroken);
    }

    #[test]
    fn tracking_starts_over_once_an_entry_is_started_again() {
        let broken = follow_player(
            &heard(),
            &playing("Second", None),
            at(0.0, 180.0),
            AFTER_GRACE + 30_000,
        );
        assert!(!broken.unbroken);
        assert!(started_entry(&broken, "cell", AFTER_GRACE + 31_000).unbroken);
    }
}
