//! The only place that talks to the Music app: JXA through `osascript` (macOS asks once for
//! Automation). Reading never launches Music; playing does. Parsing the scripts' output is pure.

use std::path::Path;
use std::time::Duration;

use serde_json::Value;

use super::state::{Player, PlayerState};
use super::tracking::Playback;
use crate::calls::checks::automation_refused;
use crate::error::{AppError, AppResult};
use crate::process;

pub const READ_TIMEOUT: Duration = Duration::from_secs(8);
/// Playing may have to launch Music first.
pub const PLAY_TIMEOUT: Duration = Duration::from_secs(25);
const OPEN_TIMEOUT: Duration = Duration::from_secs(10);

pub const AUTOMATION_HINT: &str =
    "Allow Daybook to control Music in System Settings → Privacy & Security → Automation.";
pub const NO_ANSWER: &str = "Music didn’t answer. Try again in a moment.";

pub const READ_JXA: &str = r#"function run() {
  var music = Application("Music");
  if (!music.running()) return "{}";
  var track = null;
  var playlist = null;
  try { track = [music.currentTrack.name(), music.currentTrack.artist()].filter(Boolean).join(" – "); } catch (e) {}
  try { playlist = music.currentPlaylist.name(); } catch (e) {}
  var position = null;
  var duration = null;
  try { position = music.playerPosition(); duration = music.currentTrack.duration(); } catch (e) {}
  return JSON.stringify({ player: music.playerState(), volume: music.soundVolume(), track: track, playlist: playlist, position: position, duration: duration });
}"#;

/// Arguments: the playlist's name and `true`/`false` for shuffle.
pub const PLAY_PLAYLIST_JXA: &str = r#"function run(argv) {
  var music = Application("Music");
  var list = music.playlists.byName(argv[0]);
  try { list.name(); } catch (e) { return "missing"; }
  music.shuffleEnabled = argv[1] === "true";
  list.play();
  return "ok";
}"#;

/// `pause`, `play` (resume) or `volume <0-100>`; nothing happens while Music is closed.
pub const CONTROL_JXA: &str = r#"function run(argv) {
  var music = Application("Music");
  if (!music.running()) return "closed";
  if (argv[0] === "pause") music.pause();
  else if (argv[0] === "play") music.play();
  else music.soundVolume = Number(argv[1]);
  return "ok";
}"#;

pub const PLAYLISTS_JXA: &str = r#"function run() {
  return JSON.stringify(Application("Music").userPlaylists.name());
}"#;

/// What a failed script means for the owner.
pub fn failure(stderr: &str) -> AppError {
    AppError::Network(if automation_refused(stderr) {
        AUTOMATION_HINT.into()
    } else {
        NO_ANSWER.into()
    })
}

/// Runs `program`; its trimmed output. A failure is logged as `subject` failed.
fn run(program: &str, args: Vec<String>, timeout: Duration, subject: &str) -> AppResult<String> {
    match process::run(Path::new(program), &args, timeout) {
        Ok(output) if output.success => Ok(output.stdout.trim().to_string()),
        Ok(output) => {
            log::warn!("{subject} failed: {}", output.stderr.trim());
            Err(failure(&output.stderr))
        }
        Err(e) => {
            log::warn!("{subject} failed: {e}");
            Err(failure(""))
        }
    }
}

/// Runs a JXA script with `args`; its trimmed output. Blocking.
pub fn tell(script: &str, args: &[&str], timeout: Duration) -> AppResult<String> {
    let mut all = vec![
        "-l".to_string(),
        "JavaScript".into(),
        "-e".into(),
        script.into(),
    ];
    all.extend(args.iter().map(|arg| arg.to_string()));
    run("/usr/bin/osascript", all, timeout, "Talking to Music")
}

/// Opens a `music:` link in the background, so the app in front stays in front. Blocking.
pub fn open_in_background(url: &str) -> AppResult<()> {
    let args = vec!["-g".to_string(), url.to_string()];
    run(
        "/usr/bin/open",
        args,
        OPEN_TIMEOUT,
        &format!("Opening {url}"),
    )
    .map(|_| ())
}

/// One read of Music.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub player: Player,
    pub playback: Playback,
}

fn seconds(value: Option<&Value>) -> f64 {
    value
        .and_then(Value::as_f64)
        .filter(|s| s.is_finite())
        .unwrap_or(0.0)
}

/// Music closed when the read names no player state.
fn player_of(source: &Value) -> Player {
    let text = |key: &str| source.get(key).and_then(Value::as_str);
    let Some(state) = text("player") else {
        return Player::CLOSED;
    };
    Player {
        state: match state {
            "paused" => PlayerState::Paused,
            "stopped" => PlayerState::Stopped,
            // Also fast forwarding and rewinding.
            _ => PlayerState::Playing,
        },
        track: text("track")
            .filter(|track| !track.is_empty())
            .map(str::to_string),
        volume: source
            .get("volume")
            .and_then(Value::as_f64)
            .map(|volume| volume.round().clamp(0.0, 100.0) as u8),
        playlist: text("playlist").map(str::to_string),
    }
}

fn playback_of(source: &Value) -> Playback {
    Playback {
        position: seconds(source.get("position")),
        duration: seconds(source.get("duration")),
    }
}

