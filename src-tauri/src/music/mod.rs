//! Apple Music (docs/music.md): the entries of Settings → Music played through Music (`script`), which
//! entry plays (`tracking`), the `music` overlay's and Settings' commands (`commands`), and what calls and
//! breaks do to the music (`automation`). Music is read every `POLL` once entries exist, not while
//! away, and right after every action and again `SETTLE` later.

mod automation;
pub mod catalog;
pub mod commands;
pub mod links;
pub mod rules;
pub mod script;
pub mod state;
pub mod tracking;

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use chrono::Utc;
use tauri::{AppHandle, Emitter, Manager};

pub use automation::restore_on_exit;
pub use commands::register;

use self::script::{
    CONTROL_JXA, NO_ANSWER, PLAY_PLAYLIST_JXA, PLAY_TIMEOUT, PLAYLISTS_JXA, READ_JXA, READ_TIMEOUT,
};
use self::state::{MusicState, Player, PlayerState, entry_label, music_state as describe};
use self::tracking::{Tracking, follow_player, started_entry};
use crate::error::{AppError, AppResult};
use crate::events::MUSIC_CHANGED_EVENT;
use crate::lock::MutexExt;
use crate::net::{self, Http};
use crate::state::AppState;
use crate::store::settings::{MusicEntry, MusicKind};
use crate::{presence, tools, tray};

const POLL: Duration = Duration::from_secs(30);
/// A station opened by link takes a moment to start.
const SETTLE: Duration = Duration::from_millis(2_500);

#[derive(Default)]
struct Live {
    player: Option<Player>,
    tracking: Tracking,
    last_error: Option<String>,
}

pub struct Music {
    live: Mutex<Live>,
    /// The latest settle; an earlier one's late read is skipped.
    settle: AtomicU64,
    automation: automation::Automation,
    http: Http,
}

impl Default for Music {
    fn default() -> Self {
        Self {
            live: Mutex::default(),
            settle: AtomicU64::new(0),
            automation: automation::Automation::default(),
            http: Http::new(net::HOSTS),
        }
    }
}

fn music(app: &AppHandle) -> &Music {
    app.state::<Music>().inner()
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

/// Runs blocking work that talks to the Mac off the async runtime.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|_| AppError::Network(NO_ANSWER.into()))?
}

async fn tell(script: &'static str, args: Vec<String>, timeout: Duration) -> AppResult<String> {
    blocking(move || {
        let args: Vec<&str> = args.iter().map(String::as_str).collect();
        script::tell(script, &args, timeout)
    })
    .await
}

/// What the overlay, the menu and the rail show, from the last read.
pub fn current(app: &AppHandle) -> MusicState {
    let settings = app.state::<AppState>().settings.get();
    let live = music(app).live.lock_unpoisoned();
    let player = live.player.clone().unwrap_or(Player::CLOSED);
    describe(&settings.music, &player, &live.tracking)
}

/// The Music tool's part of `ToolState`.
pub fn tool_state(app: &AppHandle) -> tools::Music {
    let state = current(app);
    tools::Music {
        entries: state
            .entries
            .into_iter()
            .map(|item| tools::MusicItem {
                id: item.id,
                name: item.name,
            })
            .collect(),
        playing: state.player == PlayerState::Playing,
        volume: state.volume,
        entry: state.entry,
    }
}

fn announce(app: &AppHandle) {
    if let Err(e) = app.emit(MUSIC_CHANGED_EVENT, current(app)) {
        log::warn!("Couldn't announce the music change: {e}");
    }
    tray::refresh(app);
}

/// Asks Music where it stands; the pages follow through `music_changed`.
pub async fn read(app: &AppHandle) -> AppResult<Player> {
    let output = tell(READ_JXA, Vec::new(), READ_TIMEOUT).await?;
    let reading = script::parse_reading(&output).ok_or_else(|| script::failure(""))?;
    let changed = {
        let mut live = music(app).live.lock_unpoisoned();
        let before = live.tracking.unbroken;
        live.tracking = follow_player(&live.tracking, &reading.player, reading.playback, now_ms());
        let changed =
            live.player.as_ref() != Some(&reading.player) || before != live.tracking.unbroken;
        live.player = Some(reading.player.clone());
        changed
    };
    if changed {
        announce(app);
    }
    Ok(reading.player)
}

/// The regular look at Music; only once there is something to play. A failure is logged once.
pub async fn refresh(app: &AppHandle) {
    if app.state::<AppState>().settings.get().music.is_empty() {
        return;
    }
    let error = read(app).await.err().map(|e| e.to_string());
    let mut live = music(app).live.lock_unpoisoned();
    if let Some(message) = &error
        && live.last_error.as_ref() != Some(message)
    {
        log::warn!("Reading Music failed: {message}");
    }
    live.last_error = error;
}

