# Music

Daybook plays, pauses and turns Apple Music up or down, and reacts to the status and to calls per context: pause
on Break and Done for today, pause or lower the volume during a call, resume afterwards, start an entry with the
day's first Working. It talks to Music.app through JavaScript for Automation; the first call asks for the
Automation permission (System Settings → Privacy & Security → Automation).

## Entries

Settings → Music holds `settings.music`: entries with a `name`, a `kind` and a `source`.
- `playlist`: a library playlist by name, played with `shuffle` on or off.
- `artist`: an artist's station (`artistId` and `storefront`, found with the artist search).
- `link`: an Apple Music link (a playlist, album or artist page); `music_link` adds `app=music` so it opens in
  Music, not the browser.

`music/links.rs` (pure) turns an entry into what Music opens: `station_url(storefront, artist id, name)` gives
`music://music.apple.com/<cc>/station/<slug>-similar-artists/ra.<id>`, `entry_url` reads artist and link entries
(an artist page link becomes its station). Stations and links open with `open -g` (in the background).

## Talking to Music

`music/script.rs` holds the four scripts (read the state, play a playlist, control, list the playlists), run as
`/usr/bin/osascript -l JavaScript` through `process::run`: 8 s to read, 25 s to play (it may launch Music).
`parse_reading` and `parse_playlists` (pure) read their output; `failure` turns an error into "Allow Daybook to
control Music in System Settings → Privacy & Security → Automation." (Automation refused) or "Music didn’t
answer. Try again in a moment." (kind `network`).

`music::Music` (managed state) keeps the last reading (`state.rs`: `PlayerState` playing, paused, stopped or
closed; the track, the volume) and which entry plays. `tracking.rs` (pure) follows the player: an entry Daybook
started counts as playing until Music shows another playlist or stops for good. Music is read every 30 s once
entries exist (not while away), right after each action and again 2.5 s later; a change emits `music_changed`
(`MusicState`) and refreshes the tray.

## Automation

`music/rules.rs` (pure) decides per context (`ContextMusic`, on the context card in Settings → Contexts):
- During a call (`onCall`): keep playing, pause, or lower to `callVolume` %; afterwards resume or restore the
  volume (only when nobody changed it meanwhile, within 1 %).
- On Break and Done for today (`pauseWhenAway`): pause; back to Working (`resume`): play again what Daybook paused.
- The day's first Working (`startWith`): start that entry unless Music already plays.
Pausing on Break and Off and starting with the workday act only while the context has the Status feature
(`rules::effective`); the call rule acts in every context. `automation.rs` follows `status_changed`
([status.md](status.md)) and the call signals (`calls::follow`, [presence-and-calls.md](presence-and-calls.md)) and runs the steps
one after another. Daybook only pauses music it found playing and resumes only what it paused. A call moving to
another app (`CallEvent::Switched`) changes nothing: the music stays lowered or paused.

The lowered volume (`Lowered`: from, to) is kept in `music-held.json` (`music::HELD_FILE`, `AppState::music_held`) until it is back.
Quitting during a call restores it (`music::restore_on_exit`, within the 3 s `app.rs` gives Slack and music
together); after a restart or a crash, once the first call check found no call, the volume goes back too
(unless it was changed by hand). Music paused for a call stays paused after a restart.

## The artist search

`music/catalog.rs` searches Apple's public catalog (`https://itunes.apple.com/search?term=…&entity=musicArtist`)
through `net::Http::get`, the only GET on the network allow-list (ADR 0020). The store is the one asked for, else
the one of the saved entries, else the Mac's region (`defaults read -g AppleLocale`), else `us`.

## Commands

| Command | Args | Returns |
|---|---|---|
| `music_state` | – | `MusicState { entries, player, track, volume, entry }` (reads Music) |
| `music_play` | `id` | `MusicState`; plays the entry |
| `music_toggle` | – | `MusicState`; pauses, resumes, or (Music closed) plays the last entry, else the first |
| `music_volume` | `volume` | `MusicState`; 0 to 100 |
| `music_playlists` | – | the library's playlist names |
| `music_find_artists` | `term`, `storefront` | `ArtistSearch { storefront, artists: { id, name, genre }[] }` |

Bus commands: `music:open` (the `music` overlay), `music:toggle`, `music:play` (continue, or start the last entry,
else the first), `music:pause` (only while playing), the families `music:play:<entry>` and `music:volume:<0-100>`.
The tray's Music submenu and the palette come from the tool choices ([commands.md](commands.md)).

## The overlay

`MusicView` (`music` view, ⌃⌘M by default): what plays and the volume on top, then the entries with ⌘1–9 (the
current one marked). ↵ or ⌘n plays an entry and closes the overlay, ⌘P pauses or resumes, ⌘↑ and ⌘↓ change the
volume by 10.
