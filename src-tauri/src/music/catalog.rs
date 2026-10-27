//! Apple's artist search (iTunes Search API), for artist radio entries; the store to search in.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use ts_rs::TS;
use url::Url;

use crate::error::{AppError, AppResult};
use crate::net::{Http, NetError};
use crate::state::AppState;
use crate::store::settings::MusicEntry;

pub const SEARCH_URL: &str = "https://itunes.apple.com/search";
const MATCHES: &str = "8";
const FALLBACK_STORE: &str = "us";

static STORE_IN_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"music\.apple\.com/([A-Za-z]{2})/").unwrap());

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ArtistMatch {
    pub id: String,
    pub name: String,
    /// Empty when Apple names none.
    pub genre: String,
}

/// Artists by name, best match first, and the store they were found in.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct ArtistSearch {
    pub storefront: String,
    pub artists: Vec<ArtistMatch>,
}

fn is_store(code: &str) -> bool {
    code.len() == 2 && code.bytes().all(|b| b.is_ascii_alphabetic())
}

pub fn search_url(base: &Url, term: &str, storefront: &str) -> Url {
    let mut url = base.clone();
    url.query_pairs_mut()
        .append_pair("term", term)
        .append_pair("entity", "musicArtist")
        .append_pair("limit", MATCHES)
        .append_pair("country", storefront);
    url
}

/// The search's JSON: artists with a numeric id and a name, in Apple's order.
pub fn parse_artists(body: &[u8]) -> Vec<ArtistMatch> {
    let parsed: Value = serde_json::from_slice(body).unwrap_or_default();
    let Some(results) = parsed.get("results").and_then(Value::as_array) else {
        return Vec::new();
    };
    results
        .iter()
        .filter_map(|item| {
            let id = item.get("artistId").filter(|id| id.is_number())?;
            let name = item.get("artistName").and_then(Value::as_str)?;
            Some(ArtistMatch {
                id: id.to_string(),
                name: name.to_string(),
                genre: item
                    .get("primaryGenreName")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            })
        })
        .collect()
}

/// The store of the first entry that names one, by its storefront or its Apple Music link.
pub fn storefront_of(entries: &[MusicEntry]) -> Option<String> {
    entries.iter().find_map(|entry| {
        if entry.storefront.is_empty() {
            STORE_IN_LINK
                .captures(&entry.source)
                .map(|found| found[1].to_lowercase())
        } else {
            Some(entry.storefront.to_lowercase())
        }
    })
}

/// The region of a macOS locale: `de_AT` → `at`; a region set apart from the language wins
/// (`en_US@rg=atzzzz` → `at`).
pub fn region_of(locale: &str) -> Option<String> {
    let (base, extra) = locale.trim().split_once('@').unwrap_or((locale.trim(), ""));
    let from_extra = extra
        .split(';')
        .find_map(|part| part.strip_prefix("rg="))
        .and_then(|region| region.get(..2));
    let from_base = base.split(['_', '-']).nth(1);
    from_extra
        .or(from_base)
        .filter(|code| is_store(code))
        .map(str::to_lowercase)
}

/// The Mac's region, from `AppleLocale`. Blocking the first time.
pub fn mac_region() -> Option<String> {
    crate::locale::apple_locale().and_then(region_of)
}

/// The store to search in: the one asked for, else one the entries name, else `region`, else the US.
pub fn storefront(asked: Option<&str>, entries: &[MusicEntry], region: Option<String>) -> String {
    asked
        .map(str::trim)
        .filter(|code| is_store(code))
        .map(str::to_lowercase)
        .or_else(|| storefront_of(entries))
        .or(region)
        .unwrap_or_else(|| FALLBACK_STORE.into())
}

pub async fn find_artists(
    http: &Http,
    base: &Url,
    term: &str,
    storefront: &str,
) -> AppResult<Vec<ArtistMatch>> {
    let body = http
        .get(&search_url(base, term, storefront))
        .await
        .map_err(|e| match e {
            NetError::Status(status) => {
                AppError::Network(format!("Apple’s artist search didn’t answer ({status})."))
            }
            other => AppError::Network(format!("Apple’s artist search failed: {other}")),
        })?;
    Ok(parse_artists(&body))
}

