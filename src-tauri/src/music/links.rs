//! The `music:` link that plays an artist radio or link entry: Music takes it directly and starts a
//! station, while the web link goes through the browser first.

use std::sync::LazyLock;

use regex::Regex;

use super::state::entry_label;
use crate::store::settings::{MusicEntry, MusicKind, music_link};

static ARTIST_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)^https://(?:[A-Za-z0-9_-]+\.)*music\.apple\.com/([a-z]{2})/artist/([^/?]+)/([0-9]+)",
    )
    .unwrap()
});
static MUSIC_LINK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^(https://([A-Za-z0-9_-]+\.)*music\.apple\.com/|music://|itmss?://)").unwrap()
});

fn slug(artist: &str) -> String {
    let lower = artist.to_lowercase();
    let mut slug = String::new();
    for c in lower.chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            slug.push(c);
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "artist".into()
    } else {
        slug.into()
    }
}

/// The "… & Similar Artists" station of an artist; the US store without one.
pub fn station_url(storefront: &str, artist_id: &str, artist: &str) -> String {
    let store = if storefront.is_empty() {
        "us"
    } else {
        storefront
    };
    format!(
        "music://music.apple.com/{store}/station/{}-similar-artists/ra.{artist_id}",
        slug(artist)
    )
}

/// What `open` plays for an artist radio or link entry; `Err` says what the entry lacks.
pub fn entry_url(entry: &MusicEntry) -> Result<String, String> {
    let label = entry_label(entry);
    if entry.kind == MusicKind::Artist {
        if entry.artist_id.is_empty() {
            return Err(format!(
                "Pick the artist for “{label}” in Settings → Music."
            ));
        }
        return Ok(station_url(
            &entry.storefront,
            &entry.artist_id,
            &entry.source,
        ));
    }
    if !MUSIC_LINK.is_match(&entry.source) {
        return Err(format!(
            "“{label}” needs an Apple Music link (in Music: Share → Copy Link)."
        ));
    }
    if let Some(artist) = ARTIST_LINK.captures(&entry.source) {
        return Ok(station_url(
            &artist[1].to_lowercase(),
            &artist[3],
            &artist[2],
        ));
    }
    Ok(with_music_scheme(music_link(&entry.source)))
}

/// `https:` becomes `music:`, in any case; other schemes stay.
fn with_music_scheme(link: String) -> String {
    match link.get(..6) {
        Some(scheme) if scheme.eq_ignore_ascii_case("https:") => format!("music:{}", &link[6..]),
        _ => link,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: MusicKind, name: &str, source: &str) -> MusicEntry {
        MusicEntry {
            id: "x".into(),
            name: name.into(),
            kind,
            source: source.into(),
            artist_id: String::new(),
            storefront: String::new(),
            shuffle: true,
        }
    }

    #[test]
    fn builds_an_artists_station_from_its_id() {
        assert_eq!(
            station_url("at", "1890089181", "Deep Idle Room"),
            "music://music.apple.com/at/station/deep-idle-room-similar-artists/ra.1890089181"
        );
        assert_eq!(
            station_url("", "5", "éé"),
            "music://music.apple.com/us/station/artist-similar-artists/ra.5"
        );
    }

    #[test]
    fn opens_artist_radio_by_the_picked_artist_and_refuses_one_not_picked_yet() {
        let picked = MusicEntry {
            artist_id: "27067520".into(),
            storefront: "at".into(),
            ..entry(MusicKind::Artist, "", "Cell")
        };
        assert_eq!(
            entry_url(&picked).unwrap(),
            "music://music.apple.com/at/station/cell-similar-artists/ra.27067520"
        );
        let err = entry_url(&entry(MusicKind::Artist, "Cell", "Cell")).unwrap_err();
        assert!(err.contains("Pick the artist"), "{err}");
    }

    #[test]
    fn turns_an_artist_page_into_its_station_and_other_links_into_music_links_with_app_music() {
        assert_eq!(
            entry_url(&entry(
                MusicKind::Link,
                "",
                "https://music.apple.com/at/artist/cell/27067520"
            ))
            .unwrap(),
            "music://music.apple.com/at/station/cell-similar-artists/ra.27067520"
        );
        assert_eq!(
            entry_url(&entry(
                MusicKind::Link,
                "",
                "https://music.apple.com/at/playlist/x/pl.123"
            ))
            .unwrap(),
            "music://music.apple.com/at/playlist/x/pl.123?app=music"
        );
        let err = entry_url(&entry(MusicKind::Link, "Site", "https://example.com/")).unwrap_err();
        assert!(err.contains("Apple Music link"), "{err}");
    }
}