/// Reads now and once more when the change has settled.
async fn settle(app: &AppHandle) -> AppResult<()> {
    let generation = music(app).settle.fetch_add(1, Ordering::SeqCst) + 1;
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SETTLE).await;
        if music(&handle).settle.load(Ordering::SeqCst) == generation {
            refresh(&handle).await;
        }
    });
    read(app).await.map(|_| ())
}

pub async fn play_entry(app: &AppHandle, id: &str) -> AppResult<()> {
    let settings = app.state::<AppState>().settings.get();
    let entry = settings
        .music
        .iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| AppError::NotFound("This entry is no longer in Settings → Music.".into()))?;
    open_entry(entry).await?;
    {
        let mut live = music(app).live.lock_unpoisoned();
        live.tracking = started_entry(&live.tracking, &entry.id, now_ms());
    }
    log::info!("Music plays {}", entry_label(entry));
    settle(app).await
}

/// A playlist through Music; a station or link through its `music:` link.
async fn open_entry(entry: &MusicEntry) -> AppResult<()> {
    if entry.kind != MusicKind::Playlist {
        let url = links::entry_url(entry).map_err(AppError::InvalidInput)?;
        return blocking(move || script::open_in_background(&url)).await;
    }
    let args = vec![entry.source.clone(), entry.shuffle.to_string()];
    if tell(PLAY_PLAYLIST_JXA, args, PLAY_TIMEOUT).await? == "missing" {
        return Err(AppError::NotFound(format!(
            "Music has no playlist named “{}”.",
            entry.source
        )));
    }
    Ok(())
}

/// Tells Music to pause or play on; nothing happens while it is closed.
async fn control(app: &AppHandle, command: &str) -> AppResult<()> {
    tell(CONTROL_JXA, vec![command.to_string()], READ_TIMEOUT).await?;
    settle(app).await
}

pub async fn pause(app: &AppHandle) -> AppResult<()> {
    control(app, "pause").await
}

pub async fn resume(app: &AppHandle) -> AppResult<()> {
    control(app, "play").await
}

/// Music's own volume, 0 to 100.
pub async fn set_volume(app: &AppHandle, volume: f64) -> AppResult<()> {
    let level = volume.round().clamp(0.0, 100.0) as u8;
    let args = vec!["volume".to_string(), level.to_string()];
    tell(CONTROL_JXA, args, READ_TIMEOUT).await?;
    read(app).await.map(|_| ())
}

/// Continues; with nothing to continue, the entry started last (or the first one) plays.
async fn start(app: &AppHandle, now: &Player) -> AppResult<()> {
    if now.state == PlayerState::Paused {
        return resume(app).await;
    }
    let settings = app.state::<AppState>().settings.get();
    let started = music(app).live.lock_unpoisoned().tracking.started.clone();
    let entry = settings
        .music
        .iter()
        .find(|entry| Some(&entry.id) == started.as_ref())
        .or(settings.music.first())
        .ok_or_else(|| AppError::NotFound("Add a playlist in Settings → Music first.".into()))?;
    play_entry(app, &entry.id).await
}

/// Music that plays already keeps playing.
pub async fn play(app: &AppHandle) -> AppResult<()> {
    let now = read(app).await?;
    if now.playing() {
        return Ok(());
    }
    start(app, &now).await
}

/// Pauses only what plays: telling Music to pause would start it when it isn't running.
pub async fn pause_if_playing(app: &AppHandle) -> AppResult<()> {
    if read(app).await?.playing() {
        pause(app).await?;
    }
    Ok(())
}

/// Pauses, or plays; with Music closed it launches it.
pub async fn toggle(app: &AppHandle) -> AppResult<()> {
    let now = read(app).await?;
    if now.playing() {
        pause(app).await
    } else {
        start(app, &now).await
    }
}

/// The library's playlist names, for Settings. Launches Music.
pub async fn playlists() -> AppResult<Vec<String>> {
    let output = tell(PLAYLISTS_JXA, Vec::new(), PLAY_TIMEOUT).await?;
    script::parse_playlists(&output).ok_or_else(|| script::failure(""))
}

/// Reads Music every `POLL` and follows calls and the status.
pub fn install(app: &AppHandle) {
    automation::install(app);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticks = tokio::time::interval(POLL);
        loop {
            ticks.tick().await;
            if !presence::is_away(&app) {
                refresh(&app).await;
            }
        }
    });
}
