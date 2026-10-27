# Presence and calls

Daybook knows whether the owner is at the Mac and whether a call is running, and joins a meeting in the
right app. Presence gates what may open by itself; calls feed the sidebar, the menu bar, the rail, Slack, music
automation, the wizard and TimeTracker's call suggestion. Daybook does not log calls.

| File | Holds |
|---|---|
| `presence/checks.rs` | pure: `Away { asleep, locked }`, `after_wake`, `is_coming_back`, `has_been_asleep_long_enough`, `locked_when_unreadable`, `is_screen_locked_output` |
| `presence/native.rs` | sleep, wake, lock and unlock notifications; the lock state from `ioreg` |
| `presence/mod.rs` | `Presence` (managed state), `is_away`, `is_asleep`, `verify`, what waking and coming back do |
| `calls/checks.rs` | pure: call titles, `microphone_pids`, `runs_from_bundle`, `lsappinfo` parsing, `parse_chrome_tabs`, the built-in checks' texts |
| `calls/detect.rs` | `detect(rules, probe, active)` over the `Probe` trait: one `RuleCheck` (`Found::Call`, `NotInCall`, `Unknown`) per rule; `Missing` permissions |
| `calls/tracker.rs` | pure: `Tracker::observe`, the call over a series of checks (`ActiveCall`, `CallEvent`, `Mode`) |
| `calls/titles.rs` | pure: `EventTitles`, the calendar title of a "Meeting" call, once per event |
| `calls/probe.rs` | `MacProbe`: the detectors' questions to the Mac; `windows_failure` (Accessibility errors) |
| `calls/access.rs` | Accessibility: trusted, the prompt, the privacy panes |
| `calls/join.rs` | joining a call: `open_args`, `join` |
| `calls/pick.rs` | `app_ref`: an app chosen in Settings, its name and bundle id (`plutil`) |
| `calls/watch.rs` | the check: the 15 s tick, `check_soon`, `check_now`, `woke`, `first_check`, announcing changes |
| `calls/mod.rs` | `Calls` (managed state), `active`, `subscribe`, `settings_changed`, the commands, `meetings:check` |

## Presence

Away means asleep or locked. Nothing may open by itself while away: a window shown at the lock screen stays
on screen after unlocking but never gets keyboard focus. Features that open an overlay without being asked
(routine and journal prompts, check-ins) check `presence::is_away` first.

macOS's events go missing at times (the lock at sleep is usually never announced), so:

- `NSWorkspaceWillSleepNotification` sets asleep and remembers when; `NSWorkspaceDidWakeNotification` reads
  the lock state from `ioreg -n Root -d1 -r` (`"CGSSessionScreenIsLocked"=Yes`) and decides with `after_wake`.
  An unreadable state counts as locked, for at most 5 minutes in a row (`locked_when_unreadable`), so a broken
  `ioreg` can't keep the prompts off for good. Waking is never coming back by itself.
- The distributed `com.apple.screenIsLocked` / `screenIsUnlocked` set and clear locked; an unlock while asleep
  is left to the wake.
- While locked, `verify` reads the lock state every 2 s until it is unlocked.
- While away, the 15 s call tick runs `verify` first: asleep for at least a minute of wall time means awake
  again even without a wake event (timers stop during sleep).

Checks run one at a time. A wake (announced or noticed) hands the time the Mac fell asleep to `calls::woke`.
Coming back (away → not away) runs a call check as if asked (`calls::check_now`), refreshes the menu bar and,
500 ms later, gives an open overlay the focus again (`window::panel::refocus`).

## Calls

`calls::start` checks every 15 seconds while the Mac is awake, at the lock screen too (no UI is involved), with
a vault set and Settings → Workflows → App triggers → Watch apps on (`detecting`). It starts after every
follower subscribed (`app.rs`), so none misses the first check's call. A regular check skips while one runs;
every other check waits for it: `check_soon` after a change of `meetings.detection`, the rules or the vault
(`calls::settings_changed`) and of the context, `check_now` when the owner comes back, and the check after a
wake.

**Detectors.** The enabled app triggers (`settings.meetings.rules`) are checked in order, each with its own
answer; checking stops once one is in a call and the active call's trigger has answered:

