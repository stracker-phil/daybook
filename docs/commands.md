# Commands

Every app-level action is a command on the Rust command bus, run by id from every entry point: the
menu-bar menu, the app menu, global shortcuts, the palette, and later the command line, links, the HTTP API,
the rail and Stage ([ADR 0017](adr/0017-commands-live-in-rust.md)). The page's own registry only holds keys
inside one window ([frontend.md](frontend.md#commands)).

## The bus

`bus/mod.rs`: `CommandBus<H: Host>`, generic over its host so it is tested without Tauri. The app is the
host (`bus/app.rs`): `has_vault` reads the settings, `choose_vault` opens Settings, `report_failure` shows a
notification (`notify.rs`). The managed `bus::Commands` holds the app's bus, filled once at startup
(`app.rs` `commands_bus`).

- **Commands.** `Command::new(id, title)` gives a `Spec`: `.help(line)` (defaults to the title), `.hidden()`
  (left out of help and lists), `.needs_vault()`, `.quits()` (the app is gone afterwards, so a caller answers
  first), `.enabled_when(check)`; then `.run(async fn)` or `.run_now(fn)` makes the `Command`. `bus.add`
  registers it; the same id again replaces it.
- **Families.** `Command::new(prefix, title).family("<arg>")` with `.choices(fn)` (what the argument can be
  now, for the command line) and `.from_words(fn)` (the argument from typed words), then `.run(async fn(host,
  argument))`; `bus.add_family`. An id runs the exact command if there is one, else the family with the
  longest matching prefix, with the rest of the id as its argument.
- **Running.** `start(host, id)` refuses an unknown id (`NotFound`) or a disabled command (`InvalidInput`);
  a command that needs a vault opens Settings instead when there is none. `run_or_err` awaits it for callers
  that report failures themselves; `run` logs an unknown id and reports a failure to the host.
  `bus::spawn(app, id)` runs one without waiting (menu clicks, global shortcuts); the page calls the Tauri
  command `run_command(id)`, which returns the failure.
- **Reading.** `knows(id)`, `info(id)` (`Info`: title, help, hidden, needs_vault, quits; `group(id)` is the
  id's first word), `is_enabled(host, id)`, `family(prefix)`, `list()` in registration order, and
  `available(host)`, the `CommandStatus` menus and the palette filter by.

### The commands so far

`bus/commands.rs` registers the app's own commands; each feature adds its own with its step (`contexts::register`, `status::register`, `calls::register`).

| Id | Title | Notes |
|---|---|---|
| `app:settings` | Settings… | opens the Settings window where it was (`window::open_settings`) |
| `settings:<page>` | Open a Settings page | family; choices: `window::settings::PAGES`, then `SECTIONS` (former pages, titled "Calendar (Connections)"), which open their page scrolled to that group; an unknown page is `not_found` |
| `view:toggleInspector` | Show or Hide the Inspector | shows the Daybook window and toggles its right column |
| `app:quit` | Quit Daybook | quits |
| `app:logs` | Open Log Folder | |
| `app:readme` | README | |
| `window:close` | Close Window | hidden; an overlay closes as Esc closes it, the Daybook window hides |
| `hub:open` | Open Daybook | the Daybook window on the pointer's screen |
| `hub:toggle` | Show or Hide Daybook | hides it when it is in front |
| `palette:open` | Command Palette… | the palette overlay; again closes it |
| `daily:open` | Open Today's Note | needs a vault; creates the note when missing |
| `daily:create` | Create Today's Note | needs a vault; disabled while Obsidian creates it |
| `context:switch` | Switch Context… | needs a vault; the switcher overlay (`contexts/`) |
| `mode:<context>` | Switch to a context | family; needs a vault; choices: the contexts; leaving the status behind while Working offers Off ([status.md](status.md#offer-off)) |
| `routine:<routine>` | Open a routine | family; needs a vault; choices: the current context's routines; adds the day's items to the note and shows the wizard overlay ([routines.md](routines.md)) |
| `notes:quick` | Quick Notes… | needs a vault; the topic picker overlay, or brings an open editor forward ([vault.md](vault.md#quick-notes)) |
| `notes:topic:<level>:<title>` | Write a topic of today's notes | family; needs a vault; choices: today's topics; opens the editor on it |
| `notes:present` | Present Quick Note | while the editor is open; brings it forward and toggles presentation (⌘⌥P in the editor; `COMMAND_SHORTCUTS` `alt+cmd+p` for Stage) |
| `notes:endPresentation` | End Quick Note Presentation | hidden; Esc while presenting; nothing when not presenting |
| `search:open` | Search Vault… | needs a vault; the `search` overlay with the context's default scope ([search.md](search.md)) |
| `search:scope:<scope>` | Search Vault… in one scope | family; needs a vault; choices: the resolved scopes (`Search Vault: <scope>`) |
| `links:open` | Links… | the `links` overlay |
| `link:<target>` | Open a link from Settings → Links | family; choices: Now then More links; opens only a configured target |
| `jira:open` | Open Jira Issue… | the `jira` overlay |
| `jira:issue:<key>` | Open a Jira issue | family; no choices |
| `projects:open` | Projects… | the `projects` overlay |
| `projects:open:<id>` | Show a project in Project Launcher | family; choices: the current projects' Launcher projects |
| `music:open`, `music:toggle`, `music:play`, `music:pause` | Music… and its controls | [music.md](music.md#commands) |
| `music:play:<entry>`, `music:volume:<n>` | Play an entry, set the volume | families; choices: the entries, volume steps |
| `rail:toggle` | Show or Hide the Rail | pins the rail on the pointer's screen, or hides a pinned one ([windows.md](windows.md#the-rail)) |
| `timetracker:show`, `timetracker:show:<day>` | Open in TimeTracker | `timetracker show [day]` ([timetracker.md](timetracker.md)) |
| `activity:switch` | Log Activity… | the `activity` overlay (⌘F2) |
| `activity:prefill:<…>` | Log Activity… with this filled in | family; choices: recent timers |
| `checkin:pause` | Pause Check-ins Today | toggles today's pause |
| `status:working`, `status:break`, `status:off` | Change Status to … | needs a vault; the status overlay with that status picked (`status/`) |
| `status:toggle` | Change Status… | needs a vault; Break picked while Working, else Working |
| `meetings:check` | Check for a Call Now | checks now; a call that is gone ends at once ([presence-and-calls.md](presence-and-calls.md#calls)) |
| `sample:open`, `sample:sticky` | Sample Overlay (Sticky) | debug builds only |

## Tools and their choices

`tools/` is pure apart from `snapshot`. A `ToolState` holds everything the tools read: the vault and
today's note, the contexts and the current one with its features, the status, journal and routine progress,
links (matching now and the rest), projects, the call, music, search scopes, today's topics and tickets.
`tools::snapshot(app)` builds it from the running app; each feature fills its part with its step (for now
the vault, today's note, the contexts with the current one and its features, the current projects' Launcher
projects, the status, and the running call with whether detection runs).

- `tool_choices(ToolId, &ToolState)` lists what a tool lets you pick: `Choice::Item` (label, command,
  detail, `checked` for a pick among several), `Heading`, `Separator` and `Steps` (a scale such as the
  volume). Contexts (`mode:<id>`, with two or more), statuses (with the Status feature), journal groups with
  their progress then routines, search scopes, Jira tickets (with Jira), topics, links (Now and More headings
  when both exist), Launcher projects (headings with several projects), music (play or pause, entries, the
  volume steps). `choice_commands` lists the commands of a list; `link_command` and `link_target` encode a
  link's target in its id.
- `visible_tools(tools, state, Surface)` is what the rail or the Daybook window shows: tools switched on for
  the surface that have something to offer, separators tidied.

## Menu-bar menu

`menu/model.rs` builds the menu as `MenuEntry` data (`Command` with an optional shown shortcut and
`enabled`, `Radio`, `Info`, `Submenu`, `Separator`); `tray.rs` turns it into native items whose ids are the
command ids.

- `build_menu(MenuInput { state, hotkeys, tools })`: without a vault only Choose Vault…, Settings… and Quit.
  Otherwise Create Today's Note (with "N entries waiting for today's note") while the note is missing, the
  tools in Settings → General → Tools order (a tool with choices is a submenu: its overlay command first, then its
  choices), the link menu (at most ten, the rest under More…, then More Links), Open Today's Note,
  Settings…, Quit Daybook. Separators never lead, trail or repeat; empty submenus go.
- `prune(entries, &dyn CommandStatus)` leaves out what runs an unknown command, greys disabled ones and drops
  submenus with nothing left to run, so a tool appears once its step registers its commands.
- `menu_bar_title(state, now)`: the current context's label, then the status (`Working`, `Break · 12m`) in a context with the `status` feature; empty (icon only) without a vault.
- `tray::refresh(app)` rebuilds the title and menu when they changed: after a settings save, a note event,
  a status change, every minute (`status::start`) and a menu click (macOS ticks a radio item itself).

## App menu

`menu/mod.rs` `build`: Daybook (About, Settings… ⌘,, Services, Hide, Hide Others, Show All, Close Window
⌘Q), File (Command Palette… ⌘K, Search Vault… ⌘F, Journal… ⌘J, Daybook ⌘0, Close Window ⌘W), Edit, Window
and Help (Open Log Folder, README; the sample overlays in debug builds). Items run bus commands and are left
out while their command is unknown, greyed while it is disabled. Their keys come from `MENU_KEYS` in
`menu/command_shortcuts.rs`, in Settings notation, through `shortcuts::accelerator`. ⌘Q only closes the
window in front; quitting is in the menu-bar menu. While a shortcut is recorded (`shortcuts_pause`) the menu
is only Daybook → Settings…, without keys, since native items always carry theirs. `menu::clicked` runs any
menu item's command.

## Global shortcuts

`shortcuts/`: the hotkeys in Settings (`Hotkeys`, `ctrl+alt+cmd+a` notation) mapped to commands by
`hotkey_commands`, converted by `accelerator` (`shift+cmd+k` → `Shift+Command+K`), registered through
`tauri-plugin-global-shortcut` by `apply` (again after a save that changed them); a shortcut whose command no
feature registered is skipped. `shortcuts_pause(paused)` takes them away while Settings records one.
Release builds register none while the prototype runs (`REGISTERED`,
[ADR 0018](adr/0018-global-shortcuts-only-in-debug-builds.md)); `shortcuts::shown` is what menus and the Daybook window's sidebar show next
to their items, so a release build shows no hints. `HOTKEY_ACTIONS` pairs each `Hotkeys` key with its command;
`shortcut_overview` (`shortcuts/overview.rs`) gives Settings → Shortcuts the ones whose command exists, with
the bus's title and help, plus the app menu's keys.

## Palette

The palette is the overlay view `palette` (`src/windows/panel/PaletteView.tsx`); `palette/` in Rust lists,
ranks and runs.

- `palette_entries(state, hotkeys, commands)`: the menu-bar menu built with every tool, pruned, flattened
  (`PaletteEntry`: kind, label, detail, shortcut, command), plus direct commands (the rail, a call check,
  the CLI install, the log folder, a "Settings: <Title>" entry per Settings page and per former page in `SECTIONS`, the sample overlays in
  debug builds). A command reads as `Music: Pause`
  (its submenu path, without overflow submenus or a repeat of the label); links, projects, music and contexts
  are named by themselves, with the link's group and "Current" as detail. Each command once; not the status
  toggle or the palette itself; nothing without a vault.
- `search(entries, typed)`: a first character `>` `#` `/` `:` `@` narrows to commands, links, projects,
  music or contexts; groups in that order, only non-empty ones; 6 per group with nothing typed, 8 when
  searching, 200 with a prefix; ranked by `rank_match`: the label starting with the query, then a word of the
  label, then every query word in label or detail, then the letters in order.
- `palette_query(query)` returns `PaletteResults` (prefix, prefixes, groups); `palette_run(command)`: a command
  that brings a window or another app forward (`hub:`, `app:settings`, `settings:`, `daily:`, quick notes,
  Jira issues, links) closes the palette first and leaves focus there; anything else runs, and the palette
  closes unless the command opened another overlay.

## Adding commands

1. In the feature's module, a `register(bus)` that adds its commands and families; call it from
   `app.rs` `commands_bus`.
2. If the menu-bar menu or palette should offer it, fill the feature's part of `ToolState` in
   `tools::snapshot`; the tool's entries in `menu/model.rs` already name the ids the prototype used.
3. A key in the app menu goes into `MENU_KEYS` and `menu::build`; a global shortcut into `hotkey_commands`.
