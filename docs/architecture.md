# Architecture

Daybook is the owner's menu-bar companion for the Obsidian vault `~/Obsidian/ThoughtForge`: it guides the day
(status, focus, routines, journal, meetings, reminders) and gives fast access to the vault (quick notes,
search, open todos, links), while Obsidian stays a plain editor. It does not track time; TimeTracker does, and
Daybook will drive it through the `timetracker` CLI. It is a Tauri 2 app: a Rust process owns every piece of
logic, a React 19 webview renders state and forwards intent, and the app keeps running in the menu bar while
its window is closed. The layer pages go into detail: [backend](backend.md), [frontend](frontend.md),
[design language](ux.md).

The app is a shell so far: a tray icon, the main window with the sidebar (the app name and a Settings gear)
and an empty main pane, the Settings dialog (General, Appearance, Shortcuts), the command palette, the
shortcuts dialog, toasts, the native menu and the CLI. The features arrive step by step
(`docs/plan/README.md`, gitignored).

## Data

The data is the vault plus small JSON files ([ADR 0012](adr/0012-vault-and-json-no-database.md)); there is
no database. The JSON files live in the config dir, `~/Library/Application Support/com.philippstracker.daybook/`:

| File | Holds | Store |
|---|---|---|
| `settings.json` | `Settings`: appearance, the vault and daily note, and every feature's configuration ([backend: settings](backend.md#settings)) | `store::settings::SettingsStore` |
| `keybindings.json` | the owner's shortcut overrides by command id | `store::keybindings::KeybindingsStore` |
| `ui_state.json` | window layout: `sidebarHidden`, `sidebarWidth`, `paneSizes` | `store::ui_state::UiStateStore` |
| `pending-entries.json` | note changes waiting for their daily note | `vault::pending::Pending` |
| `context.json`, `status-memory.json`, `jira-tickets.json`, `routines.json`, `day-message.json`, `slack-shown.json` | state imported from the prototype (step 02), read by later steps | – |

Each store wraps a `JsonStore` (`store/json_store.rs`) over a `JsonFile` (`store/json_file.rs`): files are
0600 in a 0700 folder and written atomically; a missing file loads as defaults, unknown keys are ignored and
a file that cannot be parsed is moved aside as `<name>.<timestamp>.corrupt`. The config dir also holds the
socket `daybook.sock` and the instance lock `daybook.lock`.

The vault is `settings.vault.path`. Daybook reads its daily notes and adds lines to known sections through one
write path ([vault.md](vault.md)).

## Design rules

These rules hold everywhere in the code. Keep them when you change things.

1. **Rust owns the logic; React only renders** ([ADR 0001](adr/0001-rust-owns-the-logic.md)). Settings
   sanitizing, menu state, the CLI and later every rule about the vault are Rust. The page asks, renders and
   forwards intent.
2. **IPC only in `api.ts` files** ([ADR 0002](adr/0002-ipc-in-api-files.md)). Only `src/api/index.ts` (and
   `src/<module>/api.ts` for an optional module) imports `@tauri-apps/api` or a `@tauri-apps/plugin-*`
   package. Everything else imports the `api` object, which is also what UI tests mock. A boundary test
   (`src/test/boundary.test.ts`) enforces it.
3. **Errors are data** ([ADR 0003](adr/0003-errors-as-kinds.md)). Every command returns
   `Result<T, AppError>`, serialized as `{ kind, message, field? }` (`ErrorJson` in `error.rs`) with the kinds
   `io`, `invalid_input`, `not_found` and `cancelled`. The UI branches on `kind` only.
4. **Types are generated** ([ADR 0004](adr/0004-generated-ipc-types.md)). Every type crossing IPC derives
   `ts_rs::TS` and exports to `src/api/types.gen.ts` (camelCase). `bun run types` regenerates it;
   `bun run lint` fails on drift. Never edit the generated file.
5. **Panics unwind** ([ADR 0005](adr/0005-panic-unwind.md)). A bug fails one command, not the app. Locks
   are taken with the poison-tolerant helpers in `lock.rs`.
6. **No color literal outside `src/styles/palettes/`** ([ADR 0007](adr/0007-color-palettes.md),
   [ADR 0009](adr/0009-three-schemes-four-palettes.md), [ADR 0010](adr/0010-a-scheme-per-system-mode.md)).
   Palettes are complete token sets; a Vitest scan enforces it.
