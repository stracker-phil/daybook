//! The `music` overlay's and Settings' commands (each action answers with the new state) and the
//! `music:` commands of the bus.

use serde_json::Value;
use tauri::AppHandle;

use super::catalog::{self, ArtistSearch};
use super::state::MusicState;
use crate::bus::{ArgChoice, Command, CommandBus};
use crate::error::{AppError, AppResult};
use crate::window;

const VIEW: &str = "music";

fn required(text: &str, what: &str) -> AppResult<String> {
    let text = text.trim();
    if text.is_empty() {
        return Err(AppError::InvalidInput(format!("{what} is empty")));
    }
    Ok(text.to_string())
}

/// Reads Music, then answers with what it does.
#[tauri::command]
pub async fn music_state(app: AppHandle) -> AppResult<MusicState> {
    super::read(&app).await?;
    Ok(super::current(&app))
}

#[tauri::command]
pub async fn music_play(app: AppHandle, id: String) -> AppResult<MusicState> {
    super::play_entry(&app, &required(&id, "Entry")?).await?;
    Ok(super::current(&app))
}

#[tauri::command]
pub async fn music_toggle(app: AppHandle) -> AppResult<MusicState> {
    super::toggle(&app).await?;
    Ok(super::current(&app))
}

/// `volume` is clamped to 0 to 100 and rounded.
#[tauri::command]
pub async fn music_volume(app: AppHandle, volume: f64) -> AppResult<MusicState> {
    if !volume.is_finite() {
        return Err(AppError::InvalidInput("Volume must be a number".into()));
    }
    super::set_volume(&app, volume).await?;
    Ok(super::current(&app))
}

/// The library's playlists, sorted. Launches Music.
#[tauri::command]
pub async fn music_playlists() -> AppResult<Vec<String>> {
    super::playlists().await
}

/// Artists named like `term`; `storefront` is a two-letter store, else the one the saved entries name,
/// else the Mac's region, else the US store.
#[tauri::command]
pub async fn music_find_artists(
    app: AppHandle,
    term: String,
    storefront: Option<String>,
) -> AppResult<ArtistSearch> {
    catalog::search_artists(&app, &required(&term, "Artist")?, storefront.as_deref()).await
}

/// A whole number from 0 to 100.
pub fn volume_argument(words: &[String]) -> Result<u8, String> {
    words
        .concat()
        .parse::<u8>()
        .ok()
        .filter(|level| *level <= 100)
        .ok_or_else(|| "The volume is a whole number from 0 to 100.".into())
}

fn entry_choices(app: &AppHandle) -> Vec<ArgChoice> {
    super::current(app)
        .entries
        .into_iter()
        .map(|entry| ArgChoice {
            word: entry.id.clone(),
            title: Some(entry.name),
            argument: entry.id,
        })
        .collect()
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(Command::new("music:open", "Music…").run(|app| async move {
        window::panel::open(&app, VIEW, Value::Null).await;
        Ok(())
    }));
    bus.add(
        Command::new("music:toggle", "Play or pause Music")
            .run(|app| async move { super::toggle(&app).await }),
    );
    bus.add(
        Command::new(
            "music:play",
            "Play: continue, or start the entry played last",
        )
        .run(|app| async move { super::play(&app).await }),
    );
    bus.add(
        Command::new("music:pause", "Pause Music")
            .run(|app| async move { super::pause_if_playing(&app).await }),
    );
    bus.add_family(
        Command::new("music:play:", "Play an entry from Settings → Music")
            .family("<entry>")
            .choices(entry_choices)
            .run(|app, id| async move { super::play_entry(&app, &id).await }),
    );
    bus.add_family(
        Command::new("music:volume:", "Set Music’s volume")
            .family("<0-100>")
            .argument_from_words(|words| volume_argument(words).map(|level| level.to_string()))
            .run(|app, level| async move {
                let level = volume_argument(&[level]).map_err(AppError::InvalidInput)?;
                super::set_volume(&app, f64::from(level)).await
            }),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_whole_number_from_0_to_100_is_a_volume() {
        for (input, expected) in [("0", 0u8), ("50", 50), ("100", 100)] {
            assert_eq!(volume_argument(&words(&[input])), Ok(expected), "{input}");
        }
    }

    #[test]
    fn anything_else_is_refused_with_the_allowed_range() {
        for input in [&["101"][..], &["x"], &["-1"], &["5.5"], &[]] {
            assert_eq!(
                volume_argument(&words(input)),
                Err("The volume is a whole number from 0 to 100.".to_string()),
                "{input:?}"
            );
        }
    }
}
