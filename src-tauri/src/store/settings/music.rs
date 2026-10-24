//! What the Music tool can play, and what each context does with the music by itself.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::read::*;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum MusicKind {
    /// A library playlist: `source` is its name.
    Playlist,
    /// An artist's radio station: `source` is the artist's name, found by `artist_id`.
    Artist,
    /// Any Apple Music link.
    Link,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct MusicEntry {
    pub id: String,
    pub name: String,
    pub kind: MusicKind,
    pub source: String,
    /// Artist radio only: Apple's id of the artist (its station is `ra.<id>`) and the store it was
    /// found in, e.g. `at`.
    pub artist_id: String,
    pub storefront: String,
    /// Playlists only.
    pub shuffle: bool,
}

impl MusicEntry {
    /// Entries need a name or a source; ids are unique.
    pub fn read_all(raw: &[Value]) -> Vec<Self> {
        let mut seen = HashSet::new();
        raw.iter()
            .filter_map(|item| Self::read(item, &mut seen))
            .collect()
    }

    fn read(raw: &Value, seen: &mut HashSet<String>) -> Option<Self> {
        let source = raw.as_object()?;
        let name = trimmed(source.get("name"));
        let target = trimmed(source.get("source"));
        if name.is_empty() && target.is_empty() {
            return None;
        }
        let kind = one_of(source.get("kind")).unwrap_or(MusicKind::Playlist);
        let artist = kind == MusicKind::Artist;
        let artist_id = Some(trimmed(source.get("artistId")))
            .filter(|id| artist && !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()));
        let storefront = Some(trimmed(source.get("storefront")).to_lowercase()).filter(|store| {
            artist && store.len() == 2 && store.bytes().all(|b| b.is_ascii_lowercase())
        });
        Some(Self {
            id: unique_id(source, &name, seen, "music"),
            name,
            kind,
            source: if kind == MusicKind::Link {
                music_link(&target)
            } else {
                target
            },
            artist_id: artist_id.unwrap_or_default(),
            storefront: storefront.unwrap_or_default(),
            shuffle: on_unless_false(source.get("shuffle")),
        })
    }
}

/// Apple Music web links open in the Music app (not the browser) only with `app=music`; it's stored
/// with the link so Settings shows what is opened.
fn music_link(link: &str) -> String {
    if !is_apple_music(link) {
        return link.into();
    }
    let Ok(mut url) = url::Url::parse(link) else {
        return link.into();
    };
    let kept: Vec<(String, String)> = url
        .query_pairs()
        .filter(|(key, _)| key != "app")
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.query_pairs_mut()
        .clear()
        .extend_pairs(kept)
        .append_pair("app", "music");
    url.into()
}

/// `https://music.apple.com/…` or a subdomain of it.
fn is_apple_music(link: &str) -> bool {
    let lower = link.to_lowercase();
    let Some(rest) = lower.strip_prefix("https://") else {
        return false;
    };
    let Some((host, _)) = rest.split_once('/') else {
        return false;
    };
    let Some(labels) = host
        .strip_suffix("music.apple.com")
        .filter(|prefix| prefix.is_empty() || prefix.ends_with('.'))
    else {
        return false;
    };
    labels.split_terminator('.').all(is_word)
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum OnCall {
    Nothing,
    Pause,
    /// Turn Music down to `call_volume`.
    Lower,
}

/// What Daybook does with the music by itself while a context is active.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ContextMusic {
    pub on_call: OnCall,
    /// Music's volume during a call, 0 to 100.
    pub call_volume: u8,
    /// Pause on Break and Done for today.
    pub pause_when_away: bool,
    /// After a call or back at work, play again what Daybook paused.
    pub resume: bool,
    /// Id of the entry started with the day's first Working; empty: none.
    pub start_with: String,
}

impl Default for ContextMusic {
    fn default() -> Self {
        Self {
            on_call: OnCall::Lower,
            call_volume: 2,
            pause_when_away: true,
            resume: true,
            start_with: String::new(),
        }
    }
}

impl ContextMusic {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        let defaults = Self::default();
        Self {
            on_call: one_of(field(source, "onCall")).unwrap_or(defaults.on_call),
            call_volume: within(coerced_number(field(source, "callVolume")), 0.0, 100.0)
                .map_or(defaults.call_volume, |volume| round(volume) as u8),
            pause_when_away: flag(field(source, "pauseWhenAway"))
                .unwrap_or(defaults.pause_when_away),
            resume: flag(field(source, "resume")).unwrap_or(defaults.resume),
            start_with: trimmed(field(source, "startWith")),
        }
    }
}