/// Artists named like `term` in the store `storefront` resolves to, with the saved entries and the Mac's
/// region as fallbacks.
pub async fn search_artists(
    app: &AppHandle,
    term: &str,
    storefront: Option<&str>,
) -> AppResult<ArtistSearch> {
    let entries = app.state::<AppState>().settings.get().music;
    let region = super::blocking(|| Ok(mac_region())).await.ok().flatten();
    let storefront = self::storefront(storefront, &entries, region);
    let base = Url::parse(SEARCH_URL).expect("a valid search URL");
    let artists = find_artists(&super::music(app).http, &base, term, &storefront).await?;
    Ok(ArtistSearch {
        storefront,
        artists,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::MusicKind;

    fn entry(source: &str, storefront: &str) -> MusicEntry {
        MusicEntry {
            id: "e".into(),
            name: "Entry".into(),
            kind: MusicKind::Link,
            source: source.into(),
            artist_id: String::new(),
            storefront: storefront.into(),
            shuffle: false,
        }
    }

    #[test]
    fn the_search_url_asks_for_artists_in_the_storefront() {
        let base = Url::parse(SEARCH_URL).unwrap();

        assert_eq!(
            search_url(&base, "Cell", "at").as_str(),
            "https://itunes.apple.com/search?term=Cell&entity=musicArtist&limit=8&country=at"
        );
    }

    #[test]
    fn artists_need_a_numeric_id_and_a_name() {
        let body = br#"{"results":[{"artistId":27067520,"artistName":"Cell","primaryGenreName":"Electronic"},{"artistId":"x","artistName":"Bad"},{"artistId":5,"artistName":"NoGenre"}]}"#;

        assert_eq!(
            parse_artists(body),
            vec![
                ArtistMatch {
                    id: "27067520".into(),
                    name: "Cell".into(),
                    genre: "Electronic".into()
                },
                ArtistMatch {
                    id: "5".into(),
                    name: "NoGenre".into(),
                    genre: String::new()
                },
            ]
        );
    }

    #[test]
    fn a_body_without_results_has_no_artists() {
        for body in [&b"{}"[..], b"", b"not json"] {
            assert_eq!(parse_artists(body), Vec::new(), "{body:?}");
        }
    }

    #[test]
    fn the_region_comes_from_the_locale() {
        for (locale, expected) in [
            ("de_AT", Some("at")),
            ("en_US@rg=atzzzz", Some("at")),
            ("en", None),
        ] {
            assert_eq!(region_of(locale), expected.map(String::from), "{locale}");
        }
    }

    #[test]
    fn an_entrys_storefront_wins_over_the_one_in_its_link() {
        let entries = [entry("https://music.apple.com/de/album/x", "AT")];

        assert_eq!(storefront_of(&entries), Some("at".into()));
    }

    #[test]
    fn the_storefront_falls_back_to_the_link_lowercased() {
        let entries = [
            entry("Deep Focus", ""),
            entry("https://music.apple.com/DE/album/x", ""),
        ];

        assert_eq!(storefront_of(&entries), Some("de".into()));
    }

    #[test]
    fn entries_without_a_store_name_none() {
        assert_eq!(storefront_of(&[entry("Deep Focus", "")]), None);
    }

    #[test]
    fn the_store_to_search_is_asked_then_entries_then_region_then_us() {
        let entries = [entry("x", "de")];
        let region = || Some("fr".to_string());

        assert_eq!(storefront(Some(" AT "), &entries, region()), "at");
        assert_eq!(storefront(Some("austria"), &entries, region()), "de");
        assert_eq!(storefront(None, &entries, region()), "de");
        assert_eq!(storefront(None, &[], region()), "fr");
        assert_eq!(storefront(None, &[], None), "us");
    }
}
