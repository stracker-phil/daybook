# Adapting Daybook

Daybook is written so that a coding agent can take it over. Clone the repo, open it in Claude Code (or a
similar tool), and describe what you need. `CLAUDE.md` is loaded automatically; it points the agent to the
rules in `.claude/rules/` and to these docs. The `ship-step` skill runs after every step (cleanup, review,
tests, lint, build, launch, commit), and subagents in `.claude/agents/` write the tests, run the suites and
review changed files. This page lists what such changes touch.

## Identity

The app's name and ids live in a few places. Change them together. Debug builds (`bun run build:app`) are a
second app with their own name, identifier, data, socket and URL scheme, so they run beside the release app
([ADR 0014](adr/0014-debug-build-has-its-own-data.md)).

| What | Release | Debug | Where |
|---|---|---|---|
| Product name | `Daybook` | `Daybook (Debug)` | `tauri.conf.json` `productName` and window `title`; `tauri.debug.conf.json` `productName`; `lib.rs` `APP_NAME` (the menu bar and the tray tooltip); `bus/commands.rs`, `menu/model.rs`, `cli/grammar.rs` and `cli/help.rs` (`Quit Daybook`); `tray.rs` (item labels); `index.html` and `src/windows/*/index.html`; the settings page intros in `components/settings/pages.tsx`; user-facing errors in `lib.rs`, `instance.rs`, `ipc.rs`, `remote.rs`, `deep_link.rs` and `cli/reply.rs` |
| Bundle | `Daybook.app` | `Daybook (Debug).app` | `scripts/install.sh`, `scripts/stop-app.sh`, `scripts/dev/launch.sh`, `scripts/dev/smoke.sh`, the `app` script of `package.json`; the window owner `Daybook (Debug)` in `scripts/dev/screenshot.sh` |
| Bundle identifier | `com.philippstracker.daybook` | `com.philippstracker.daybook.debug` | `tauri.conf.json` and `tauri.debug.conf.json` `identifier`; `lib.rs` `IDENTIFIER` (a test compares it with both files); the signing identifier in `scripts/install.sh` and `scripts/dev/launch.sh`; the socket path in `scripts/dev/js.sh` and `scripts/dev/smoke.sh`. It names the config dir, the log dir and the Keychain service, so changing it starts with empty settings and no tokens |
| Binary and CLI command | `daybook` | the same binary name inside the debug bundle, run by path | Cargo package `name` in `src-tauri/Cargo.toml` (also the self dev-dependency); `cli/mod.rs` (`name`), `cli/help.rs` `TOOL`; `cli/reply.rs` (`Daybook <version>` line); the README's command examples; `scripts/install.sh`, `scripts/stop-app.sh`, `scripts/dev/launch.sh`, `scripts/dev/smoke.sh`; `.claude/settings.json` (`pkill -x daybook`) |
| Installed links | `~/Applications/Daybook.app`, `~/.local/bin/daybook` | none: debug builds are never installed | `scripts/install.sh` (the CLI link only when the path is free or already its link, else `--replace-cli`); `automation/cli.rs` `cli_info` (whether `~/.local/bin/daybook` runs this app) |
| Rust lib | `daybook_lib` | | `Cargo.toml` `[lib]`; `main.rs` |
| URL scheme | `daybook://` | `daybook-debug://` | `tauri.conf.json` and `tauri.debug.conf.json` `plugins.deep-link.desktop.schemes` (the bundler writes them into `Info.plist`); `lib.rs` `LINK_SCHEME` (a test compares it with both files); `deep_link.rs` runs the automation actions `state`, `show`, `status`, `slack`, `command` ([automation.md](automation.md#links)) |
| Socket, lock | `daybook.sock`, `daybook.lock` in the config dir | the same names in the debug config dir | `lib.rs` `SOCKET_NAME`, `instance.rs` `LOCK_NAME`; `scripts/dev/js.sh`, `scripts/dev/smoke.sh` (the socket) |
| Config dir | `~/Library/Application Support/com.philippstracker.daybook/` | `….daybook.debug/` | `lib.rs` `config_dir()` |
| Data files | in the config dir: `settings.json`, `keybindings.json`, `ui_state.json`, `context.json`, `status-memory.json`, `pending-entries.json`, `routines.json`, `day-message.json`, `slack-shown.json`, `music-held.json`, `jira-tickets.json`, `checkins.json`, `quick_notes.json` | the same in the debug config dir | `store/settings/store.rs`, `store/keybindings.rs`, `store/ui_state.rs`, `contexts/mod.rs`, `status/memory.rs`, `vault/pending/mod.rs`, `routines/fired.rs`, `slack/day_message.rs`, `slack/shown.rs`, `state.rs` (`music_held`), `jira/tickets.rs`, `activity/pause.rs`, `quick_notes/mod.rs` |
| Logs dir | `~/Library/Logs/com.philippstracker.daybook/` | `….daybook.debug/` | Tauri's `app_log_dir`, from the identifier |
| LaunchAgent | `~/Library/LaunchAgents/Daybook.plist`, written by `tauri-plugin-autostart` and named after `productName` | named after the debug `productName` | `app.rs` (`MacosLauncher::LaunchAgent`, `--hidden`), `autostart.rs` |
| Keychain | generic passwords, service = the bundle identifier: accounts `slack-token` and `api-token` (the HTTP API) | the debug identifier | `secrets.rs`, `slack/token.rs`, `http_api/token.rs` |
| HTTP API | `127.0.0.1:27180` (`settings.api.port`), off by default | the same setting in the debug config dir | `http_api/`, `store/settings/` ([ADR 0021](adr/0021-a-local-http-api-with-guards.md)) |
| Stage discovery | `~/Library/Application Support/glaze-stage/apps/<identifier>.json` | the debug identifier | `stage/discovery.rs` ([stage.md](stage.md)) |
| Signing certificate | `Daybook Local Signing` in the login keychain, or `DAYBOOK_SIGN_IDENTITY` | the same certificate | `scripts/create-signing-cert.sh`, `scripts/sign-app.sh`; `tauri.conf.json` `bundle.macOS.signingIdentity: "-"` (ad hoc at build time, re-signed by the scripts). The grant for Accessibility, Calendars and Automation belongs to identifier + certificate ([testing.md](testing.md#signing)) |
| Env vars | `DAYBOOK_SIGN_IDENTITY` | also `DAYBOOK_WINDOW_FRAME`, `DAYBOOK_BACKGROUND`, `DAYBOOK_DEV_TERMINAL` | `scripts/sign-app.sh`; `dev.rs` (compiled into debug builds only); `scripts/dev/launch.sh` |
| Store on `window` | none | `window.daybook` (`VITE_DEV_HOOKS=1`) | `src/main.tsx`, `src/vite-env.d.ts`, the dev scripts |
| Icons | the prototype's app icon | the same | `src-tauri/icons/` generated with `bun tauri icon <1024px png>` (only the desktop files are kept); tray template images in `src-tauri/icons/tray/` |
| Author, license | Philipp Stracker, MIT | | `Cargo.toml`, `package.json`, `LICENSE` |

## Network hosts

`net.rs` is the only HTTP client and talks only to the hosts in `HOSTS`
([ADR 0020](adr/0020-network-only-to-listed-hosts.md)); any other host is `NetError::NotAllowed`.

| Host | For | Where |
|---|---|---|
| `slack.com` | the Slack Web API: identity, status, presence, channel messages | `slack/` |
| `itunes.apple.com` | the artist search for music entries | `music/` |

Everything else leaves the Mac only through other apps: links and Jira issues open in the browser
(`tauri-plugin-opener`), joining hands a meeting to the call app (`calls/join.rs`). A new host goes into
`HOSTS` with an ADR.

## External tools

Daybook drives other apps through their command line tools, found without the login `PATH` (an app started by
launchd has a short one; `process.rs` `EXTRA_PATH` adds the Homebrew folders).

| Tool | Needed for | Where |
|---|---|---|
| `timetracker` (TimeTracker's CLI) | the timer, projects, recordings, suggestions, `show` | see [TimeTracker](#timetracker) |
| `obsidian` (the Obsidian CLI) | creating today's note | `vault/obsidian.rs` `CANDIDATES` |
| `plaunch` (Project Launcher) | the Projects overlay, `projects:open:<id>` | `launcher/tool.rs` `TOOL`, `Launcher::default` (`~/.local/bin`, `/usr/local/bin`, `/opt/homebrew/bin`) |
| `osascript` | Music (JXA), Chrome's tabs for call detection | `music/script.rs`, `calls/probe.rs` |
| `lsappinfo`, `pgrep`, `pmset`, `ps`, `ioreg` | call detection, the lock state | `calls/probe.rs`, `presence/native.rs` |
| `open -b` | starting call apps before joining | `calls/join.rs` |

### TimeTracker

TimeTracker (`../timetracker`) owns timers; Daybook never tracks time
([ADR 0022](adr/0022-daybook-drives-timetracker-through-its-cli.md), [timetracker.md](timetracker.md)).

- **Path:** `settings.timetracker.path`, default `~/.local/bin/timetracker` (`timetracker/client.rs`
  `DEFAULT_PATH`; Settings → Connections → TimeTracker).
- **Words Daybook runs**, each with `--json`: `status`, `projects --archived`, `timer status`,
  `timer recent --limit <n>`, `timer start <project> [--note=…]`, `timer stop`, `record …`, `recordings <day>`,
  `suggestions <day>`, `show [day]`, `focus`, and `watch --json` as a long-running stream
  (`timetracker/watch.rs`). An older TimeTracker without `watch` is polled with `timer status`.
- **Contract:** exit 0 with the JSON reply on stdout, exit 1 with a JSON refusal, exit 2 when TimeTracker isn't
  running (`timetracker/client.rs` `reply_result`, the parsers in `timetracker/reply.rs`).
- **Debug builds** run only the words in `READS` (`changes_data`); writes are logged as dry runs.
- Without TimeTracker the app works; the timer surfaces show "not running" or "missing".

### Prototype import

`store/import/` copies the Glaze prototype's configuration once
([ADR 0013](adr/0013-one-settings-model-imported-once.md)). It runs while Daybook has no `settings.json` and
`~/Library/Application Support/app.glaze.macos.2mykv1oc-local/` (`PROTOTYPE_DIR`) has one, and only reads that
folder.

- Settings: converted by `store/import/settings.rs`; tracking-only keys are dropped (`DROPPED`).
- State files: `STATE_FILES` in `store/import/state.rs` (`mode.json` becomes `context.json`; `status-memory.json`,
  `jira-tickets.json`, `routines.json`, `day-message.json`, `slack-shown.json`, `pending-entries.json` keep their
  names).
- The Slack token (`slack-token.bin`, Electron safeStorage) can't be read: the report asks for it again.
- `settings.json` is written last, so an import that fails part-way runs again at the next start.

Owner-specific: delete the module and its calls in `lib.rs` and `app.rs` (`report_import`) to drop it.

## Platform-specific code

| Piece | Where | Linux | Windows |
|---|---|---|---|
| **Tray**: template images (black with alpha, tinted by macOS), menu on left click | `tray.rs`, `src-tauri/icons/tray/` | Works with an `AppIndicator`; template tinting does not apply, ship colored icons | Works; colored icons |
| **Dock icon**: hidden by Hide to Menu Bar, shown with the window | `window/mod.rs` `set_dock_icon` | No Dock; nothing to do | Same |
| **Calendar**: EventKit through `objc2-event-kit`, the usage strings in `src-tauri/Info.plist` | `calendar/eventkit.rs` | No EventKit: read CalDAV or `.ics` instead behind the same `Store` | No EventKit: the Windows calendar APIs |
| **Presence**: NSWorkspace sleep/wake, the distributed lock/unlock notifications, `ioreg` for the lock state | `presence/native.rs` | logind / D-Bus session signals | `WTSRegisterSessionNotification` |
| **Call detection**: `lsappinfo`, `pgrep`, `pmset -g assertions`, `ps`, `CGWindowListCopyWindowInfo`, `osascript` (Chrome tabs; the Automation usage string `NSAppleEventsUsageDescription` in `src-tauri/Info.plist`), Accessibility window titles; bundle ids in `store/settings/calls.rs` | `calls/probe.rs`, `calls/access.rs`, `calls/checks.rs` | Process names and PulseAudio/PipeWire sources behind the same `Probe` | Process list and the audio session API behind the same `Probe` |
| **Secrets**: the login Keychain through `security-framework` | `secrets.rs` | Secret Service (`libsecret`) behind `SecretStore` | Credential Manager behind `SecretStore` |
| **Time zone label**: `localtime_r`'s `tm_zone` | `status_rules/mod.rs` `zone_of` | Works | No `tm_zone`: format the zone from the Windows API |
| **Joining**: `open -b`, a Chrome web app through `--app-id` | `calls/join.rs` | `xdg-open`; Chrome's `--app-id` works the same | `start`; same for Chrome |
| **Launch at login**: `MacosLauncher::LaunchAgent` with `--hidden` | `app.rs` (autostart plugin), `autostart.rs` | The plugin writes an XDG autostart entry | The plugin writes a registry key |
| **Shortcuts**: `Mod` is ⌘, labels use ⌘⌥⇧ glyphs | `src/commands/registry.ts`, `store/keybindings.rs`, `src/lib/shortcuts.ts` `shortcutKeys`, `menu/mod.rs` | Map `Mod` to Ctrl | Same |
| **Config dir** hard-coded to `~/Library/Application Support/<id>` | `lib.rs` `config_dir()`, `scripts/dev/js.sh` | `dirs::config_dir()`; it must resolve before Tauri starts, because the CLI and socket run first | `dirs::config_dir()` (`%APPDATA%\<id>`) |
| **Unix socket** for the single-instance handoff and the CLI, with a peer-uid check (`libc::getuid`) | `ipc.rs` | Works as is; mind the ~108-byte path limit | A named pipe (`tokio::net::windows::named_pipe`) with the same line protocol |
| **File permissions** (0600 files, 0700 dirs) | `ipc.rs`, `store/json_file.rs` | Works | Gate with `#[cfg(unix)]` |
| **Window chrome**: overlay title bar, `trafficLightPosition`, `transparent`, `windowEffects: ["sidebar"]`, `macOSPrivateApi` | `tauri.conf.json`; Cargo feature `macos-private-api` | Ignored; the page is opaque already | `windowEffects: ["mica"]`, or an opaque background |
| **Traffic-light space** and drag regions | `--traffic-lights` and `--toolbar-height` in `theme.css`; `data-tauri-drag-region` in `Toolbar.tsx` and `Sidebar.tsx` (`.sidebar-titlebar`) | Remove the inset and the strip | Same |
| **System accent**: WebKit's `AccentColor` follows the macOS accent color; the sidebar turns transparent for the window's vibrancy where its tone matches the mode | `theme.css` (`[data-accent="system"]`), `sidebar.css` | `AccentColor` is the platform's accent or a default blue; drop the transparent sidebar rules | Same |
| **Menu**: app menu with Services, Hide, Show All | `menu/mod.rs` | Move Settings and Quit into a File menu | Same |
| **Fonts**: the system font (`-apple-system`, SF Pro) and `ui-monospace` (SF Mono), with `system-ui` and `Menlo` fallbacks | `theme.css` (`--font-ui`, `--font-mono`) | Falls back to the platform's UI font; bundle a font if it must match | Same |
| **Bundle targets**: `app` and `dmg`, signed ad hoc (`bundle.macOS.signingIdentity: "-"`), `icon.icns` | `tauri.conf.json` `bundle`; `package.json` `release`, `build:app`, `app` | `deb`, `appimage` | `nsis`, `msi` (`icon.ico` is already generated) |
| **Install and dev scripts** (zsh, `ln` into `~/Applications` and `~/.local/bin`, `codesign`, Swift, CoreGraphics, `screencapture`) | `scripts/install.sh`, `scripts/stop-app.sh`, `scripts/sign-app.sh`, `scripts/create-signing-cert.sh`, `scripts/dev/` | Link into `~/.local/bin` and a `.desktop` file; rewrite the dev scripts or use `bun tauri dev` | PowerShell |
| **Obsidian CLI**: looked up in a login `/bin/zsh`, then in `/usr/local/bin`, `/opt/homebrew/bin` and `Obsidian.app`; child processes get the Homebrew folders on the `PATH` | `vault/obsidian.rs` `CANDIDATES`, `process.rs` (`EXTRA_PATH`, `run_in_login_shell`) | The user's shell and `/usr/bin/obsidian`; drop the Homebrew paths | `where obsidian` and `%LOCALAPPDATA%\Obsidian`; no login shell |
| **File watcher**: `notify`'s FSEvents backend; reported paths are canonical (`/private/var` for `/var`) | `vault/watch.rs`, `vault/commands.rs` `same_file` | inotify; works as is | ReadDirectoryChangesW; works as is |
| **Overlay panel**: a non-activating `NSPanel` by class swap (`DaybookPanel`, which needs tao pinned to 0.37.1), Space behaviour, `NSScreen`/`NSEvent` for the pointer's screen, `NSWorkspace` and the window list for focus hand-back | `window/native.rs`, `window/panel/native.rs` ([windows.md](windows.md#what-needed-objc2)) | An always-on-top undecorated window; no Spaces | A tool window with `WS_EX_NOACTIVATE` |
| **Rail and its tooltip**: `DaybookInertPanel`s by class swap that never become key, on every Space and over full-screen apps; the pointer is polled and relayed to the pages | `rail/native.rs`, `rail/mod.rs` | An always-on-top undecorated window; poll the pointer through X11/Wayland | A `WS_EX_NOACTIVATE` tool window |
| **Quick notes editor**: level above the menu bar, hidden window buttons, moving to the active Space, screen-change notification, presentation over one screen | `window/native.rs`, `window/presentation.rs` | Always-on-top window; full screen through the window manager | Same |
| **Accessibility**: `AXIsProcessTrusted` and the prompt; window titles through `AXUIElement` | `calls/access.rs`, `calls/probe.rs` | AT-SPI, or drop title checks | UI Automation |
| **Music**: JXA through `osascript` (Automation permission) | `music/script.rs` | MPRIS over D-Bus | The media session API |
| **Notifications**: `tauri-plugin-notification`, a button runs a bus command | `notify.rs` | Works through the plugin; actions depend on the desktop | Works through the plugin |
| **Random token**: `SecRandom` for the HTTP API token | `http_api/token.rs` | `getrandom` | `getrandom` |
| **Global shortcuts**: registered only in debug builds until the prototype is retired (`shortcuts::REGISTERED`, [ADR 0018](adr/0018-global-shortcuts-only-in-debug-builds.md)) | `shortcuts/` | Works through the plugin on X11 | Works through the plugin |
| **Tests**: the setup pretends to be a Mac (`navigator.platform`) | `src/test-setup.ts` | Make the platform a parameter | Same |

A port is done when `bun run test`, `bun run lint` and `bun run release` pass on the target, and the app can:
hand a second `daybook` launch and every CLI command to the running instance; hide on close and come back
from the tray; start hidden at login; show its menus and shortcuts with the right modifier.

## Common changes

| Goal | Start here |
|---|---|
| Add a command or shortcut | [frontend.md: commands](frontend.md#commands) |
| Add a menu item | [backend.md: native menu](backend.md#native-menu) |
| Add a setting | [backend.md: persistence](backend.md#persistence) |
| Add a settings page | [frontend.md: Settings](frontend.md#settings) |
| Add a Tauri command | [backend.md: Tauri commands](backend.md#tauri-commands) |
| Add a CLI subcommand | [backend.md: command tree](backend.md#command-tree); the steps are listed in `cli/mod.rs` |
| Add a `daybook://` link | [backend.md: deep links](backend.md#deep-links) |
| Emit a new event | [backend.md: events](backend.md#events-to-the-page) |
| Change colors or schemes | [frontend.md: styling](frontend.md#styling), [ux.md](ux.md#appearance-and-tokens) |
| Remove the tray | [Dropping the tray](#dropping-the-tray) |

### Dropping the tray

The tray is the only way back to a hidden window besides a second launch, a Dock click and `daybook show`,
so decide first: either quit on close (remove `prevent_close` in `app.rs`'s `WindowEvent::CloseRequested`
handler and the `ExitRequested` `prevent_exit`), or keep running and rely on the Dock (`RunEvent::Reopen` →
`window::show_main`, already wired; then Hide to Menu Bar must stop hiding the Dock icon in
`window::hide_main`). Then delete `tray.rs`, `pub mod tray;` in `lib.rs`, `tray::install` in `app.rs`,
`src-tauri/icons/tray/`, and the `tray-icon` feature of `tauri` in `Cargo.toml`. Update the Tray section of
[backend.md](backend.md#tray).

## Conventions to keep

- **Rust owns the logic.** The frontend renders and forwards intent.
- **IPC only in `api.ts` files.**
- **Typed errors** cross the boundary as `{kind, message}`.
- **No database to start.** The vault plus small JSON files; a database only when a step needs one.
- **No color literal outside a palette.**
- **Tests with every change**, through public interfaces ([`.claude/rules/testing.md`](../.claude/rules/testing.md)).
- **One commit per step or unit**, each revertible, never pushed ([`.claude/rules/commits.md`](../.claude/rules/commits.md)).
