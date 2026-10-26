# Backend (Rust core)

This page covers `src-tauri/`, the Rust side of Daybook. The crate is `daybook`, the library `daybook_lib`,
the binary `daybook`. There is no database: the data is small JSON files in the config dir ([ADR 0012](adr/0012-vault-and-json-no-database.md)).

## Startup

`main.rs` calls `daybook_lib::run()` (`lib.rs`):

1. **Config dir.** `config_dir()` resolves `~/Library/Application Support/<IDENTIFIER>`
   (`com.philippstracker.daybook`; no home dir: exit 2) and `create_private_dir` (`store/json_file.rs`) makes
   it mode 0700. This happens before Tauri starts, because the CLI and the socket need the path first. Debug
   builds use `com.philippstracker.daybook.debug` (as `tauri.debug.conf.json`), so the debug app has its own
   folder, socket, lock, settings and logs and never touches the release app's
   ([ADR 0014](adr/0014-debug-build-has-its-own-data.md)). `APP_NAME` is `Daybook`, or `Daybook (Debug)` in debug
   builds; the app menu and the tray tooltip use it.
2. **CLI.** `cli::parse(std::env::args_os())` returns:
   - `Exit { message }`: `--help` or `--version`; printed, exit 0.
   - `Run { hidden, log_level }`: becomes `IpcMessage::Activate { hidden }`.
   - `Remote { message, json, starts_app }`: a subcommand; `instance::ask` sends it to the running app and
     exits with the code it returns. With no app running, a subcommand with `starts_app` (`show`, `focus`)
     continues with its message as the launch message; any other exits 2 with `Daybook is not running`.
   - an error: printed to stderr. With `in_app` it is turned into `IpcMessage::Error`, the exit code after
     forwarding is 2, and a newly started app shows it; without (a subcommand's error) the process exits 2.
3. **Single-instance handoff.** `instance::forward_or_lock` (`instance.rs`) sends the message with
   `ipc::try_forward`. `Delivered` exits; `NoInstance` continues. Then it takes `daybook.lock` in the config
   dir (`File::try_lock`), held for the app's lifetime; if another process holds it, it retries the socket
   25 × 200 ms while that process starts up, then fails with `Another Daybook is starting but does not
   answer`.
4. **`AppState`.** `AppState::open(&config_dir)` opens every JSON store. The log level is the CLI
   `--log-level`, else Info (`LogLevel::default`), so logging never starts at `Trace`.
5. **`app::start`** (`app.rs`) builds Tauri with `.manage(state)` first, then `remote::Bridge`, the overlay's
   `Panel`, the command bus (`bus::Commands`, filled by `commands_bus`), `shortcuts::Pause` and `tray::Shown`:
   - plugins: `log_plugin()` (stdout plus a rotating file in the log dir, 3 × 5 MiB, local timestamps,
     `tao`/`wry` capped at Warn, `tauri_runtime_wry` at Info), `init_logging(level)` (sets the effective
     level right after the log plugin's setup, whose dispatch level caps every later change), opener,
     dialog, deep-link, global shortcut, notification, autostart (`MacosLauncher::LaunchAgent` with `--hidden`);
   - `on_menu_event` → `menu::clicked` (every item's id is a command id, [commands.md](commands.md));
   - `on_window_event`: `CloseRequested` on `main` prevents the close and hides the window.

   Then `setup` runs: it logs the version, sets the app menu (`menu::build`), queues the initial launch
   message (`LaunchQueue::push`), reports the prototype import (`report_import`: the log and a launch notice,
   see [Prototype import](#prototype-import)), calls `commands::apply_settings(app, None, &settings)`, installs the vault
   service, the tray (`tray::install`) and the global shortcuts (`shortcuts::apply`), builds the overlay, applies `DAYBOOK_WINDOW_FRAME` in debug builds (`apply_dev_frame`), binds and serves the
   socket (`start_ipc`; a bind error is logged and the app runs without handoff), registers
   `deep_link::listen`, and shows the window in front (`window::show_main`) unless started with
   `--hidden`, which hides the Dock icon instead (`window::hide_main`). The window is declared
   `"visible": false`, so a hidden start never flashes it.

   The run loop prevents `RunEvent::ExitRequested { code: None }`, which macOS sends when the last window
   closes; an explicit `app.exit(0)` carries a code and passes. On `RunEvent::Reopen` (a Dock icon click, or
   `open` on the running app) it calls `window::show_main`.

### Settings side effects

`update_settings` and startup both call `commands::apply_settings(app, previous, saved)`. The decision is the
pure `settings_effects(previous, saved) -> SettingsEffects { launch_at_login }`: without `previous`
everything applies; otherwise only what changed (`changed`). `launch_at_login` goes to `autostart::apply`
(below), which changes the LaunchAgent only when its state differs; at startup this reconciles a LaunchAgent
that went missing while the setting says on. A failure is logged. Appearance and text size have no effect
outside the store; the page applies them.

### Launch at login

`autostart.rs` wraps `tauri-plugin-autostart` (`MacosLauncher::LaunchAgent`, started with `--hidden`; the
agent lives in `~/Library/LaunchAgents`). The LaunchAgent is the truth and `settings.launchAtLogin` mirrors
it: `status(app)` asks the plugin, `apply(app, enabled)` installs or removes the agent (`io` on failure), and
`set(app, state, enabled)` applies, then saves the setting through `commands::save_settings` and returns
`status`. The agent records the bundle path at enable time: after moving the app, turn it off and on again.

A login launch runs `daybook --hidden`: the window stays invisible and the tray icon is the only sign the app
runs; a later launch or the tray shows it.

## Window lifecycle

The main window is declared in `tauri.conf.json`: label `main`, title Daybook, 1200 × 800 (minimum 900 ×
560), invisible until setup shows it, overlay title bar with hidden title, traffic lights at 20/28,
transparent background with the `sidebar` vibrancy effect (`followsWindowActiveState`). The overlay panel
(`window/panel/`) is built at setup, the Settings window (`window/settings.rs`) on first use; all three share the
capability `capabilities/default.json`. Window behaviour,
the overlay and focus hand-back: [windows.md](windows.md).

| Event | Result |
|---|---|
| close button / ⌘W and ⌘Q (`window:close`, `window::close_focused`), Esc in a window (`window.close`) | the window in front hides (`window::close_requested`); with the last regular window the Dock icon goes and an active Daybook hides, so macOS picks what comes forward; the tray keeps running. With the overlay in front it closes instead |
| `app:settings`, `settings:<page>`, the sidebar gear, Choose Vault… | `window::settings::open`: the Settings window on the pointer's screen, at the page, in front; the Dock icon shows |
| last window closed (`ExitRequested { code: None }`) | prevented |
| "Open Daybook" (`hub:open`), a second `daybook` launch, `daybook focus`, Dock icon click (`RunEvent::Reopen`) | `window::show_main`: the Dock icon returns, the window moves to the pointer's screen, show, unminimize, focus |
| `daybook show` | `window::show_main_in_background`: shown without taking focus |
| `daybook --hidden` (launch at login) | first instance: the window is never shown and the Dock icon is hidden; running instance: nothing |
| `hub:toggle` (a global shortcut) | hides the window when it is in front, else `window::show_main` |
| "Quit Daybook" (`app:quit`, no key) in the menu-bar menu and the palette | `app.exit(0)` |

`show_main_in_background` emits `window_opened` when the window was hidden before (`main_visible`).
`window/mod.rs` never sets the window theme, so its native appearance follows macOS (the page draws every surface
opaquely). `bring_to_front` does nothing with `DAYBOOK_BACKGROUND=1` in debug builds.

## AppState

`state.rs`. Tauri manages it; every command receives it as `State<'_, AppState>`. Each store locks inside
itself. Panics unwind ([ADR 0005](adr/0005-panic-unwind.md)), so code takes `std::sync` locks with `lock::MutexExt::lock_unpoisoned` and
`lock::RwLockExt::{read_unpoisoned, write_unpoisoned}` (`lock.rs`).

| Field | Type | Holds |
|---|---|---|
| `settings` | `SettingsStore` | `settings.json` |
| `keybindings` | `KeybindingsStore` | `keybindings.json`, the page's key overrides |
| `ui_state` | `UiStateStore` | `ui_state.json` |
| `launches` | `LaunchQueue` | launch events the page has not taken yet |
| `notes` | `Arc<DailyNotes>` | the vault service: `pending-entries.json`, the writer, the watcher ([vault.md](vault.md)) |
| `contexts` | `Contexts` | `context.json`, the current context, the work-hours check |
| `statuses` | `Statuses` | `status-memory.json`, today's status ([status.md](status.md)) |
| `slack` | `SlackFiles` | `slack-shown.json`, `day-message.json` ([slack.md](slack.md)) |

Managed beside it: `remote::Bridge` (requests waiting for the page), `window::panel::Panel` (the overlay),
`bus::Commands` (the command bus), `calls::Calls`, `presence::Presence`, `slack::Slack` (the Slack client, token and live state), `shortcuts::Pause` and `tray::Shown` (what the menu-bar item shows).

## Tauri commands

Registered in `handlers.rs` (`handlers()`, one `generate_handler!` list; the dev commands carry
`#[cfg(debug_assertions)]`). Commands stay thin; logic lives in
its module.

| Command | Args | Returns | Errors |
|---|---|---|---|
| `get_settings` | – | `Settings` | – |
| `update_settings` | `settings: Settings`; `launchAtLogin` is kept as stored (only `autostart_set` changes it) | `Settings` (repaired by `from_json`); emits `settings_changed` | `io` |
| `get_keybindings` / `set_keybindings` | – / `bindings: Keybindings` | `Keybindings` (sanitized); `set_keybindings` emits `keybindings_changed` | `io` |
| `get_ui_state` / `set_ui_state` | – / `ui: UiState` | `UiState` | `io` |
| `autostart_set` (`autostart.rs`) | `enabled: bool` | `bool`: whether the LaunchAgent is installed now; also saves `settings.launchAtLogin` | `io` |
| `autostart_status` (`autostart.rs`) | – | `bool`: whether the LaunchAgent is installed | – |
| `open_log_folder` | – | – | `io` |
| `open_url` | `url: String` | – | `invalid_input` unless `http(s)` without credentials (`is_web_url`) |
| `log_frontend_error` | `message: String` | – | – |
| `take_launch_events` | – | `LaunchEvent[]` (`error`, `notice`, `activate`) | – |
| `choose_folder` | – | `string \| null` (the folder panel, which can create a folder; `null` when cancelled) | `io` |
| `reveal_path` | `path: String` | – (Finder shows the enclosing folder with `path` selected) | `io` |
| `context_state` (`contexts/mod.rs`) | – | `ContextState` | – |
| `hub_view` (`hub/mod.rs`) | `day: string \| null` (`YYYY-MM-DD`, today when null) | `HubView`: the day, its weekday and date, the board's title (a greeting today), the closest days with a note (null until their step), the sidebar's tool rows, today's status, the reminder (today only) and the day's events ([calendar.md](calendar.md#the-day-model)) | `invalid_input` for a malformed day |
| `calendar_info`, `calendar_request_access`, `calendar_join` (`calendar/mod.rs`) | – / – / `url` | `CalendarInfo` / `CalendarAccess` / – ([calendar.md](calendar.md)); `calendar_join` joins through `calls::join` ([presence-and-calls.md](presence-and-calls.md#joining)) | `calendar_request_access`: `io` when the privacy settings don't open; `calendar_join`: `invalid_input` unless a join link |
| `slack_info`, `slack_connect`, `slack_test`, `slack_disconnect` (`slack/commands.rs`) | – / `token` / – / – | `SlackInfo` / `SlackIdentity` / `SlackIdentity` / – ([slack.md](slack.md#commands)) | `slack_connect`: `invalid_input`, `network`, `io`; `slack_test`: `not_found`, `network` |
| `calls_info`, `calls_request_access`, `calls_pick_app` (`calls/mod.rs`) | – / `permission` / – | `CallsInfo` / whether Accessibility is allowed / `AppRef \| null` ([presence-and-calls.md](presence-and-calls.md#commands)) | `calls_request_access`: `io`; `calls_pick_app`: `invalid_input` for anything but an app |
| `shortcut_overview` (`shortcuts/overview.rs`) | – | `ShortcutOverview`: whether this build registers global shortcuts, the global shortcuts of the commands that exist (action, command, title, help), the app menu's keys | – |
| `run_command` (`bus/app.rs`) | `id: String` | – (runs a bus command, [commands.md](commands.md)) | `not_found` (unknown), `invalid_input` (disabled), the command's error |
| `palette_query`, `palette_run` (`palette/mod.rs`) | `query` / `command` | `PaletteResults` / – ([commands.md](commands.md#palette)) | `palette_run`: `not_found` |
| `shortcuts_pause` (`shortcuts/mod.rs`) | `paused: bool` | – (global shortcuts and the app menu's keys step aside) | – |
| `remote_reply` (`remote.rs`) | `id, result, error` | – | – |
| `daily_note_state`, `daily_note_open`, `daily_note_create`, `vault_info` (`vault/commands.rs`) | see [vault.md](vault.md#commands) | | |
| `status_set` (`status/mod.rs`) | `status, message, endOfDay: "done" \| "incomplete" \| null, endOfDayList: string \| null` | – (posts the end-of-day list going Off, sets today's status, closes the status overlay) | `io`, `network` |
| `focus_info`, `focus_save`, `focus_send` (`focus/mod.rs`) | – / `body` / `body` | `FocusInfo` / – / – ([slack.md](slack.md#the-days-message)) | `invalid_input` (over 8000 characters; `focus_send` when not shared or already sent), `not_found` (Slack not connected), `network`, `io` |
| `panel_open`, `panel_close`, `panel_session`, `panel_ready`, `panel_painted`, `panel_set_height`, `panel_keep_open` (`window/panel/commands.rs`) | see [windows.md](windows.md#commands-and-events) | | |
| `dev_edit_today_note` (`dev.rs`, debug builds only) | `op: NoteOp` | `WriteOutcome` | as `edit_today` |

**Adding a command:** write it in its module (or `commands.rs` for app-wide ones), register it in
`handlers.rs`, export its types with ts-rs (`#[ts(export, export_to = "src/api/types.gen.ts")]`, then
`bun run types`; [ADR 0004](adr/0004-generated-ipc-types.md)), add the wrapper to `src/api/index.ts` ([ADR 0002](adr/0002-ipc-in-api-files.md)) and a default to
`src/test/apiMock.ts`.

## Events to the page

`events.rs` names every event, so all emitting goes through one place.

| Constant | Name | Payload |
|---|---|---|
| `LAUNCH_EVENT` | `launch` | none; the page calls `take_launch_events` |
| `REMOTE_EVENT` | `remote` | `RemoteEvent { id, request }`, answered with `remote_reply` |
| `WINDOW_OPENED_EVENT` | `window_opened` | none; `window::show_main_in_background` showed the hidden window |
| `PANEL_SESSION_EVENT` | `panel_session` | to the overlay window: the `PanelSession` to render, `null` once it closed ([windows.md](windows.md#the-overlay)) |
| `NOTE_CHANGED_EVENT` | `note_changed` | `NoteChanged { day }`: today's note, the waiting changes or another daily note changed ([vault.md](vault.md#watcher)) |
| `CONTEXT_CHANGED_EVENT` | `context_changed` | `ContextChanged { id }`: another context became current |
| `CALENDAR_CHANGED_EVENT` | `calendar_changed` | none; an event, a calendar or the calendar access changed ([calendar.md](calendar.md#reading-events)) |
| `SLACK_CHANGED_EVENT` | `slack_changed` | none; what Slack shows or the connection changed ([slack.md](slack.md)) |
| `CALL_CHANGED_EVENT` | `call_changed` | none; a call started, changed or ended ([presence-and-calls.md](presence-and-calls.md#calls)) |
| `STATUS_CHANGED_EVENT` | `status_changed` | `StatusChanged`: today's status changed; Rust features listen too ([status.md](status.md#status_changed)) |
| `OPEN_SETTINGS_EVENT` | `open_settings` | to the Settings window: the id of the page to show (a window built for a page reads it from `?page=` instead) |
| `TOGGLE_INSPECTOR_EVENT` | `toggle_inspector` | to the Daybook window: none; shows or hides the inspector (`view:toggleInspector`) |
| `KEYBINDINGS_CHANGED_EVENT` | `keybindings_changed` | the saved `Keybindings`, so the other window takes them |
| `SETTINGS_CHANGED_EVENT` | `settings_changed` | the saved `Settings`, after every `update_settings` and `autostart_set` |

## Launch queue

`launch.rs`. A launch message may arrive before the page listens (the launch that starts the app, a deep
link that starts it), so it becomes a `LaunchEvent` in `AppState.launches` and the page takes them with
`take_launch_events` once it has loaded, and again on every `launch` event.

- `LaunchEvent` (tagged by `type`): `error { message }`, `notice { message }` (something that went fine and
  the user should know; `LaunchQueue::notice`, no caller yet) and `activate`.
- `LaunchQueue::push(message)`: `Error` becomes `error`; `Activate { hidden: false }` and `Show { .. }`
  become `activate`; anything else, a hidden activation included, produces no event (`false`).
- `launch::queue(app, message)` pushes, emits `launch` when an event was queued, and shows the window
  (`window::show_main`) unless the message is `Activate { hidden: true }`.

## CLI and single-instance handoff

The `daybook` command is the app binary. Without a subcommand it starts the app, or activates the running
one; every subcommand is a second process that sends one request over the socket, prints the reply and exits
([ADR 0008](adr/0008-cli-asks-the-running-app.md)).

### Command tree

`cli/mod.rs` (clap derive; `Cli`, `Command`, `parse` and `message`). Flags: `--hidden` (start without the
window; a running instance stays as it is), `--log-level <error|warn|info|debug|trace>` (`LogLevel`, for this
run), `--json` (global: replies as JSON), `--help` (with examples, `EXAMPLES`), `--version`.

| Subcommand | `IpcMessage` | Prints |
|---|---|---|
| `show` | `show { focus: false }`; starts the app when none runs | – |
| `focus` | `activate { hidden: false }`; starts the app when none runs | – |
| `hide`, `quit` | `hide`, `quit` | – |
| `status` | `status` | two lines, below |

`parse(args) -> Result<CliOutcome, CliError>` is pure: `CliOutcome` is `Run { hidden, log_level }`,
`Exit { message }` (help, version) or `Remote { message, json, starts_app }`. `CliError.in_app` is true only
when every argument is a flag or a flag's value (`names_command`): a plain launch with bad flags also shows the
error in the app, which may have been opened from Finder. An error with a word in it, a subcommand known or
mistyped (`daybook stauts`), only prints and exits 2 without starting the app.

`cli/reply.rs`: `render(message, result)` is the human text, `None` for subcommands that print nothing; a
reply of the wrong shape is `The running Daybook sent an unexpected reply: …`. `status` prints one fact per
line, so scripts can grep:

```
Daybook 0.1.0
window: hidden          (window: visible)
```

`--json` prints the reply's `result` instead (`status --json` is a `StatusReport { version, windowVisible }`;
a `null` result prints nothing); an error prints `{ "kind", "message" }` (`IpcError`) on stdout, else the
message on stderr.

**Exit codes:** 0 ok; 1 the app answered with an error; 2 bad arguments, Daybook is not running, or the
running app could not be reached.

**Adding a subcommand:** a `Command` variant and its arm in `cli::message` (and in `starts_app` if it may
start the app), an `IpcMessage` variant with its `kind`, its arm in `remote::handle`, a `reply::render` arm
when it prints something, and parser tests.

### Socket and protocol

`ipc.rs`:

- **Socket:** a Unix domain socket at `<config dir>/daybook.sock`, mode 0600 in a 0700 dir. `serve` drops
  peers with another uid (`same_user`, `libc::getuid`).
- **Protocol:** one line of JSON per connection, at most 64 KiB (`MAX_MESSAGE_BYTES`), within 5 s
  (`CLIENT_TIMEOUT`). Any process of the same user may speak it:
  - one-way (a bare `IpcMessage`): answered with `ok\n` before it is handled (`try_forward`);
  - request (`IpcRequest { id, message }`, sent by `ipc::request` with a fresh uuid): answered once handled
    with one `IpcReply { id, ok, result, error }` line, `error` being `IpcError { kind, message }` (`kind` as
    in [Errors](#errors)); `IpcReply::into_result` turns it into a `HandlerResult`. A reply with another id
    is an error (the running app may be older than the command).

  ```
  → {"id":"7f0c…","message":{"type":"status"}}
  ← {"id":"7f0c…","ok":true,"result":{"version":"0.1.0","windowVisible":false},"error":null}
  ```

  Malformed, oversized or incomplete lines get no answer.
- **`Forward` outcomes** of `try_forward`: `Delivered` (exit 0, 2 for a CLI error), `NoInstance` (socket
  missing or refused: start the app), `Rejected` (no `ok`) or an error such as a timeout (print and exit 2,
  so a hung instance never leads to a second one).
- **Messages** (`IpcMessage`, tagged by `type`, camelCase): `activate { hidden }`, `error { message }`,
  `show { focus }`, `hide`, `quit`, `status`, and `eval { js }` (debug builds only, sent by
  `scripts/dev/js.sh`). `IpcMessage::kind` names each for the log.
- **Stale sockets:** `bind` removes a leftover socket only when connecting to it is refused; it refuses to
  bind while a listener is alive.

### Dispatch

`remote::handle(app, message)` maps each message to the module that does it; it is the one table to extend.
Every message is logged by type (`IpcMessage::kind`, never the contents).

| Message | Does | Result |
|---|---|---|
| `activate`, `error` | `launch::queue` ([Launch queue](#launch-queue)) | `null` |
| `show { focus }` | `window::show_main`, or `show_main_in_background` without `focus` | `null` |
| `hide` | `window::hide_main` | `null` |
| `quit` | `app.exit(0)` 200 ms after replying (`QUIT_DELAY`), so the reply reaches the sender | `null` |
| `status` | `remote::status_report` | `StatusReport` |
| `eval { js }` | `dev::eval` ([Dev hooks](#dev-hooks)) | `null` |

A message that starts the app is pushed to `LaunchQueue` in `setup` instead, so `show` and `focus` reach the
page as `activate` once it has loaded.

### Requests the page answers

`remote.rs`: `Bridge::ask(app, request)` emits `remote` with a fresh id (`pending::PendingReplies`) and waits
up to 4 s (`REPLY_TIMEOUT`, below the IPC client's 5 s) for `remote_reply`; no window, a dropped request and
a timeout each become an `io` error naming it. `remote_reply` with `error` set is a failure; a late or
unknown id is logged at debug. `RemoteRequest` (tagged by `type`) has no variants yet; later steps add them.
`src/commands/remote.ts` (`answerRemote`) always replies, with an error for a request it does not know.

`pending.rs` is generic: `PendingReplies<T>::ask(timeout, send)` hands `send` the id and waits;
`answer(id, reply)` is `false` when nothing waits for that id. `NoReply` is `NotSent(e)`, `Dropped` or
`TimedOut`.

### Deep links

`deep_link.rs`: `listen` registers `on_open_url` for `daybook://` (scheme in `tauri.conf.json`
`plugins.deep-link`). macOS delivers a link to the running app, or starts the app and then delivers it.
`link_message(url)` maps a link to a message:

| Link | Message |
|---|---|
| `daybook://show` (a trailing `/` and a query are ignored) | `show { focus: true }` |

Every link goes through `launch::queue`, so a link that starts the app waits for the page. A link that does
not parse or names nothing becomes `IpcMessage::Error` (`A daybook:// link failed: Daybook has no such
link`): the window shows a toast.

## Contexts and projects

**Contexts** (`contexts/`). The contexts themselves are in the settings; `context.json` (`Active { id, setAt }`,
imported from the prototype's `mode.json`) says which is current. `contexts::current` is the stored one, else the
first; `Contexts` (in `AppState`) reads and sets it and gives the current context's features.

- **Switching.** `mode:<id>` (a family, needs a vault) switches by hand and closes the switcher if it made the
  switch; `context:switch` opens the switcher overlay (`context` view). A switch saves `context.json`, logs,
  emits `context_changed` (`ContextChanged { id }`) and refreshes the menu-bar item, whose title starts with
  the current context's emoji. Leaving the status behind while Working offers Off instead of closing the
  switcher ([status.md](status.md#offer-off)).
- **Work hours.** `contexts::start` checks every 30 s. `AutoSwitch::check` (`contexts/schedule.rs`, pure)
  switches to the first context with auto-switch whose work hours contain now, when they start; a switch by
  hand holds until the next start, and nothing happens when they end. On the first check after launch, hours
  that started after the last switch still win. Hours whose end is before their start run past midnight. An
  automatic switch shows a notification ("Switched to Nexo", "Was Personal.").
- `context_state` returns the switcher's `ContextState { contexts: [{ id, title, color }], current }`;
  `context_title` is "🐈 Personal".

**Projects** (`projects/`, pure). `current_projects(projects, context)`: the active projects with no contexts or
with the current one. It is the only source for offering, picking or matching a project. `meeting_project(title,
projects)`: the first whose name or meeting word is in the title, ignoring case. `project_groups` lists their
Launcher projects (the Projects tool), the current project first. `follow_projects` runs in `update_settings`:
links bound to a project follow its rename in place, and a removed project is dropped from them.

## Tray

`tray.rs`: `tray::install(app)` builds the menu bar item `main` from `icons/tray/tray.png` (36 px, the menu bar's
18 pt icon height) as a template image (macOS tints it for the menu bar), tooltip `APP_NAME`, menu on click.
Its title and menu come from `menu::model` and are rebuilt by `tray::refresh` when they changed; item ids are
command ids, so clicks run through `menu::clicked`. The menu's contents: [commands.md](commands.md#menu-bar-menu).

## Persistence

Everything lives in the config dir, `~/Library/Application Support/com.philippstracker.daybook/` (debug
builds: `….daybook.debug/`). JSON
stores wrap a `JsonStore<T>` (`store/json_store.rs`): the parsed value behind an `RwLock`, with
`read(|t| ..)`, `update` and `update_async(|t| ..) -> AppResult<R>`. `update_async` runs on a blocking
thread: under a write gate it applies the closure to a clone, saves the clone and only then swaps it in, so a
failed save leaves memory unchanged. `JsonStore::open_with` normalizes the loaded value (settings use
`Settings::sanitized`, keybindings `keybindings::sanitize`). The file itself is `JsonFile`
(`store/json_file.rs`), which:

- writes pretty-printed JSON to `<name>.tmp` (mode 0600), `sync_all`s it and renames it over the target;
- loads defaults when a file is missing;
- moves a file it cannot read or parse aside to `<name>.<UTC yyyymmddTHHMMSS>.corrupt` (with a counter when
  taken) and logs a warning. If that rename fails, the file is poisoned: `save` refuses with `AppError::Io`
  instead of overwriting it.

| File | Contents |
|---|---|
| `settings.json` | `Settings` ([Settings](#settings)) |
| `keybindings.json` | `{ "commandId": "Mod+Shift+X" \| null }` overrides; `null` unbinds ([Key bindings](#key-bindings)) |
| `ui_state.json` | `UiState` (`store/ui_state.rs`): `sidebarHidden`, `inspectorHidden`, `sidebarWidth` (px, default 200), `paneSizes` (splitter positions in px by pane id); missing keys take the defaults, unknown keys are ignored |
| `daybook.sock`, `daybook.lock` | single instance |
| `context.json` | `Active { id, setAt }`, the current context (`contexts::Contexts`) |
| `status-memory.json` | `StatusMemory { date, status, since, workedToday }`, today's status (`status::Statuses`, [status.md](status.md)) |
| `jira-tickets.json`, `routines.json`, `day-message.json`, `slack-shown.json` | written only by the [prototype import](#prototype-import) so far; their steps add the stores that read them |

### Settings

`store/settings/` holds Daybook's whole configuration as one model
([ADR 0013](adr/0013-one-settings-model-imported-once.md)): `mod.rs` (`Settings`, `from_json`, `sanitized`),
`store.rs` (`SettingsStore`, `SETTINGS_FILE`), `read.rs` (readers for untrusted JSON, the port of the
prototype's `config/primitives.ts`), `defaults.rs` (the starting contexts, projects, journal groups,
routines and reminders) and one module per group:

| Key | Module, type | What |
|---|---|---|
| `lightScheme`, `darkScheme`, `accent`, `textSize`, `launchAtLogin` | `types.rs` | appearance and launch at login (below) |
| `vault` | `vault.rs`, `Vault` | `path`, `name` (for the Obsidian CLI), `obsidianCliPath` |
| `dailyNote` | `vault.rs`, `DailyNote` | fallback folder and format, the notes heading, the journal, focus and plan sections (`SectionPlacement`), their `order` |
| `search` | `vault.rs`, `Search` | named `scopes` and `exclude`d folders |
| `statusMessages`, `slack`, `endOfDay`, `focus`, `statusRules` | `slack.rs` | channel messages per status, the Slack status per status (`StatusMap<SlackStatus>`) and `manualWorkflows`, the end-of-day check, the focus list, absence heads-ups |
| `meetings`, `joining` | `calls.rs` | call `detection` and its app `rules` (built in: Zoom, Teams, Meet), what joining starts and opens |
| `projects`, `contexts` | `contexts.rs` | `Project` (Slack emoji, active, contexts, meeting keywords, `launcherProjects`), `Context` (features, colour, work hours, auto-switch, calendars, check-in minutes, `ContextMusic`) |
| `music` | `music.rs`, `MusicEntry` | what the Music tool plays |
| `journal`, `routines`, `reminders` | `workflows.rs` | journal groups, routines (both with `Timing`), reminder rules |
| `links`, `jira` | `links.rs` | quick links with their rules, the Jira base URL and prefix |
| `rail` | `rail.rs`, `Rail` | the rail's edges, look, delays and tooltips |
| `tools`, `hotkeys`, `api`, `timeline` | `shell.rs` | tool placement and order, global shortcuts (`ctrl+alt+cmd+a`; `""` is none), the HTTP API (off by default), the day timeline's hours |

Everything is camelCase. `Settings` derives `Serialize` and `TS` only; its `Deserialize` hands any JSON to
`Settings::from_json`, which ports the prototype's `config-normalize.ts`, `app-settings.ts` and
`rail-settings.ts` rule by rule: a part of the wrong type gets its default, a list keeps its valid items
(an empty list stays empty), text is trimmed, numbers are clamped or replaced, ids are made unique (`work`,
`work-2`), a context without a colour takes the next one of the palette, a context's start music must exist,
every routine and journal group has exactly one reminder rule, and a section or tool added later is placed
after the one it follows by default. Loading a file and every `update_settings` from the page take this one
path, so nothing the page or a hand edit sends can break the app, and only invalid JSON quarantines the file.
`sanitized()` repairs a value built in Rust the same way (a round trip through `from_json`). Unknown keys are
dropped on the next save. `SettingsStore::update` returns `(previous, saved)` from one lock; every save emits
`settings_changed` with the saved settings.

Appearance is one scheme per macOS mode ([ADR 0009](adr/0009-three-schemes-four-palettes.md),
[ADR 0010](adr/0010-a-scheme-per-system-mode.md)): `lightScheme`/`darkScheme` are `Scheme` (`default`, a dark
sidebar beside light content; `light`; `dark`; defaults `default` and `dark`), `accent` is `system` or one of
`ACCENTS` (else `system`), `textSize` is `small`, `medium` (default) or `large` (pixel sizes in the frontend's
`TEXT_SIZES`), `launchAtLogin` mirrors the LaunchAgent. `src/api/types.ts` mirrors the constants the page needs
(`ACCENTS`, `SYSTEM_ACCENT`, `TEXT_SIZES`); a Rust test (`accents_match_the_frontend_list`) compares `ACCENTS`
with it.

**Adding a setting:**
1. Add the field to its group's struct (or a new group module) with its default, and read it in the group's
   `read` with the helpers in `read.rs`; keep the prototype's rule when it has one.
2. If it changes something outside the store, add it to `SettingsEffects` and apply it in `apply_settings`.
3. Regenerate `src/api/types.gen.ts` (`bun run types`) and refresh `src/test/default-settings.json`
   (`UPDATE_FIXTURES=1` on the drift test); mirror any constant the page needs in `src/api/types.ts`.
4. Render it on its settings page.

### Prototype import

`store/import/` runs once, from `lib.rs` after the single-instance handoff and before the stores open:
`import::run(prototype_dir(), config_dir)` acts only while Daybook has no `settings.json` and the Glaze
prototype's data folder (`~/Library/Application Support/app.glaze.macos.2mykv1oc-local/`) has one, and it only
reads that folder. `settings.rs` maps the prototype's file into Daybook's shape (clients → `projects`, modes
→ `contexts`, the feature `tracking` → `status`, `modes`/`clients` in links, routines and projects →
`contexts`/`projects`, the activity format's status notes → `statusMessages`) and leaves every time-tracking
key behind (`DROPPED`: the activity section, the activity line formats, `statusLogLines`, project colours);
`Settings::from_json` then repairs the result. `state.rs` (`STATE_FILES`) copies the state files Daybook
doesn't have yet in Daybook's shapes: `mode.json` → `context.json` (`{ id, setAt }`), `status-memory.json`
(`{ date: "YYYY-MM-DD", status, since, workedToday }`), `jira-tickets.json` (day → keys), `routines.json`
(`{ date, fired }`), `day-message.json` (`{ date: "YYYY-MM-DD", channel, ts, greeting, eodSent }`),
`slack-shown.json` (`{ emoji, text, presence, expiresAt }`) and `pending-entries.json` without the note ops
that only wrote the activity log (`activity`, `status`, `meeting`). Settings are written last, so an import
that fails part-way runs again on the next launch. `Import::Imported(ImportReport)` is logged in full
(`log_text`) and shown once as a launch notice (`notice`: the counts, and that the Slack token must be entered
again, since the prototype's `slack-token.bin` is encrypted for it alone); `Import::Failed` is logged.

### Key bindings

`store/keybindings.rs`: the user's overrides of the page's keys (`src/commands`).

- `Keybindings` is a `BTreeMap<String, Option<String>>`. `KeybindingsStore::update` replaces all overrides.
- `sanitize` drops (and logs) entries whose id is not `[A-Za-z0-9.]+` or whose binding is malformed. Ids are
  not checked against the page's registry, which Rust does not know.
- A binding is modifiers from `Mod`, `Ctrl`, `Alt`, `Shift` and one key (one character or a name such as
  `Enter`), joined by `+`; a trailing `++` (or a bare `+`) is the `+` key (`split`).

## Native menu

`menu/mod.rs` builds the app menu from bus commands with the keys of `menu/command_shortcuts.rs`; items whose
command is unknown are left out. The menus, keys and clicks: [commands.md](commands.md#app-menu); the keys also
in [ux.md](ux.md#menu-bar).

- **README.** Help → README runs `app:readme` (`help::open_readme`): the `README.md` bundled as a resource
  (`bundle.resources` in `tauri.conf.json`), else in debug builds the repository's (`readme_path`), opened in
  the default app.

## Logging

`tauri-plugin-log` writes to stdout and to Tauri's `app_log_dir`, `~/Library/Logs/com.philippstracker.daybook/`,
rotating 3 files of 5 MiB, local timestamps. The level is Info, or `--log-level` for that run (`app::start`
passes it to `init_logging`); there is no setting. `open_log_folder` (Help → Open Log Folder) opens the
folder. Text from `log_frontend_error` (target `frontend`) is cut to 4 KiB on a character boundary and its
`\n`/`\r` escaped (`sanitize_log_text`), so the page cannot flood or forge log lines. Messages from other
processes are logged by kind only; a deep link is logged without its URL. Vault content and note text are
user data: log them at Debug at most.

## Errors

`error.rs`: `AppError` serializes through `ErrorJson` as `{ kind, message, field? }` ([ADR 0003](adr/0003-errors-as-kinds.md)):

- `message`: the `Display` text.
- `field`: `{ path, message }` (`FieldJson`) for `AppError::InvalidField` (kind `invalid_input`); `message`
  there is without the `` `path`: `` prefix.

| kind | Variant | Raised for |
|---|---|---|
| `io` | `Io` | file and system errors (`From<std::io::Error>`, message `File error: …`) |
| `invalid_input` | `InvalidInput`, `InvalidField { field, message }` | bad arguments (`From<serde_json::Error>` too); a form field |
| `not_found` | `NotFound` | something that names nothing (no README in this build) |
| `cancelled` | `Cancelled` | the user cancelled |

ts-rs generates `AppError`, `AppErrorField` and `ErrorKind` into `src/api/types.gen.ts`. Across the socket an
error travels as `IpcError { kind, message }` (`From<AppError>`).

## Dev hooks

`dev.rs` is compiled into debug builds only (`#[cfg(debug_assertions)]` on the module and its call sites), so
scripts in `scripts/dev/` can drive the app without disturbing the user:

- `window_frame()`: `DAYBOOK_WINDOW_FRAME="x,y,width,height"` (logical points) places the main window at
  startup (`app::apply_dev_frame`); anything but four finite numbers with a positive size is ignored.
- `background()`: `DAYBOOK_BACKGROUND=1` keeps `window::bring_to_front` from taking focus.
- `eval(app, js)`: runs `IpcMessage::Eval` (sent by `scripts/dev/js.sh`) in the main window, or in the window
  named by a first line `@<label>`.

`DAYBOOK_DEV_TERMINAL` (`scripts/dev/launch.sh`) and `DAYBOOK_SIGN_IDENTITY` (`scripts/sign-app.sh`) are read
by the scripts, not by the app.

## Tests

Unit tests sit in each module (`#[cfg(test)]`). `test_support/` (`temp_config_dir`: a fresh 0700 temp dir
standing in for the config dir) is compiled for tests and behind the `test-util` feature, which the crate's
dev-dependency on itself turns on for integration tests ([ADR 0006](adr/0006-test-doubles-behind-test-util.md)). See [testing.md](testing.md).

## Dependencies

| Crate | Use |
|---|---|
| `tauri` (features `macos-private-api`, `tray-icon`, `image-png`), `tauri-build` | app framework, transparent window, tray |
| `tauri-plugin-log`, `log` | logging |
| `tauri-plugin-opener` | web links, the log folder, the README, Finder reveal |
| `tauri-plugin-dialog` | the folder panel |
| `tauri-plugin-deep-link` | `daybook://` links |
| `tauri-plugin-autostart` | launch at login (LaunchAgent) |
| `tauri-plugin-global-shortcut` | global shortcuts ([commands.md](commands.md#global-shortcuts)) |
| `tauri-plugin-notification` | notifications (`notify.rs`): failed commands nobody waits for |
| `reqwest` (rustls, form) | the only HTTP client, in `net.rs` ([ADR 0020](adr/0020-network-only-to-listed-hosts.md)) |
| `security-framework`, `zeroize` | the Keychain and wiped secrets (`secrets.rs`) |
| `percent-encoding` | a link's target in its command id (`tools::link_command`) |
| `tokio` | runtime, IPC socket, timeouts |
| `serde`, `serde_json` (`preserve_order`) | IPC, stores |
| `clap` | CLI |
| `thiserror` | `AppError` |
| `chrono`, `uuid`, `url` | quarantine time stamps, request ids, links and `is_web_url` |
| `ts-rs` | TypeScript types for IPC |
| `libc` | `getuid` for the IPC peer check |
| `objc2`, `objc2-foundation`, `objc2-app-kit`, `objc2-core-foundation`, `objc2-core-graphics` | the overlay panel, screens, the pointer, the front app ([windows.md](windows.md#what-needed-objc2)) |
| `notify` | the daily-notes watcher ([vault.md](vault.md#watcher)) |
| `regex` | note edits ([vault.md](vault.md#sections)) |
| `tempfile` (dev, and optional behind `test-util`) | store, socket and README tests |

No database. The only network access is `net.rs`. The library builds as `rlib`. The release profile uses LTO,
`codegen-units = 1`, `opt-level = 3` and strip, and keeps `panic = "unwind"` ([ADR 0005](adr/0005-panic-unwind.md)). The toolchain is
pinned to stable in `rust-toolchain.toml`; edition 2024.

## Content security policy

`app.security.csp` in `tauri.conf.json`:

- `default-src 'self'`: scripts, fonts and frames come from the bundle only;
- `img-src 'self' data:`: no view loads a remote image;
- `style-src 'self' 'unsafe-inline'`: React `style` attributes;
- `connect-src ipc: http://ipc.localhost`: Tauri IPC only; Slack is called from Rust ([ADR 0020](adr/0020-network-only-to-listed-hosts.md)).

`app.security.devCsp` applies under `tauri dev` and adds inline scripts and Vite's HMR websocket on
`localhost:1420`/`1421`.
