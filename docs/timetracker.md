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
project; without a timer `tools::snapshot` falls back to the kept activity's project
([The activity overlay](#the-activity-overlay)).

## What Daybook shows

- **Menu-bar title** (`menu/title.rs`, README decision 6): only the icon and the context emoji while Working with a
  running timer (TimeTracker's own item shows it); `No timer` while Working without one; `Working` when TimeTracker
  isn't running or is missing; `Break · 12m` on a break; nothing for Off.
- **Sidebar, rail and tray:** they show `ToolState.activity`: the running timer, else the activity kept without
  one (`activity::current_view`). The Activity row and button read "Oscorp · note" (no elapsed time; the linked
  Daybook project's name when there is one); the rail's caption is the project; the tray's Activity part adds
  "(no timer)" to a kept activity.
- **Slack:** the Working text appends the current project ("9:00 - 18:00 | Oscorp") and follows a project change
  ([slack.md](slack.md)).
- **Links and Projects:** project rules in Settings → Links and the Projects tool's order use `current_project()`.
- **Open in TimeTracker:** `timetracker:show` (tray, palette, Daybook window) runs `timetracker show`; the family
  `timetracker:show:<YYYY-MM-DD>` shows another day.

Settings → Connections → TimeTracker shows the tool's path, whether TimeTracker runs (and its version) and Open TimeTracker.

| Command | Returns |
|---|---|
| `timetracker_info` | `TimeTrackerInfo { path, state, version }` (runs `status --json`) |
| `timetracker_projects { fresh }` | `TimeTrackerProjects { state, projects: TtProject[] }` (archived included) |
| `timetracker_timer` | `TimerChanged` |
| `timetracker_focus` | –; starts or shows TimeTracker |

## The activity overlay

`activity/` (`activity:switch`, ⌘F2; family `activity:prefill:<project>:<note>[:<ttId>[:<tag>]]`) opens the sticky
`activity` overlay with `ActivitySession` (`session.rs`, pure): the prompt ("What are you working on?", or "Still on
Oscorp · QRS-6805?" for a check-in, from the current activity), the current context's Daybook projects
(`activity_projects`), each with its active linked TimeTracker projects (`tt_projects`; archived ones left out) and
`timerRow` when it links more than one, the project, timer project and note picked first (the prefill's, else the
timer's, else the kept activity's). `activity_suggestions` (`suggest.rs`, pure) lists the live call, today's events
since the timer started (at most 3), today's Jira tickets and recent timers (`timer recent`, `recent.rs`; a pick
fills the Daybook project, the timer project, the note and the tag).

The overlay picks two things ([ADR 0028](adr/0028-overlays-follow-the-prototype.md)): the **activity** (a Daybook
project and a note) and the **timer** (a TimeTracker project, or none). The project's links decide the timer
(`pick::timer_project`):

| Linked TimeTracker projects | Timer |
|---|---|
| none | never a timer |
| one | always a timer on it |
| several | the one picked in the timer row, or "None" (the default) for no timer |

↵ (`activity_submit`, `pick.rs` `plan`, done by `commands::submit_pick`):
- a timer project: `timer start <ttProjectId> --note=<note> [--tag=<tag>]` (the tag carried over, so TimeTracker
  continues today's recording);
- no timer (`Plan::NoTimer`): `timer stop` when one runs, else nothing;
- a check-in confirmed as it opened (`confirms`: the kept activity's project and note, and with several links the
  running timer's project) changes nothing (`Plan::Unchanged`), not even a timer on a project no Daybook project
  links; a check-in whose timer already runs on that project and note, or with no timer and none running, sends
  nothing either.

Then the pick becomes the current activity, and Working is set when the status was Break or Off (marked
`byActivity`, so the status change starts no timer of its own and sends no channel message). A TimeTracker error
keeps the overlay open with the typed text; "Open TimeTracker" starts it. Debug builds dry-run the start.

**The current activity** (`activity/current.rs`, `CurrentActivity` in `AppState`): the Daybook project and note
picked last, with or without a timer, kept in `activity.json` (`KeptActivity` for a day), so a restart keeps it.
It ends with the day (`get` drops one kept on an earlier day) and when the status becomes Off. It follows
TimeTracker's timer: a timer started or changed there sets it (`follow_timer`: its note, and the kept project
while that links the timer's project, else the first project linking it, else TimeTracker's name); a timer that
stops leaves it. The surfaces show the running timer, else the kept activity (`activity::current_view`,
`ToolState.activity`); a change of the kept activity alone announces `timer_changed` again
(`timetracker::announce_timer`), so they read it.

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

## The status drives the timer

`timetracker/drive.rs` follows `status_changed` while `timetracker.statusTimer` is on (Settings → Workflows, "Start
and stop the timer"; on by default). `status_timer::timer_step` (pure) decides:

| Change | Timer | Does |
|---|---|---|
| set by picking an activity (`byActivity`) | any | nothing (the pick started its timer) |
| None, Break or Off → Working | running | nothing |
| None, Break or Off → Working | idle or unknown | continues the last timer (`timer recent --limit 1`: same project, note and tag, so TimeTracker continues today's recording) when its project belongs to a current project; else opens the activity overlay (only in a context with the Status feature) |
| → Break or Off | running or unknown | `timer stop` |
| → Break or Off | idle | nothing |

The timer changing in TimeTracker never changes the status ([ADR 0019](adr/0019-daybook-owns-the-status.md)). When
TimeTracker isn't running the status still changes and a notification ("TimeTracker isn't running", "The timer wasn't
started." or "…stopped.") offers Open TimeTracker.

## Calls become suggestions

On a call's end (a call that drops and comes back within the grace is one call), the call waits 2 minutes
(`split_calls.rs`, pure `SplitCalls`): when the same app (bundle id) is in a call again within 2 minutes of the end
(a reconnect, breakout rooms), the new part continues it and the joined call (the first part's start and, unless it
was "Meeting", its title) is suggested once it ends for good. A call moving to another app (`Switched`) ends the old
one at once. The status counts as it was when the call ended. Then `call_suggestion` (pure) checks, in
order: the status is Working; the context has the Status feature; the call (cut at midnight) lasts 3 minutes or
more. Every call that passes goes to TimeTracker ([ADR 0029](adr/0029-every-call-goes-to-timetracker.md)): when the
title (or, for a generic "Meeting", the overlapping event's) names a current project by its name or a meeting word and
that project is linked to TimeTracker, on its first link; otherwise without a project. It sends
`timetracker record [--project <id>] --span <start>-<end> --note=<App> - <title> --source Daybook`: a stored
suggestion TimeTracker shows as "Suggested by Daybook" to approve; one without a project waits in TimeTracker's Day
view for the owner to pick one, and the action log reads "Suggestion: Without a project …". TimeTracker shows only the
part of a suggestion no recording covers, so a call made while a timer runs often doesn't show. When TimeTracker isn't
running a notification says the call wasn't sent. Every outcome is logged.

## The Activity tab

The inspector's Activity tab shows TimeTracker's day read-only ([ADR 0024](adr/0024-daybook-shows-timetrackers-day-read-only.md));
it is offered (`ToolState::offers_day_activity`, `HubView.activityTab`) in a context with the Status feature while
TimeTracker is installed. `timetracker_day { day }` runs `recordings <day> --json` and `suggestions <day> --json`
(`client::recordings`, `client::suggestions`; reads, so debug builds run them too) and returns
`TimeTrackerDay { state, activity }`: no `activity` while TimeTracker isn't running or is missing, an error when it
refuses.

`day/view.rs` (pure, `day_view`) builds `DayActivity`:
- **Timeline:** one row per span of each recording and each suggestion, in start order. A span without an end is the
  running row ("now"). A gap of 5 minutes or more between rows is a "Nothing recorded" row; suggestions count as
  covered time. Suggestions show "Suggested by Slack" and count toward no total; one without a project reads
  "No project" in place of the project's name and draws in graphite.
- **By Project:** projects in order of first appearance with their total, then one line per recording: the note
  (or "No description"), the tag and its spans ("10:30–11:30, 12:00–12:45").
- Totals use TimeTracker's durations, formatted like TimeTracker (`1h 05m`, `45m`). Colours come from the project
  list; a project without one draws in graphite.

Rust sends every text (`title`, `copyText`, `suggestedBy`, `empty`), so the page builds none. Clicking a duration
copies it; clicking a note or a work row copies the note.

**Reloads:** Rust remembers the day the tab asked for last and emits `activity_changed` when `day/reload.rs`
(`reloads`, pure) says so: a `watch` data line for recordings, rules, tags or projects of that day (or of no day;
projects after the colours reloaded), TimeTracker starting or quitting, and the timer changing while the tab shows
today. The page also reloads on a day change, on window focus and every minute while the tab shows.

`activity:showTab` ("Day Activity", in the menu-bar menu's Hub tool and the palette) shows the Daybook window and
emits `show_activity_tab`; the page opens the inspector on the tab. The tab (`inspectorTab`) and its view
(`activityView`: timeline or byProject) are kept in `ui_state.json`. The tab's toolbar link is
`HubView.timetracker` (`timetracker/commands.rs` `show_link`): "Open in TimeTracker" (`timetracker:show`) today, "Show <date> in
TimeTracker" (`timetracker:show:<YYYY-MM-DD>`, the date by `DateLocale::long_date` in the Mac's locale) on
another day.
