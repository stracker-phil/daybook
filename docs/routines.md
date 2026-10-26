# Routines, the wizard and the scheduler

Routines are named checklists (Morning, Evening) you come back to during the day. Their state lives only in
today's note: opening a routine adds its items to the plan section unticked, and ticking one writes `[x]`. The
wizard opens a routine, or a journal group, by itself at the right moment. The scheduler's tick drives it,
along with the other work that falls due by the clock.

## Routines

Settings → Routines holds `settings.routines`: each has an `id`, a `name`, the `contexts` it runs in, its
`steps` and a `timing`. A step has a title, an optional `link` and the weekdays it is due (`days`, 0 = Sunday;
empty: every day). A routine runs in its contexts, or in every context when it lists none, but only while the
current context has the Routines feature (`active_routines`). Rust gives a new routine (empty id) its id from
the name on save; renaming keeps the id, so its reminder rule follows.

`routines/` reads and writes them:

| File | Does |
|---|---|
| `progress.rs` | pure: `routine_view(routine, lines, heading, weekday)` lists the day's items (`Routine::steps_on`) with their place in the routine (`index`) and whether the plan section ticks them (`read_plan_tasks`, title matched ignoring case and spacing); `RoutineView::progress` counts them (`RoutineProgress`: done of total) |
| `fired.rs` | `Fired` over `routines.json` (`{ date, fired: [key] }`): the once-a-day triggers that already fired today; a new day starts empty. The prototype import writes the same file |
| `mod.rs` | the commands, `open` and the `routine:<id>` family |

Opening a routine (`routine:<id>`, from the board, the menus, the palette, a reminder or the wizard) writes
`NoteOp::PlanList` with the day's titles (only the missing ones are added), then shows the `wizard` overlay with
`{ routine }`. Ticking writes `NoteOp::PlanStep` ([vault.md](vault.md#note-ops)). Both go through the writer and
wait for today's note when it doesn't exist yet.

| Command | Args | Returns |
|---|---|---|
| `routine_view_today` | `id` | `RoutineView`: today's items, ticked or not; `not_found` for an unknown routine |
| `routine_step` | `id`, `index`, `done` | –; ticks or unticks the item at `index` of the routine's steps |
| `routine_open_link` | `link` | –; opens an `http(s)://` or `obsidian://` link as it is, anything else as a vault path in Obsidian (`.md` added without an extension; a path leaving the vault is `invalid_input`). `vault::commands::open_target` |

Progress shows in the board's Routines row (`HubView.routines`, the current context's routines on the shown
day), the tool snapshot (`ToolState.routines`: the Journal menu's Routines section and the palette) and the
routine reminder rules ([calendar.md](calendar.md#reminders)).

## The wizard

`wizard/` opens something by itself. Its candidates (`prompts`) are the current context's routines, then the
journal groups, each in Settings order, with their `Timing`:

- **Due** (`openWhenDue` with a `due` window): it opens while the window is open, also when the Mac slept
  through its start. The scheduler tries it every tick.
- **Events** (`openOn`): `start` (Working after another status or none), `off` (Done for today), `context` (a
  context switch), `meeting-start` and `meeting-end` (a detected call). `wizard::install` listens to
  `status_changed`, `context_changed` and the calls' broadcast.

`first_to_open` (`wizard/prompts.rs`, pure) picks the first candidate that opens for the reason and still has
something to do: a routine with an unticked item today, a journal group with nothing written yet. The due
time, `start` and `context` open a candidate at most once a day: the key `routine:<id>@<reason>` or
`journal:<group>@<reason>` goes into `routines.json` before the check, so it counts as fired even when nothing
was open. Calls and `off` open every time. Only one thing opens per reason; the others wait for their next
chance.

It never opens:
- **while away** (asleep or locked, [presence-and-calls.md](presence-and-calls.md#presence)): events that
  came meanwhile are dropped, and a due time opens on the first tick back.
- **over another overlay**: an event waits in `Waiting` and gets its chance on a later tick once the overlay
  has closed. An event first waits 0.6 s, so Done for today opens the Evening routine after the status overlay
  has gone.
- **twice at once**: one opening at a time (`Wizard::opening`).
- **without a vault**.

Debug builds have `dev_wizard_fire(reason, at?)`: it fires `due` or an event as if it happened today at `at`
(`HH:MM`), also while away; the once-a-day keys still apply, so delete the debug data folder's `routines.json`
to fire one again.

### The overlay

The `wizard` view (`WizardView`): the routine's name with "2 of 4" or "All done", then its items as a list. ↑↓
select (cycling through all items, ticked ones too), Space or ↵ (or a click) ticks or unticks the selected item
at once and writes it, ⌘O or the row's link button opens its link. It opens on the first unticked item. When the
last open item is ticked here it shows "All done" and closes 0.7 s later; a routine opened when already done
stays open. There is no skip: an item is ticked or still open. The view reads again on `note_changed` and
`settings_changed`, so ticks made in Obsidian show.

## The scheduler

`scheduler.rs` ticks every 15 s, and once at once when the owner comes back to the Mac (`presence`):

1. the context switch by work hours (`contexts::apply_schedule`, [backend.md](backend.md#contexts-and-projects));
2. the changes waiting for today's note (`DailyNotes::note_appeared`: written once the note exists and
   Templater is done with it);
3. the wizard: the waiting events, then the due times.

Calls keep their own 15 s check and the menu bar its minute refresh; check-ins come with the TimeTracker steps.
