# Adapting Daybook

Daybook is written so that a coding agent can take it over. Clone the repo, open it in Claude Code (or a
similar tool), and describe what you need. `CLAUDE.md` is loaded automatically; it points the agent to the
rules in `.claude/rules/` and to these docs. The `ship-step` skill runs after every step (cleanup, review,
tests, lint, build, launch, commit), and subagents in `.claude/agents/` write the tests, run the suites and
review changed files. This page lists what such changes touch.

## Identity

The app's name and ids live in a few places. Change them together.

| What | Value | Where |
|---|---|---|
| Product name | `Daybook` | `tauri.conf.json` `productName` and window `title`; `lib.rs` `APP_NAME` (`Daybook (Debug)` in debug builds; the menu bar and the tray tooltip); `bus/commands.rs`, `menu/model.rs` and `cli/mod.rs` (`Quit Daybook`); `tray.rs` (item labels); `index.html` and `src/windows/panel/index.html`; the settings page intros in `components/settings/pages.tsx`; user-facing errors in `lib.rs`, `instance.rs`, `ipc.rs`, `remote.rs`, `deep_link.rs` and `cli/reply.rs`; the `Daybook.app` path in `scripts/install.sh`, `scripts/stop-app.sh`, `scripts/dev/launch.sh` and the `app` script of `package.json`; the window owner in `scripts/dev/screenshot.sh`; the certificate name in `scripts/create-signing-cert.sh` and `scripts/sign-app.sh` |
| Bundle identifier | `com.philippstracker.daybook` | `tauri.conf.json` `identifier`; `lib.rs` `IDENTIFIER` (a test compares both); `scripts/dev/js.sh` (socket path); `scripts/install.sh` and `scripts/dev/launch.sh` (the signing identifier); `tauri.debug.conf.json` and `lib.rs` `IDENTIFIER` in debug builds (`com.philippstracker.daybook.debug` and `Daybook (Debug)`, so macOS lists and grants it apart and it has its own data, ADR 0014); `scripts/dev/js.sh` uses the debug folder's socket. It also names the config dir and the log dir; changing it starts with empty settings |
| Binary and CLI command | `daybook` | Cargo package `name` in `src-tauri/Cargo.toml` (also the self dev-dependency); `cli/mod.rs` `name` and `EXAMPLES`; `cli/reply.rs` (`Daybook <version>` line); the README's command examples; `scripts/install.sh`; `scripts/stop-app.sh`; `scripts/dev/launch.sh`; `.claude/settings.json` (`pkill -x daybook`) |
| Rust lib | `daybook_lib` | `Cargo.toml` `[lib]`; `main.rs` |
| URL scheme | `daybook://` (only `daybook://show`) | `tauri.conf.json` `plugins.deep-link.desktop.schemes` (the bundler writes it into `Info.plist`); `deep_link.rs` (`parse_link`, error text) |
| Socket, lock | `daybook.sock`, `daybook.lock` in the config dir | `lib.rs` `SOCKET_NAME`, `instance.rs` `LOCK_NAME`; `scripts/dev/js.sh` (the socket) |
| Config files | `settings.json`, `keybindings.json`, `ui_state.json` in the config dir (plus the state files the import copies) | `store/settings/store.rs` `FILE_NAME`, `store/keybindings.rs`, `store/ui_state.rs` |
| LaunchAgent | `~/Library/LaunchAgents/Daybook.plist`, written by `tauri-plugin-autostart` and named after `productName` | `app.rs` (`MacosLauncher::LaunchAgent`, `--hidden`), `autostart.rs` |
| Config dir | `~/Library/Application Support/com.philippstracker.daybook/` (debug builds: `….daybook.debug/`) | `lib.rs` `config_dir()` |
| Prototype import | reads `~/Library/Application Support/app.glaze.macos.2mykv1oc-local/` once (ADR 0013) | `store/import/` (`PROTOTYPE_DIR`); owner-specific: delete the module and its calls in `lib.rs` and `app.rs` (`report_import`) to drop it |
| Keychain | generic password, service = the bundle identifier, account `slack-token` | `secrets.rs`, `slack/token.rs` |
| Network hosts | `slack.com`, `itunes.apple.com` (the artist search) | `net.rs` `HOSTS` ([ADR 0020](adr/0020-network-only-to-listed-hosts.md)) |
| Logs dir | `~/Library/Logs/com.philippstracker.daybook/` (debug builds: `….daybook.debug/`) | Tauri's `app_log_dir`, from the identifier |
| Env vars | `DAYBOOK_WINDOW_FRAME`, `DAYBOOK_BACKGROUND` (debug builds), `DAYBOOK_DEV_TERMINAL`, `DAYBOOK_SIGN_IDENTITY` | `dev.rs`, `window.rs`, `scripts/dev/launch.sh`, `scripts/sign-app.sh` |
| Store on `window` | `window.daybook` | `src/main.tsx`, `src/vite-env.d.ts`, the dev scripts |
| Icons | the prototype's app icon | `src-tauri/icons/` generated with `bun tauri icon <1024px png>` (only the desktop files are kept); tray template images in `src-tauri/icons/tray/` |
| Author, license | Philipp Stracker, MIT | `Cargo.toml`, `package.json`, `LICENSE` |

