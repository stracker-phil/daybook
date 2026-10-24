# Status

`src-tauri/src/status/`. Working, Break and Off are Daybook's own state ([ADR 0019](adr/0019-daybook-owns-the-status.md)):
the owner sets them, Daybook remembers them, and other features react to a change. The daily note is never
read for the status and no status writes a line into it. Status UI shows only in contexts with the `status`
feature; the commands work in every context, so Off can still be set after switching away.

| File | Holds |
|---|---|
| `mod.rs` | `set_status`, `StatusChanged`, `status_label`, the commands, the minute tick |
| `memory.rs` | `StatusMemory`, `Statuses` (`status-memory.json`), `is_status_change` |
| `request.rs` | the overlays' args: `StatusRequest`, `EndOfDayCheck`, `OfferOff`; `toggled`, `offers_off` |

## Memory

`status-memory.json` in the config dir: `{ date: "YYYY-MM-DD", status, since: "HH:MM", workedToday }`, the
shape the prototype import writes. `Statuses::today(now)` is the memory if it is from today, else a new day
without a status ("No status yet"). `since` is the time the status was set, so it survives a restart.
`workedToday` tells the day's first Working apart.

## The change rule

`is_status_change(current, next)`: picking the status you have changes nothing; the first status of a day always
counts, even when it matches yesterday's last. `Statuses::set` applies it: an unchanged pick returns `None` and
nothing happens (no event, no message, nothing sent).

## `status_changed`

`set_status(app, status, message, end_of_day)` remembers the status, then emits `status_changed` with a
`StatusChanged { previous, next, first, since, message, endOfDay }`: `previous` is `None` for the day's first
status, `first` marks the day's first Working, `message` is the channel message as confirmed in the overlay
(empty: none), `endOfDay` the end-of-day check's answer when going Off (`done`, `incomplete`). Then it refreshes
the menu-bar item.

Listeners:

| Who | Does |
|---|---|
| the Daybook window | reloads `hub_view` (the sidebar's status row) |
| later steps, in Rust (`app.listen(STATUS_CHANGED_EVENT, …)`, parsing the payload) | Slack status and channel message (step 11), focus list (12), the day wizard (14), music (19), the timer (26) |

## The overlay

`status:working`, `status:break` and `status:off` open the overlay view `status` with that status picked;
`status:toggle` (the `toggleStatus` hotkey, ⌘F1; the sidebar row; the menu's Change Status…) picks Break while
Working, else Working. The same request again closes it. The args are a `StatusRequest`:

- `status` (picked), `current` (marked Current);
- `messages`: the suggested message per status from `statusMessages`; the day's first Working suggests
  `firstWorking` when it is set;
- `endOfDay`: the check's reminder, question and link, while `endOfDay.enabled` and the Off workflow
  (`slack.manualWorkflows.off`) are on.

The page (`src/windows/panel/StatusView.tsx`) keeps the keyboard in the message field: ↑↓ or ⌘1–3 pick, ↵ or
a click sets. The suggested message follows the pick; typed text stays. Going Off with a check shows
`EndOfDayStep`: the reminder and the question (numbered when both exist), Open for the link, Yes (Y) and Not
yet (N); it asks `panel_keep_open`, so the overlay stays while the owner checks the link elsewhere. Esc cancels
and sends nothing. Setting calls `status_set(status, message, endOfDay)`, which sets the status and closes the
overlay, also when nothing changed. The focus list step comes before the check with step 12.

## Offer-off

Leaving a context with the `status` feature for one without while Working (`offers_off`) asks whether to go
Off:

- by hand (switcher, menu, `mode:<id>`): the switcher becomes the `offer-off` view (`OfferOffView`): "Set to
  Off" runs `status:off` (the status overlay, so the end-of-day check comes first), "Keep Working" closes;
- by work hours: the notification "Switched to …" says "You're still set to Working." and offers a Set to Off
  button (`notify::show_with_actions`; a button's id is the command it runs).

## Surfaces

- **Sidebar row** (`hub/tools.rs`): the status label ("Working and present", "Taking a break", "Done for
  today", "No status yet"), the time it was set, and a dot (`HubView.status`: green, orange, grey, a ring).
- **Menu-bar title** (`menu::model::menu_bar_title`): the context's label, then `Working` or `Break · 12m`;
  nothing for Off or before the first status, nor in a context without the feature. `status::start` refreshes
  the item every minute, so the break minutes move.
- **Menu-bar menu**: "Status: Working" with Change Status… and the three statuses as radio items.
- **Palette**: the statuses from the menu (`Status: Break`); not the toggle.

## Settings

Settings → Workflows (`WorkflowsPage.tsx`): "When you pick a status" switches each status's workflow
(`slack.manualWorkflows`) and edits its suggested message (`statusMessages`, with the first Working of the day);
Off's "Ask first" switches the end-of-day check. "End-of-day check" edits its reminder, question, link and the
messages for yes and not yet (`{message}` is the channel message).