| Trigger | In a call while | Permission |
|---|---|---|
| Zoom (`us.zoom.xos`) | Zoom runs and `pgrep -x CptHost` finds its meeting process | none |
| Teams (`com.microsoft.teams2`) | Teams has a level-19 window on any Space (the window server's list), or a process inside the Teams bundle records from a microphone (`pmset -g assertions`: `audio-in` assertions and their `Created for PID`, mapped with `ps -o comm=` to the bundle path from `lsappinfo info`) | none |
| Meet (the Google Meet Chrome app) | Chrome runs, a tab shows `meet.google.com/xxx-xxxx-xxx` (JXA through `osascript`) and a process in the Chrome bundle records from a microphone (the tab stays on the page after leaving); titled by the tab ("Meet – Weekly" → "Weekly") | Automation for Chrome |
| any other app | it runs (`lsappinfo list`); with "only while it records from the microphone" (`needs_microphone`), while it does; with a window title set, while one of its windows' titles contains it (Accessibility, `AXWindows`/`AXTitle`, 2 s timeout; a window-title rule sees only the windows on the current Space); titled by the window, without the app's name | Accessibility |

The prototype tried `AVCaptureDevice.isInUseByAnotherApplication` for the microphone, which always said no;
the power assertions work. A Meet call joined without a microphone isn't seen.

A trigger whose app doesn't name the call is titled "Meeting"; the calendar event running now replaces that
title (`call_title`), looked up once per event and app (`EventTitles`: until the event ends, without one again
after 5 minutes). A refused permission leaves that check without an answer (not in a call) and Settings shows
it. Any other failure (a timed-out Accessibility request, unreadable JSON from Chrome, a failing `pmset`)
makes only that trigger unknown and shows as the error in Settings; without `lsappinfo list` every trigger is
unknown.

**The call.** A call is its trigger's, keyed by bundle id: a trigger renamed mid-call keeps the call
(`Changed`). `Tracker::observe` turns checks into `CallEvent`s:

- `Started`: no call ran and a trigger is in a call (the first in order).
- `Changed`: the same app's call with a new title or trigger name; it keeps its start.
- `Switched { from, to, end }`: the active call is gone and another app is in a call; one change, not an end
  and a start. `to` starts when it was first seen while `from` was missing.
- `Ended { call, end, reason }`: gone (`Gone`), detection switched off (`Off`), or its detector failing
  (`Failing`).

The active call's trigger is asked first. While it is missing for less than 45 s, nothing changes, even when
another app is in a call (a stray Meet tab or Teams window doesn't take over); after that it ends or
switches with `end` = when it was first missed, so one missed check never splits it. `meetings:check` (Check
Now in Settings; later the rail) and coming back end a missing call at once (`Mode::Asked`). The first check
after a wake ends a missing call at once at the earlier of its first miss and the time the Mac fell asleep
(`Mode::Woke`), so a call that ended during the sleep doesn't run on to the wake. Detection switched off ends
a call at once.

**A failing detector.** While the active call's own detector fails, the call keeps running; after 4 failed
checks in a row (about a minute) it ends at its first miss or first failure (`EndReason::Failing`), with a log
line, an action-log line under "Call ended: <app>" and one notification. Another trigger failing never ends or
holds the call.

**Signals.** Every change goes to `calls::subscribe` (a broadcast of `Announced`: the `CallEvent` and its
action-log trigger: "Call started", "Call moved: Zoom → Teams", "Call ended") for the features that follow
calls, refreshes the menu bar and emits `call_changed` (no payload); the Daybook window reloads its view.
`calls::first_check` returns once the first check since the start ran (Slack and music restore what a quit
mid-call left). `calls::active` is the call now; the tool state carries its start and app (`meeting_since`,
`meeting_app`): the menu shows "In a meeting since 10:15", the sidebar's Call row the same with the app.

| Follower | Started | Switched | Ended |
|---|---|---|---|
| Slack (`slack/mod.rs`) | the trigger's call status | only the new trigger's status (none when it is the same) | restores the picked status |
| Music (`music/automation.rs`) | lowers or pauses | nothing: stays lowered | restores, resumes |
| Wizard (`wizard/mod.rs`) | MeetingStart | nothing | MeetingEnd |
| TimeTracker (`timetracker/drive.rs`) | a restart within 2 minutes continues the ended call | the old call ends (a suggestion) | a suggestion, after 2 minutes |

## Joining

`calendar_join(url)` (the reminder's Join and an event row) accepts only a join link (`events::join_link`) and
runs `calls::join`, following Settings → Workflows → Joining a Call (`settings.joining`):

1. Start-first apps that aren't running are started in the background (`open -g -b <id>`); if one was started,
   Daybook waits 2.5 s (a virtual camera such as Camo needs a moment).
2. The link opens in the app chosen for its service (`open_with`): `open -b <id> <url>`, and a Chrome web app
   (the Meet app, `com.google.Chrome.app.<id>`) through Chrome: `open -n -b com.google.Chrome --args
   --app-id=<id> --app-launch-url-for-shortcuts-menu-item=<url>`. Without a choice, or when that fails, the app
   macOS has for the link opens it.

## Permissions

- **Accessibility** (other apps' window titles): `calls_request_access("accessibility")` shows macOS's prompt
  the first time and opens Privacy & Security → Accessibility after that. Settings → Workflows → App triggers
  shows the state with Allow Access…, and warns while an enabled trigger without a built-in check can't read
  titles. An ad hoc
  signed debug build loses the grant with every rebuild.
- **Automation** (Chrome's tabs): macOS asks with the first script, with the text of
  `NSAppleEventsUsageDescription` (`src-tauri/Info.plist`; without it the prompt never appears); after a
  refusal (`-1743`) Settings → Workflows → App triggers offers Open Privacy Settings.

## Commands

| Command | Arguments | Returns |
|---|---|---|
| `calls_info` | – | `CallsInfo`: `detecting`, the call (`CallView`: app, title, `since` HH:MM), `accessibility`, `missing` permissions, the last `error`, the built-in triggers with their check text |
| `calls_request_access` | `permission: Missing` | whether Accessibility is allowed now; `io` when the privacy settings don't open |
| `calls_pick_app` | – | the app chosen in the file panel (`/Applications`) as `AppRef` (name, bundle id from `Info.plist`), null when cancelled; `invalid_input` for anything but an app |
| `meetings:check` (bus) | – | Check for a Call Now |
