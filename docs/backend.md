# Backend (Rust core)

This page covers `src-tauri/`, the Rust side of Daybook. The crate is `daybook`, the library `daybook_lib`,
the binary `daybook`. There is no database: the data is small JSON files in the config dir ([ADR 0012](adr/0012-vault-and-json-no-database.md)).

## Startup

`main.rs` calls `daybook_lib::run()` (`lib.rs`):

1. **Config dir.** `config_dir()` resolves `~/Library/Application Support/<IDENTIFIER>`
   (`com.philippstracker.daybook`; no home dir: exit 2) and `create_private_dir` (`store/json_file.rs`) makes
   it mode 0700. This happens before Tauri starts, because the CLI and the socket need the path first. The
   debug bundle has its own identifier (`com.philippstracker.daybook.debug` in `tauri.debug.conf.json`, so
   macOS grants it apart) but shares this folder. `APP_NAME` is `Daybook`, or `Daybook (Debug)` in debug
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
5. **`app::start`** (`app.rs`) builds Tauri with `.manage(state)` first, then `remote::Bridge` and
   `menu::MenuSync`:
   - plugins: `log_plugin()` (stdout plus a rotating file in the log dir, 3 × 5 MiB, local timestamps,
     `tao`/`wry` capped at Warn, `tauri_runtime_wry` at Info), `init_logging(level)` (sets the effective
     level right after the log plugin's setup, whose dispatch level caps every later change), opener,
     dialog, deep-link, autostart (`MacosLauncher::LaunchAgent` with `--hidden`);
   - `on_menu_event` → `menu::forward`;
   - `on_window_event`: `CloseRequested` on `main` prevents the close and hides the window.

   Then `setup` runs: it logs the version, sets the native menu (`menu::build`), queues the initial launch
   message (`LaunchQueue::push`), calls `commands::apply_settings(app, None, &settings)`, installs the tray
   (`tray::install`), applies `DAYBOOK_WINDOW_FRAME` in debug builds (`apply_dev_frame`), binds and serves the
   socket (`start_ipc`; a bind error is logged and the app runs without handoff), registers
   `deep_link::listen`, and shows the window (`window::show_main_in_background`) unless started with
   `--hidden`, which hides the Dock icon instead (`window::hide_to_menu_bar`). The window is declared
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
transparent background with the `sidebar` vibrancy effect (`followsWindowActiveState`). It is the only
window; its capability is `capabilities/default.json`.

| Event | Result |
|---|---|
| close button / ⌘W (`window.hide`, `hide_window`) | `window::hide_main`: the window hides; the app, the tray and the Dock icon stay |
| last window closed (`ExitRequested { code: None }`) | prevented |
| ⌘Q (`app.background`, `hide_to_menu_bar`) | `window::hide_to_menu_bar`: the window and the Dock icon hide (`set_dock_icon`, `set_dock_visibility`); the tray keeps running |
| tray "Open Daybook", a second `daybook` launch, `daybook focus`, Dock icon click (`RunEvent::Reopen`) | `window::show_main`: the Dock icon returns, show, unminimize, focus |
| `daybook show` | `window::show_main_in_background`: shown without taking focus |
| `daybook --hidden` (launch at login) | first instance: the window is never shown and the Dock icon is hidden; running instance: nothing |
| "Quit Daybook" (`app.quit`, no default key) in the app menu, the palette and the tray, `quit_app` | `app.exit(0)` |

`show_main_in_background` emits `window_opened` when the window was hidden before (`main_visible`).
`window.rs` never sets the window theme, so its native appearance follows macOS (the page draws every surface
opaquely). `bring_to_front` does nothing with `DAYBOOK_BACKGROUND=1` in debug builds.

## AppState

`state.rs`. Tauri manages it; every command receives it as `State<'_, AppState>`. Each store locks inside
itself. Panics unwind ([ADR 0005](adr/0005-panic-unwind.md)), so code takes `std::sync` locks with `lock::MutexExt::lock_unpoisoned` and
`lock::RwLockExt::{read_unpoisoned, write_unpoisoned}` (`lock.rs`).

| Field | Type | Holds |
|---|---|---|
| `settings` | `SettingsStore` | `settings.json` |
| `keybindings` | `KeybindingsStore` | `keybindings.json`; also feeds the menu |
| `ui_state` | `UiStateStore` | `ui_state.json` |
| `launches` | `LaunchQueue` | launch events the page has not taken yet |

Managed beside it: `remote::Bridge` (requests waiting for the page) and `menu::MenuSync` (the page's last
menu report).

## Tauri commands

Registered in `handlers.rs` (`handlers()`, one `generate_handler!` list). Commands stay thin; logic lives in
its module.

| Command | Args | Returns | Errors |
|---|---|---|---|
| `get_settings` | – | `Settings` | – |
| `update_settings` | `settings: Settings`; `launchAtLogin` is kept as stored (only `autostart_set` changes it) | `Settings` (sanitized) | `io` |
| `get_keybindings` / `set_keybindings` | – / `bindings: Keybindings` | `Keybindings` (sanitized; the menu is rebuilt) | `io` |
| `get_ui_state` / `set_ui_state` | – / `ui: UiState` | `UiState` | `io` |
| `autostart_set` (`autostart.rs`) | `enabled: bool` | `bool`: whether the LaunchAgent is installed now; also saves `settings.launchAtLogin` | `io` |
| `autostart_status` (`autostart.rs`) | – | `bool`: whether the LaunchAgent is installed | – |
| `open_log_folder` | – | – | `io` |
| `open_url` | `url: String` | – | `invalid_input` unless `http(s)` without credentials (`is_web_url`) |
| `log_frontend_error` | `message: String` | – | – |
| `take_launch_events` | – | `LaunchEvent[]` (`error`, `notice`, `activate`) | – |
| `hide_window` | – | – | – |
| `hide_to_menu_bar` | – | – (the window and the Dock icon hide) | – |
| `quit_app` | – | never returns | – |
| `choose_folder` | – | `string \| null` (the folder panel, which can create a folder; `null` when cancelled) | `io` |
| `reveal_path` | `path: String` | – (Finder shows the enclosing folder with `path` selected) | `io` |
| `open_readme` (`help.rs`) | – | – (opens the README, see [Native menu](#native-menu)) | `not_found` (none in this build), `io` |
| `menu_sync` (`menu/sync.rs`) | `state: MenuState { enabled, checked }` | – (greys out and checks the menu items) | – |
| `remote_reply` (`remote.rs`) | `id, result, error` | – | – |

**Adding a command:** write it in its module (or `commands.rs` for app-wide ones), register it in
`handlers.rs`, export its types with ts-rs (`#[ts(export, export_to = "src/api/types.gen.ts")]`, then
`bun run types`; [ADR 0004](adr/0004-generated-ipc-types.md)), add the wrapper to `src/api/index.ts` ([ADR 0002](adr/0002-ipc-in-api-files.md)) and a default to
`src/test/apiMock.ts`.

## Events to the page

`events.rs` names every event, so all emitting goes through one place.

| Constant | Name | Payload |
|---|---|---|
| `LAUNCH_EVENT` | `launch` | none; the page calls `take_launch_events` |
| `COMMAND_EVENT` | `command` | a command id (menu) |
| `REMOTE_EVENT` | `remote` | `RemoteEvent { id, request }`, answered with `remote_reply` |
| `WINDOW_OPENED_EVENT` | `window_opened` | none; `window::show_main_in_background` showed the hidden window |

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
`Exit { message }` (help, version) or `Remote { message, json, starts_app }`. `CliError.in_app` is true when
the arguments name no subcommand (`names_subcommand`): a plain launch with bad arguments also shows the error
in the app, which may have been opened from Finder; a subcommand's error only prints.

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

## Tray

`tray.rs`: `tray::install(app)` builds the menu bar item `main` from `icons/tray/tray.png` (36 px, the menu bar's
18 pt icon height) as a template image (macOS tints it for the menu bar), tooltip `APP_NAME`, menu on click.
The item ids carry a `tray.` prefix, because menu events also reach `menu::forward`:

| Item | Id | Action |
|---|---|---|
| Open Daybook | `tray.open` | `window::show_main` |
| Quit Daybook | `tray.quit` | `app.exit(0)` |

## Persistence

Everything lives in the config dir, `~/Library/Application Support/com.philippstracker.daybook/`. JSON
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
| `ui_state.json` | `UiState` (`store/ui_state.rs`): `sidebarHidden`, `sidebarWidth` (px, default 200), `paneSizes` (splitter positions in px by pane id); missing keys take the defaults, unknown keys are ignored |
| `daybook.sock`, `daybook.lock` | single instance |

### Settings

`store/settings/`: `mod.rs` (`Settings`, `sanitized`), `types.rs` (`Scheme`, `TextSize`, `ACCENTS`,
`SYSTEM_ACCENT`, `is_accent`, `is_app_accent`), `store.rs` (`SettingsStore`),
`serde_helpers.rs` (`lenient`).

| Key | Type | Default |
|---|---|---|
| `lightScheme` | `Scheme`: `default` (a dark sidebar beside light content), `light`, `dark` | `default` |
| `darkScheme` | `Scheme` | `dark` |
| `accent` | `system` (`SYSTEM_ACCENT`, the macOS accent color) or one of `ACCENTS` | `system` |
| `textSize` | `TextSize`: `small`, `medium`, `large`; the pixel sizes per step are the frontend's `TEXT_SIZES` | `medium` |
| `launchAtLogin` | `bool`, mirrors the LaunchAgent | `false` |

Appearance is one scheme per macOS mode ([ADR 0009](adr/0009-three-schemes-four-palettes.md), [ADR 0010](adr/0010-a-scheme-per-system-mode.md)). `Settings` is camelCase with
`#[serde(default)]`, so a partial file fills the missing fields. Every field uses
`deserialize_with = "lenient"`: an unknown or wrongly typed value falls back to that field's default (logged) instead of
quarantining the whole file; `darkScheme` uses `lenient_dark_scheme`, whose fallback is `dark`. Unknown keys
are ignored on load and dropped on the next save. `sanitized()` runs on load and on every update and resets
an unknown accent (`is_app_accent`) to `system`. `SettingsStore::update` returns `(previous, saved)` from one
lock.

`src/api/types.ts` mirrors the constants the page needs (`ACCENTS`, `SYSTEM_ACCENT`, `TEXT_SIZES`); a Rust
test (`accents_match_the_frontend_list`) compares `ACCENTS` with it.

**Adding a setting:**
1. Add the field to `Settings` in `store/settings/mod.rs` (enums in `types.rs`), with a default in
   `Settings::default`.
2. Clamp or reset it in `sanitized`; give an enum field `#[serde(deserialize_with = "lenient")]`.
3. If it changes something outside the store, add it to `SettingsEffects` and apply it in `apply_settings`.
4. Regenerate `src/api/types.gen.ts` (`bun run types`), mirror any constant the page needs in
   `src/api/types.ts`, and add the default to `src/test/settings.ts`.
5. Render it on its page in `src/components/settings/pages.tsx`.

### Key bindings

`store/keybindings.rs`. The webview resolves most shortcuts; the native menu reads the same overrides for its
accelerators, so the two never disagree.

- `Keybindings` is a `BTreeMap<String, Option<String>>`. `KeybindingsStore::update` replaces all overrides.
- `sanitize` drops (and logs) entries whose id is not `[A-Za-z0-9.]+` or whose binding is malformed. Ids are
  not checked against the registry: the frontend owns most commands.
- A binding is modifiers from `Mod`, `Ctrl`, `Alt`, `Shift` and one key (one character or a name such as
  `Enter`), joined by `+`; a trailing `++` (or a bare `+`) is the `+` key (`split`).
- `menu_binding(id)`: the user override, else the menu default (`menu::default_binding`).
- `is_chord`: holds ⌘ or ⌃, so it never types a character. `accelerator` converts to Tauri's form (`Mod` →
  `CmdOrCtrl`, `-` → `Minus`, `=` → `Equal`, `+` → `Shift+Equal`; `None` for a key Tauri cannot name).
  `is_global` is a chord with an accelerator. `key_label` gives `⇧Y`, as `bindingLabel` in
  `src/commands/registry.ts` shows it.

## Native menu

`menu/` builds the menu bar: Daybook (About, Settings…, Launch at Login, Services, Hide, Hide Others, Show
All, Hide to Menu Bar, Quit Daybook), File (Close Window), Edit (native Undo, Redo, Cut, Copy, Paste, Select
All), View (Toggle Sidebar, then the native Enter Full Screen), Window (native Minimize, Zoom, Bring All to
Front) and Help (Keyboard Shortcuts, Command Palette…, Open Log Folder, README). [ux.md](ux.md) lists the
keys.

- **Items.** `menu/items.rs` lists every item that runs an app command once in `MENU_ITEMS` (`MenuItemDef`:
  `id`, `label`, `default_binding`, `check`; built with `item` and `check`). `build()` in `menu/mod.rs`
  places them, in groups of ids with a separator between (`groups`). The TypeScript command with the same id
  has `menu: true` and the same default.

  | Id | Label | Default |
  |---|---|---|
  | `settings.toggle` | Settings… | `Mod+,` |
  | `app.launchAtLogin` | Launch at Login (check item) | – |
  | `app.background` | Hide to Menu Bar | `Mod+Q` |
  | `app.quit` | Quit Daybook | – |
  | `window.hide` | Close Window | `Mod+W` |
  | `sidebar.toggle` | Toggle Sidebar | `Mod+Ctrl+S` |
  | `shortcuts.show` | Keyboard Shortcuts | `Shift+?` |
  | `palette.toggle` | Command Palette… | `Mod+K` |
  | `logs.open` | Open Log Folder | – |
  | `help.readme` | README | – |

- **Bindings.** An item shows `KeybindingsStore::menu_binding` of its command. `label_and_accelerator` turns
  a chord (`keybindings::is_chord`) into an accelerator; any other binding goes into the label (`Keyboard
  Shortcuts (⇧?)`) and is never an accelerator, because a bare-key or ⌥ accelerator would take its key from
  text fields; the page's key handler runs those keys. `set_keybindings` rebuilds the menu (`menu::rebuild`).
  No two items share a default binding (a test checks it).
- **Enabled and checked.** The page reports `MenuState { enabled, checked }` (ids) through `menu_sync`
  whenever it changes; `menu/sync.rs` keeps it in the managed `MenuSync` and applies it to every listed item
  (`item_state`, `apply`): items missing from `enabled` are greyed out, check items show `checked` (Launch at
  Login). Before the first report every item is enabled; `app.quit` and `app.background` always are
  (`ALWAYS_ENABLED`). `rebuild` builds with the last report.
- **Clicks.** `menu::forward`: `app.quit` exits and `app.background` hides to the menu bar right there, so
  both work even when the page is broken; every other listed id is emitted as `command` (`is_menu_command`
  filters out the native items). A click on a check item re-applies the last report first, since macOS
  toggles the mark itself; the page's next report sets it.
- **README.** Help → README calls `open_readme` (`help.rs`): the `README.md` bundled as a resource
  (`bundle.resources` in `tauri.conf.json`), else in debug builds the repository's (`readme_path`), opened in
  the default app.

**Adding a menu item:** add the TypeScript command with `menu: true` (and `checked` for a check item), add
the id, label and default to `MENU_ITEMS` (and to the `DOCUMENTED_IDS` test list), place it in `build()`,
and list it in [ux.md](ux.md).

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
| `tokio` | runtime, IPC socket, timeouts |
| `serde`, `serde_json` (`preserve_order`) | IPC, stores |
| `clap` | CLI |
| `thiserror` | `AppError` |
| `chrono`, `uuid`, `url` | quarantine time stamps, request ids, links and `is_web_url` |
| `ts-rs` | TypeScript types for IPC |
| `libc` | `getuid` for the IPC peer check |
| `tempfile` (dev, and optional behind `test-util`) | store, socket and README tests |

No database and no HTTP client yet. The library builds as `rlib`. The release profile uses LTO,
`codegen-units = 1`, `opt-level = 3` and strip, and keeps `panic = "unwind"` ([ADR 0005](adr/0005-panic-unwind.md)). The toolchain is
pinned to stable in `rust-toolchain.toml`; edition 2024.

## Content security policy

`app.security.csp` in `tauri.conf.json`:

- `default-src 'self'`: scripts, fonts and frames come from the bundle only;
- `img-src 'self' data:`: no view loads a remote image;
- `style-src 'self' 'unsafe-inline'`: React `style` attributes;
- `connect-src ipc: http://ipc.localhost`: Tauri IPC only.

`app.security.devCsp` applies under `tauri dev` and adds inline scripts and Vite's HMR websocket on
`localhost:1420`/`1421`.