7. **One write path into the vault** ([ADR 0015](adr/0015-one-write-path-into-the-vault.md)). Only
   `vault::writer::NoteWriter::edit_note` writes into the vault; a change is a `NoteOp` applied by a pure
   function that touches only its section's lines. Daily notes are created by Obsidian, and nothing goes into
   frontmatter.
8. **Overlays never navigate** ([ADR 0016](adr/0016-overlays-never-navigate.md)). Overlays are a
   non-activating panel; closing one never switches Spaces or brings back an app the owner left
   ([windows.md](windows.md)).
9. **The look is native** ([ADR 0011](adr/0011-native-macos-look.md)): system font, inset groups, a unified
   toolbar, AppKit control metrics ([ux.md](ux.md)).

## A setting, end to end

Changing the accent, the main write path today:

1. Settings → Appearance (`src/components/settings/pages.tsx`) calls the `settings` slice's
   `saveSettings` with the edited `Settings`.
2. The slice calls `api.updateSettings(settings)`, which invokes the Tauri command `update_settings`
   (`src/api/index.ts` is the only place that calls `invoke`).
3. `commands::update_settings` hands it to `SettingsStore::update`, which sanitizes it (`Settings::sanitized`:
   an unknown accent becomes `system`), writes `settings.json` and returns the previous and the saved value.
4. `commands::apply_settings` applies what changed outside the store (`settings_effects`: today only
   `launchAtLogin`, through `autostart::apply`).
5. The saved `Settings` comes back as the command's result; the slice stores it, and `App` calls
   `applyAppearance` (`lib/theme.ts`), which sets `data-content`, `data-sidebar`, `data-accent` and the font
   sizes on `<html>`.

## A CLI request, end to end

`daybook status`:

1. `lib.rs` `run` parses argv (`cli::parse`) into `CliOutcome::Remote { message: IpcMessage::Status, … }`.
2. `instance::ask` sends it as an `IpcRequest` line to `daybook.sock` (`ipc::request`) and waits for the
   `IpcReply` with the same id.
3. In the running app, `ipc::serve` checks the peer's uid and calls `remote::handle`, which answers `Status`
   with a `StatusReport { version, windowVisible }`.
4. The CLI prints it through `cli::reply::render`, or as JSON with `--json`, and exits 0. With no app running
   it exits 2 with "Daybook is not running"; `show`, `focus` and a bare `daybook` start the app instead.

## Process model

```
┌──────────── Daybook.app ──────────────────────────────────────────────────┐
│  Webview (React, hidden when closed)      Rust core (tokio runtime)       │
│  ┌──────────────────────┐   invoke   ┌─────────────────────────────────┐  │
│  │ components/          │ ─────────▶ │ #[tauri::command]s              │  │
│  │ state/store.ts       │            │ commands.rs, autostart, help    │  │
│  │ commands/ (registry) │ ◀───────── │                                 │  │
│  │ api/index.ts         │   events   ├─────────────────────────────────┤  │
│  └──────────────────────┘            │ AppState: settings, keybindings,│  │
│  Tray icon, native menu ◀───────────▶│ ui_state, launches              │  │
│                                      │ store/ (JSON files)             │  │
└──────────────────────────────────────┴───────┬─────────────────────────┴──┘
                     Unix socket (ipc.rs)      │
  `daybook …` (2nd process, CLI) ──────────────┘
  daybook://show (deep link): macOS → tauri-plugin-deep-link → launch queue
```

