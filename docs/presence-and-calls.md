# Presence and calls

Daybook knows whether the owner is at the Mac and whether a call is running, and joins a meeting in the
right app. Presence gates what may open by itself; calls feed the sidebar, the menu bar, the rail, Slack, music
automation, the wizard and TimeTracker's call suggestion. Daybook does not log calls.

| File | Holds |
|---|---|
| `presence/checks.rs` | pure: `Away { asleep, locked }`, `after_wake`, `is_coming_back`, `has_been_asleep_long_enough`, `is_screen_locked_output` |
| `presence/native.rs` | sleep, wake, lock and unlock notifications; the lock state from `ioreg` |
| `presence/mod.rs` | `Presence` (managed state), `is_away`, `verify`, what coming back does |
| `calls/checks.rs` | pure: call titles, `microphone_pids`, `runs_from_bundle`, `lsappinfo` parsing, the built-in checks' texts |
| `calls/detect.rs` | `detect(rules, probe)` over the `Probe` trait; `Missing` permissions |
| `calls/tracker.rs` | pure: `Tracker::observe`, the call over a series of checks (`ActiveCall`, `CallEvent`) |
| `calls/probe.rs` | `MacProbe`: the detectors' questions to the Mac |
| `calls/access.rs` | Accessibility: trusted, the prompt, the privacy panes |
| `calls/join.rs` | joining a call: `open_args`, `join` |
| `calls/pick.rs` | `app_ref`: an app chosen in Settings, its name and bundle id (`plutil`) |
| `calls/mod.rs` | `Calls` (managed state), the 15 s check, the commands, `meetings:check` |

## Presence

Away means asleep or locked. Nothing may open by itself while away: a window shown at the lock screen stays
on screen after unlocking but never gets keyboard focus. Features that open an overlay without being asked
(routine and journal prompts, check-ins) check `presence::is_away` first.

macOS's events go missing at times (the lock at sleep is usually never announced), so:

- `NSWorkspaceWillSleepNotification` sets asleep; `NSWorkspaceDidWakeNotification` reads the lock state from
  `ioreg -n Root -d1 -r` (`"CGSSessionScreenIsLocked"=Yes`) and decides with `after_wake`. An unreadable state
  counts as locked. Waking is never coming back by itself.
- The distributed `com.apple.screenIsLocked` / `screenIsUnlocked` set and clear locked; an unlock while asleep
  is left to the wake.
- While locked, `verify` reads the lock state every 2 s until it is unlocked.
- While away, the 15 s call tick runs `verify` instead of a call check: asleep for at least a minute of wall
  time means awake again even without a wake event (timers stop during sleep).

Checks run one at a time. Coming back (away → not away) runs a call check, refreshes the menu bar and, 500 ms
later, gives an open overlay the focus again (`window::panel::refocus`).

## Calls

`calls::start` checks every 15 seconds while the owner is at the Mac, with a vault set and Settings → Calls →
Watch apps on (`detecting`). A regular check skips while one runs.

**Detectors.** The app triggers (`settings.meetings.rules`) are tried in order; the first enabled one in a
call wins:

| Trigger | In a call while | Permission |
|---|---|---|
| Zoom (`us.zoom.xos`) | Zoom runs and `pgrep -x CptHost` finds its meeting process | none |
| Teams (`com.microsoft.teams2`) | a process inside the Teams bundle records from a microphone (`pmset -g assertions`: `audio-in` assertions and their `Created for PID`, mapped with `ps -o comm=` to the bundle path from `lsappinfo info`), or Teams has a level-19 window on any Space (the window server's list) | none |
| Meet (the Google Meet Chrome app) | Chrome runs and a tab shows `meet.google.com/xxx-xxxx-xxx` (JXA through `osascript`); titled by the tab ("Meet – Weekly" → "Weekly") | Automation for Chrome |
| any other app | it runs (`lsappinfo list`); with a window title set, while one of its windows' titles contains it (Accessibility, `AXWindows`/`AXTitle`); titled by the window, without the app's name | Accessibility |

A trigger whose app doesn't name the call is titled "Meeting"; a calendar event running now replaces that
title (`call_title`). A refused permission leaves that check without an answer and Settings shows it; any
other failure skips the whole check and shows as the error in Settings.

**The grace rule.** `Tracker::observe` turns checks into `CallEvent`s: `Started`, `Changed` (the same app's call
with a new title; it keeps its start), `Ended { call, end }`. A call ends only after it was missing for 45 s,
with `end` = when it was first missed, so one missed check never splits it. `meetings:check` (Check Now in
Settings; later the rail) waits for a running check and ends a missing call at once. Detection switched off
ends a call at once.

**Signals.** Every change goes to `calls::subscribe` (a broadcast of `CallEvent`) for the features that follow
calls, refreshes the menu bar and emits `call_changed` (no payload); the Daybook window reloads its view.
`calls::active` is the call now; the tool state carries its start and app (`meeting_since`, `meeting_app`):
the menu shows "In a meeting since 10:15", the sidebar's Call row the same with the app.

## Joining

`calendar_join(url)` (the reminder's Join and an event row) accepts only a join link (`events::join_link`) and
runs `calls::join`, following Settings → Calls → Joining a Call (`settings.joining`):

1. Start-first apps that aren't running are started in the background (`open -g -b <id>`); if one was started,
   Daybook waits 2.5 s (a virtual camera such as Camo needs a moment).
2. The link opens in the app chosen for its service (`open_with`): `open -b <id> <url>`, and a Chrome web app
   (the Meet app, `com.google.Chrome.app.<id>`) through Chrome: `open -n -b com.google.Chrome --args
   --app-id=<id> --app-launch-url-for-shortcuts-menu-item=<url>`. Without a choice, or when that fails, the app
   macOS has for the link opens it.

## Permissions

- **Accessibility** (other apps' window titles): `calls_request_access("accessibility")` shows macOS's prompt
  the first time and opens Privacy & Security → Accessibility after that. Settings → Calls asks for it only
  while an enabled trigger without a built-in check exists; Settings → General shows the state. An ad hoc
  signed debug build loses the grant with every rebuild.
- **Automation** (Chrome's tabs): macOS asks with the first script; after a refusal
  (`-1743`) Settings → Calls offers Open Privacy Settings.

## Commands

| Command | Arguments | Returns |
|---|---|---|
| `calls_info` | – | `CallsInfo`: `detecting`, the call (`CallView`: app, title, `since` HH:MM), `accessibility`, `missing` permissions, the last `error`, the built-in triggers with their check text |
| `calls_request_access` | `permission: Missing` | whether Accessibility is allowed now; `io` when the privacy settings don't open |
| `calls_pick_app` | – | the app chosen in the file panel (`/Applications`) as `AppRef` (name, bundle id from `Info.plist`), null when cancelled; `invalid_input` for anything but an app |
| `meetings:check` (bus) | – | Check for a Call Now |
