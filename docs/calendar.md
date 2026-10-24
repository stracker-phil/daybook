# Calendar and reminders

Daybook reads the calendars of the current context through EventKit, shows the day's events in the Daybook
window with a timeline, and puts one reminder at the top of the board. It never changes a calendar.

| File | Holds |
|---|---|
| `calendar/eventkit.rs` | the only EventKit code: access, `Store` (the event calendars and their events), change notifications |
| `calendar/events.rs` | pure: `Event`, `timed_events`, `running_event`, `meeting_link`, `is_meeting_url` |
| `calendar/mod.rs` | `Calendar` (the cache), `events_on`, the commands, `install` |
| `reminders/mod.rs` | pure: `pick_reminder` over the rules of Settings → Reminders |
| `hub/day.rs` | pure: the events section's view model, `day_events` (timeline geometry and rows) |
| `hub/reminder.rs` | pure: `reminder_line`, the reminder as the Today card's line |

## Access

Daybook asks for full access to events (read only; macOS has no read-only grant). The usage strings
`NSCalendarsFullAccessUsageDescription` and `NSCalendarsUsageDescription` are in `src-tauri/Info.plist`, which
Tauri merges into the bundle. The app isn't sandboxed and has no hardened runtime, so no entitlement is
needed. The grant belongs to the bundle id and the signing certificate (`scripts/sign-app.sh`); the debug
build (`com.philippstracker.daybook.debug`) is asked separately.

`calendar_info` returns `CalendarInfo { access, calendars }`: the access (`not-determined`, `restricted`,
`denied`, `full-access`, `write-only`) and, with full access, the titles of this Mac's event calendars, sorted
and without duplicates. `calendar_request_access` asks macOS while the access is undetermined (macOS shows its
prompt once) and drops the store, since a store made before the grant shows no calendars; after a refusal it
opens System Settings → Privacy & Security → Calendars instead. Either way it resolves the access now.

## Reading events

A context lists its calendars by title (`Context.calendars`, Settings → Contexts). `events_on(app, day, today)`
reads the current context's calendars on a blocking thread and returns the day's **timed** events
(`timed_events`): all-day events, canceled events and invitations the owner declined are left out; the rest is
sorted by start, in local time. Without calendars or without full access the list is empty.

`Calendar` (managed state) holds one `EKEventStore` behind a mutex and caches the events of a day per
calendar list for five minutes: today, plus the last other day the window looked at. `EKEventStoreChangedNotification`
(any event, calendar or access change) drops the cache and emits `calendar_changed`; the Daybook window
reloads its view and the Settings pages read `calendar_info` again. The window also reloads every minute, so a
cache older than five minutes is read again within a minute.

## Meeting links

`meeting_link(url, location, notes)` finds the first join link in an event's link, location and notes, in that
order of text: Zoom (`…zoom.us/j|my|w|s/…`), Teams (`teams.microsoft.com/l/meetup-join/…`, `/meet/…`,
`teams.live.com/meet/…`) and Meet (`meet.google.com/…`); help pages, dial-in and option links don't count.
Notes are often HTML: `&amp;` is decoded, and a link inside Google's redirect (`…&sa=D&source=calendar…`) is cut
before `&sa=D` and percent-decoded. `calendar_join(url)` opens only an address that is nothing but a join link
(`is_meeting_url`); step 10 replaces it with joining in the call app.

## The day model

`hub_view` adds two parts to `HubView`:

- `events: DayEvents { timeline, rows }` from `day_events`. Positions are fractions of the shown hours
  (Settings → Appearance → Day timeline, `timeline.firstHour` to `lastHour`):
  - `hours`: a tick per hour; labels every 1, 2 or 3 hours (up to 8, up to 16, more shown hours), hour 24 reads "00".
  - `workHours`: the current context's work hours on their weekdays; an end at or before the start (bedtime after
    midnight) runs to the end of the day.
  - `blocks`: one per event, clipped to the shown hours; an event outside them has a row but no block. `follows`
    marks a block another event reaches into, so a hairline sets them apart. `past`: ended by now.
  - `fullDay`: events that fill the whole day (from midnight to midnight or beyond) are a line along the bottom
    and read "All day" in their row.
  - `now`: today only, while now is within the shown hours.
  - `rows`: every event in order with its time ("09:00–09:30"), title, calendar, join link and `past`. Keys
    (`event-<index>`) tie a row to its block.
- `reminder: ReminderLine | null`, today only.

## Reminders

`settings.reminders` is an ordered list of rules; `pick_reminder` returns the first enabled rule that applies
now:

| Rule | Applies |
|---|---|
| `meeting { minutesBefore }` | a running meeting (the latest started) until it ends; without one, the next meeting from `minutesBefore` until it starts |
| `routine { routine }` | while the routine is due (its `timing.due`) and not done; a routine without items today or a due time never applies |
| `journal { group }` | while the group is due and has open questions; a free-text group until it is written |
| `todos { span, from, to }` | between `from` and `to` (an empty `to`: the end of the day), when open todos from the span (today, this week from Monday, this month, any day) up to today exist |

Every routine and journal group has exactly one rule (`with_item_reminders`). The routine, journal and todo
inputs are empty until their steps (13 to 15) fill them; the meeting rule works now.

`reminder_line` turns it into `{ label, text, action }`: "Now · Sprint Review until 15:30" or "Next · Sprint
Review starting at 15:00" with Join for a call (`{ kind: "join", url }`), "Routine · Evening · 2 open" with
Start or Continue (`routine:<id>`), "Journal · Dusk · 2 open" or "· not written yet" with Open
(`journal:open:<group>`), "Todos · 3 open todos from this week to review" without an action.