- **One instance.** The first process binds `daybook.sock` in the config dir and holds `daybook.lock`. A
  second `daybook …` sends its request over the socket and exits
  ([backend: single instance](backend.md#cli-and-single-instance-handoff),
  [ADR 0008](adr/0008-cli-asks-the-running-app.md)).
- **Window.** Closing the window (also ⌘Q, Hide to Menu Bar) hides it and the Dock icon; the app keeps running
  in the menu bar. Every show moves it to the pointer's screen. Quit only from the tray menu, the app menu's Quit Daybook, or `daybook quit`
  ([backend: window lifecycle](backend.md#window-lifecycle)). A Dock click shows the window again; showing a
  hidden window emits `window_opened`.
- **Overlay.** A non-activating panel (`window/panel/`) with its own page (`src/windows/panel/`) renders one view
  at a time over any app, on the pointer's screen ([windows.md](windows.md)).
- **Tray.** A template-image menu bar item with Open Daybook and Quit Daybook (`tray.rs`).
- **Launch at login** through `tauri-plugin-autostart` (a LaunchAgent) with `--hidden`, so the app starts
  without showing the window.
- **Launch requests are pulled, not pushed.** Rust queues `LaunchEvent`s (`Error`, `Notice`, `Activate`) and
  emits a bare `launch` event; the page calls `take_launch_events`, so it can't miss the request that started
  the app. Deep links (`deep_link.rs`; only `daybook://show`) go through the same queue.
- **Commands live in Rust.** Menus, global shortcuts and the palette run commands of one Rust bus by id;
  the page runs window-level commands through `run_command` and keeps its registry for keys inside a window
  ([commands.md](commands.md), [ADR 0017](adr/0017-commands-live-in-rust.md)).
- **Requests only the page can answer** go out as a `remote` event with an id and come back through
  `remote_reply` (`remote::Bridge`). `RemoteRequest` has no variants yet.

## Repository map

```
src-tauri/
  Cargo.toml                 crate `daybook`, lib `daybook_lib`, edition 2024
  tauri.conf.json            window, CSP, deep-link scheme, bundle; tauri.debug.conf.json renames the debug bundle
  src/main.rs lib.rs         startup: config dir, CLI parse, socket handoff, AppState, then app::start
  src/app.rs handlers.rs     Tauri builder (plugins, setup, run loop); every registered command
  src/instance.rs            socket handoff to the running app, instance lock, subcommand replies
  src/cli/                   CLI: command tree (mod.rs), human replies (reply.rs)
  src/ipc.rs remote.rs pending.rs launch.rs   socket, dispatch and requests the page answers, launch queue
  src/events.rs              every event name the backend emits
  src/state.rs               AppState
  src/error.rs lock.rs       AppError (+ ErrorKind), poison-tolerant lock helpers
  src/store/                 JsonFile, JsonStore, settings/, keybindings.rs, ui_state.rs
  src/commands.rs            settings, keybindings, ui state, window, links, folders, logs
  src/bus/                   the command bus and the app's commands (commands.md)
  src/tools/                 the tools and their choices, which tools a surface shows
  src/palette/               palette entries, ranking, running a pick
  src/shortcuts/             global shortcuts, accelerator names
  src/tray.rs notify.rs      menu-bar item, notifications
  src/deep_link.rs           daybook:// links
  src/window/                the Daybook window, the overlay panel (panel/), screens, focus hand-back (windows.md)
  src/menu/                  app menu (mod.rs), its keys (command_shortcuts.rs), the menu-bar menu model (model.rs)
  src/help.rs                Help menu targets outside the app (the README)
  src/autostart.rs           launch at login
  src/vault/                 daily notes: location, note edits, ops, writer, pending changes, Obsidian CLI, watcher (vault.md)
  src/clock.rs process.rs    HH:MM and time windows; child processes with a timeout
  src/dev.rs                 debug builds only: window frame, background launch, eval
  src/test_support/          test helpers behind cfg(test) / feature test-util
src/
  main.tsx modules.ts App.tsx
  api/index.ts types.gen.ts types.ts
  state/store.ts requests.ts slices/ (settings, ui, viewState)
  commands/                  registry, scopes (only `global`), index (APP_COMMANDS: the window's own keys), remote
  components/                Sidebar, EmptyView, Toolbar, Modal, Dialogs, Toasts, settings/, …
  windows/panel/             the overlay page: PanelApp, PaletteView, SampleView
  registry/settingsPages.ts
  lib/ styles/ test/
docs/                        this folder; adr/ holds the decision records
scripts/dev/ scripts/*.sh    dev loop; install, stop, signing
.claude/                     rules, agents, skills, settings
```

## Platform

The code base targets **macOS on Apple Silicon** only. The platform-specific parts are listed in
[adapting.md](adapting.md#platform-specific-code): the config path, Unix sockets, window vibrancy, the tray,
the LaunchAgent and the dev scripts.