/// `READ_JXA`'s output; empty output reads as Music closed, anything but JSON fails.
pub fn parse_reading(output: &str) -> Option<Reading> {
    let text = if output.is_empty() { "{}" } else { output };
    let source: Value = serde_json::from_str(text).ok()?;
    Some(Reading {
        player: player_of(&source),
        playback: playback_of(&source),
    })
}

/// `PLAYLISTS_JXA`'s output: the names, without blanks and duplicates, sorted; anything but JSON fails.
pub fn parse_playlists(output: &str) -> Option<Vec<String>> {
    let text = if output.is_empty() { "[]" } else { output };
    let parsed: Value = serde_json::from_str(text).ok()?;
    let mut names: Vec<String> = parsed
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .filter(|name| !name.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then(a.cmp(b)));
    names.dedup();
    Some(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    fn player(state: PlayerState, track: Option<&str>, volume: Option<u8>) -> Player {
        Player {
            state,
            track: track.map(str::to_string),
            volume,
            playlist: None,
        }
    }

    #[test]
    fn empty_or_bare_output_reads_as_music_closed() {
        for output in ["", "{}"] {
            assert_eq!(
                parse_reading(output),
                Some(Reading {
                    player: Player::CLOSED,
                    playback: Playback {
                        position: 0.0,
                        duration: 0.0
                    },
                }),
                "{output:?}"
            );
        }
    }

    #[test]
    fn a_full_reading_carries_state_volume_track_playlist_and_playback() {
        let output = r#"{"player":"paused","volume":49.6,"track":"Tideland – LUCEROSSA","playlist":"Deep Focus","position":12.5,"duration":200}"#;

        assert_eq!(
            parse_reading(output),
            Some(Reading {
                player: Player {
                    state: PlayerState::Paused,
                    track: Some("Tideland – LUCEROSSA".into()),
                    volume: Some(50),
                    playlist: Some("Deep Focus".into()),
                },
                playback: Playback {
                    position: 12.5,
                    duration: 200.0
                },
            })
        );
    }

    #[test]
    fn player_states_map_and_unknown_ones_count_as_playing() {
        for (state, expected) in [
            ("playing", PlayerState::Playing),
            ("paused", PlayerState::Paused),
            ("stopped", PlayerState::Stopped),
            ("fast forwarding", PlayerState::Playing),
            ("rewinding", PlayerState::Playing),
        ] {
            let output = format!(r#"{{"player":"{state}"}}"#);
            assert_eq!(
                parse_reading(&output).unwrap().player,
                player(expected, None, None),
                "{state}"
            );
        }
    }

    #[test]
    fn an_empty_track_is_none_but_an_empty_playlist_stays() {
        let reading = parse_reading(r#"{"player":"playing","track":"","playlist":""}"#).unwrap();

        assert_eq!(reading.player.track, None);
        assert_eq!(reading.player.playlist, Some(String::new()));
    }

    #[test]
    fn a_missing_or_non_numeric_volume_is_none() {
        for output in [
            r#"{"player":"playing"}"#,
            r#"{"player":"playing","volume":"loud"}"#,
            r#"{"player":"playing","volume":null}"#,
        ] {
            assert_eq!(
                parse_reading(output).unwrap().player.volume,
                None,
                "{output}"
            );
        }
    }

    #[test]
    fn null_position_and_duration_read_as_zero() {
        let reading =
            parse_reading(r#"{"player":"playing","position":null,"duration":null}"#).unwrap();

        assert_eq!(
            reading.playback,
            Playback {
                position: 0.0,
                duration: 0.0
            }
        );
    }

    #[test]
    fn output_that_is_not_json_is_not_a_reading() {
        assert_eq!(parse_reading("Music got an error"), None);
    }

    #[test]
    fn playlist_names_are_sorted_without_blanks_and_duplicates() {
        assert_eq!(
            parse_playlists(r#"["b","A","",1,"A","a"]"#),
            Some(vec!["A".to_string(), "a".into(), "b".into()])
        );
    }

    #[test]
    fn empty_or_non_array_playlist_output_is_an_empty_list() {
        for output in ["", "{}"] {
            assert_eq!(parse_playlists(output), Some(Vec::new()), "{output:?}");
        }
    }

    #[test]
    fn playlist_output_that_is_not_json_fails() {
        assert_eq!(parse_playlists("nope"), None);
    }

    #[test]
    fn a_refused_automation_request_gets_the_automation_hint() {
        for stderr in [
            "execution error: Not authorized to send Apple events to Music. (-1743)",
            "Not authorised to send Apple events",
        ] {
            let err = failure(stderr);
            assert_eq!(err.kind(), ErrorKind::Network, "{stderr}");
            assert!(
                matches!(err, AppError::Network(ref m) if m == AUTOMATION_HINT),
                "{stderr}"
            );
        }
    }

    #[test]
    fn any_other_failure_says_music_did_not_answer() {
        for stderr in ["", "execution error: Can’t get playlist. (-1728)"] {
            let err = failure(stderr);
            assert_eq!(err.kind(), ErrorKind::Network, "{stderr}");
            assert!(
                matches!(err, AppError::Network(ref m) if m == NO_ANSWER),
                "{stderr}"
            );
        }
    }
}