## Platform-specific code

| Piece | Where | Linux | Windows |
|---|---|---|---|
| **Tray**: template images (black with alpha, tinted by macOS), menu on left click | `tray.rs`, `src-tauri/icons/tray/` | Works with an `AppIndicator`; template tinting does not apply, ship colored icons | Works; colored icons |
| **Dock icon**: hidden by Hide to Menu Bar, shown with the window | `window.rs` `set_dock_icon` (`cfg(target_os = "macos")`) | No Dock; nothing to do | Same |
| **Calendar**: EventKit through `objc2-event-kit`, the usage strings in `src-tauri/Info.plist` | `calendar/eventkit.rs` | No EventKit: read CalDAV or `.ics` instead behind the same `Store` | No EventKit: the Windows calendar APIs |
| **Presence**: NSWorkspace sleep/wake, the distributed lock/unlock notifications, `ioreg` for the lock state | `presence/native.rs` | logind / D-Bus session signals | `WTSRegisterSessionNotification` |
| **Call detection**: `lsappinfo`, `pgrep`, `pmset -g assertions`, `ps`, `CGWindowListCopyWindowInfo`, `osascript` (Chrome tabs), Accessibility window titles; bundle ids in `store/settings/calls.rs` | `calls/probe.rs`, `calls/access.rs`, `calls/checks.rs` | Process names and PulseAudio/PipeWire sources behind the same `Probe` | Process list and the audio session API behind the same `Probe` |
| **Secrets**: the login Keychain through `security-framework` | `secrets.rs` | Secret Service (`libsecret`) behind `SecretStore` | Credential Manager behind `SecretStore` |
| **Time zone label**: `localtime_r`'s `tm_zone` | `status_rules/mod.rs` `zone_of` | Works | No `tm_zone`: format the zone from the Windows API |
| **Joining**: `open -b`, a Chrome web app through `--app-id` | `calls/join.rs` | `xdg-open`; Chrome's `--app-id` works the same | `start`; same for Chrome |
| **Launch at login**: `MacosLauncher::LaunchAgent` with `--hidden` | `app.rs` (autostart plugin), `autostart.rs` | The plugin writes an XDG autostart entry | The plugin writes a registry key |
| **Shortcuts**: `Mod` is ⌘, labels use ⌘⌥⇧ glyphs | `src/commands/registry.ts`, `store/keybindings.rs` `key_label`, `menu/mod.rs` | Map `Mod` to Ctrl | Same |
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
| **Overlay panel**: a non-activating `NSPanel` by class swap, Space behaviour, `NSScreen`/`NSEvent` for the pointer's screen, `NSWorkspace` and the window list for focus hand-back | `window/native.rs`, `window/panel/native.rs` | An always-on-top undecorated window; no Spaces | A tool window with `WS_EX_NOACTIVATE` |
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
