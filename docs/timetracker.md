# TimeTracker

Daybook never tracks time ([ADR 0022](adr/0022-daybook-drives-timetracker-through-its-cli.md)). TimeTracker
(`../timetracker`) owns timers and recordings; Daybook reads what it is timing and drives
it through its `timetracker` command line tool.

## The client

`timetracker/client.rs`: the `TimeTracker` trait is the only seam (tests use a fake); `Cli` runs
`timetracker … --json` (`settings.timetracker.path`, default `~/.local/bin/timetracker`) with an 8 s timeout and
maps the result (`reply_result`):

| Exit | Means | Error kind |
|---|---|---|
| 0 | done; stdout is the JSON reply (empty means `null`) | – |
| 1 | TimeTracker refused; its JSON error is on stdout | `timetracker_refused` |
| 2, "TimeTracker is not running" or "Could not reach…" | not running or hung | `timetracker_not_running` |
| 2, `error: …` | an older TimeTracker or bad words | `timetracker_refused` |
| no exit code, timeout | didn't answer | `timetracker_not_running` |
| can't start the tool | not installed | `timetracker_missing` |

Notes go as `--note=<text>`, so a note starting with `-` is never read as a flag. **Debug builds never send a
command that changes TimeTracker's data** (`timer start`, `timer stop`, `record`, `changes_data`): they log it as a
dry run, so a debug Daybook can't touch the owner's real timers. Reads and `show` run as usual.

`timetracker/reply.rs` parses the replies (`timer status`, `projects --archived`, `status`, refusals, watch lines).

## Watching

`timetracker/watch.rs`: one background thread keeps `timetracker watch --json` running. The reply first carries the
current timer and tracker, then one line per change: `timer` (the `TimerState`), `data` (`projects` reloads the
cached project list and rereads the timer), `tracker`. When the stream ends Daybook reconnects with backoff (2, 4, 8,
16, 30 s; a minute while the tool is missing); an older TimeTracker without `watch` is polled with `timer status`
every 30 s instead. Changing the path reconnects at once.

The state is `tt: available | notRunning | missing` and `timer: Option<TimerState>`, emitted as `timer_changed`
(`TimerChanged { tt, timer }`); the tray, the rail and the Daybook window follow it.

## Linking projects

A Daybook project lists the TimeTracker projects it stands for (`timetrackerProjects`, Settings → Projects); in
TimeTracker a client such as Oscorp has several projects. The first time projects load, `link::auto_link` links
each Daybook project to the TimeTracker projects with its name or its client's name (case-insensitive) and sets
`timetracker.linked`; projects it can't match stay unlinked. Renames in TimeTracker follow (`follow_names`).
`current_project()` is the Daybook project, among the current context's projects, whose list holds the timer's
project.

## What Daybook shows

- **Menu-bar title** (`menu/title.rs`, README decision 6): only the icon and the context emoji while Working with a
  running timer (TimeTracker's own item shows it); `No timer` while Working without one; `Working` when TimeTracker
  isn't running or is missing; `Break · 12m` on a break; nothing for Off.
- **Sidebar and rail:** the Activity row and button show the timer as "Oscorp · note" (no elapsed time; the
  linked Daybook project's name when there is one); the rail's caption is the project.
- **Slack:** the Working text appends the current project ("9:00 - 18:00 | Oscorp") and follows a project change
  ([slack.md](slack.md)).
- **Links and Projects:** project rules in Settings → Links and the Projects tool's order use `current_project()`.
- **Open in TimeTracker:** `timetracker:show` (tray, palette, Daybook window) runs `timetracker show`; the family
  `timetracker:show:<YYYY-MM-DD>` shows another day.

Settings → TimeTracker shows the tool's path, whether TimeTracker runs (and its version) and Open TimeTracker.

| Command | Returns |
|---|---|
| `timetracker_info` | `TimeTrackerInfo { path, state, version }` (runs `status --json`) |
| `timetracker_projects { fresh }` | `TimeTrackerProjects { state, projects: TtProject[] }` (archived included) |
| `timetracker_timer` | `TimerChanged` |
| `timetracker_focus` | –; starts or shows TimeTracker |

## The activity overlay

`activity/` (`activity:switch`, ⌘F2; family `activity:prefill:<project>:<note>[:<ttId>[:<tag>]]`) opens the sticky
`activity` overlay with `ActivitySession` (`session.rs`, pure): the prompt ("What are you working on?", or "Still on
Oscorp · QRS-6805?" for a check-in), the note prefilled with the timer's note, and the current context's projects
with their linked TimeTracker projects (a project without links shows how to link it). `activity_suggestions`
(`suggest.rs`, pure) lists the live call, today's events since the timer started (at most 3), today's Jira tickets
and recent timers (`timer recent`, `recent.rs`; a pick fills project, note and tag).

↵ (`activity_submit`, `pick.rs`): a check-in whose project and note equal the running timer's sends nothing ("still
on it"); otherwise `timer start <ttProjectId> --note=<note> [--tag=<tag>]` (the tag carried over, so TimeTracker
continues today's recording), and then Working is set when the status was Break or Off (marked `byActivity`, so the
status change starts no timer of its own and sends no channel message). A TimeTracker error keeps the overlay open
with the typed text; "Open TimeTracker" starts it. Debug builds dry-run the start.

## Check-ins

`check_in.rs` (pure) decides on the scheduler's 15 s tick, in order: no check-in without a vault, outside Working, in a
context without the Status feature or with `checkInMinutes` 0, outside work hours, when paused today (`checkin:pause`,
the tray's "Pause Check-ins Today"; `checkins.json`, per day) or while TimeTracker isn't running; during a call the
count stops and starts again after it; while away it waits; when another overlay is open it tries again in 60 s;
else it opens the overlay in check-in mode. A timer change from TimeTracker or a context switch starts the count
again. The overlay's 60 s timeout counts only while it has focus and confirms nothing.

Recent timers are cached: the overlay asks again after 4 s, the tray, rail and palette after a minute; any timer
change drops the cache. The tray's Activity part (`menu/activity.rs`) shows the timer or TimeTracker's state, "Log
Activity…" (with recent timers as a submenu) and the pause checkbox.
