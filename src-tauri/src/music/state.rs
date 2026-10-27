//! What Music is doing, as read last, and the state the overlay, the menu and the rail show.

use serde::Serialize;
use ts_rs::TS;

use super::tracking::{Tracking, current_entry};
use crate::store::settings::MusicEntry;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum PlayerState {
    Playing,
    Paused,
    Stopped,
    /// Music isn't running.
    Closed,
}

/// Music as one read saw it.
#[derive(Clone, Debug, PartialEq)]
pub struct Player {
    pub state: PlayerState,
    /// `Name – Artist`.
    pub track: Option<String>,
    pub volume: Option<u8>,
    /// The playlist Music names; a station has none.
    pub playlist: Option<String>,
}

impl Player {
    pub const CLOSED: Player = Player {
        state: PlayerState::Closed,
        track: None,
        volume: None,
        playlist: None,
    };

    pub fn playing(&self) -> bool {
        self.state == PlayerState::Playing
    }
}

/// An entry of Settings → Music as the overlay lists it.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct MusicListItem {
    pub id: String,
    /// The name, else the source.
    pub name: String,
}

/// Payload of `music_changed` and the answer of every music command.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct MusicState {
    pub entries: Vec<MusicListItem>,
    pub player: PlayerState,
    pub track: Option<String>,
    pub volume: Option<u8>,
    /// The entry that plays or is paused.
    pub entry: Option<String>,
}

pub fn entry_label(entry: &MusicEntry) -> &str {
    if entry.name.is_empty() {
        &entry.source
    } else {
        &entry.name
    }
}

pub fn music_state(entries: &[MusicEntry], player: &Player, tracking: &Tracking) -> MusicState {
    MusicState {
        entries: entries
            .iter()
            .map(|entry| MusicListItem {
                id: entry.id.clone(),
                name: entry_label(entry).to_string(),
            })
            .collect(),
        player: player.state,
        track: player.track.clone(),
        volume: player.volume,
        entry: current_entry(entries, player, tracking),
    }
}
