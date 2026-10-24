# Testing and dev tooling

## Commands

Coding agents don't run these themselves. They hand test writing to the `write-ts-unit-test` and
`write-rust-unit-test` agents and runs to the `ci` agent (`.claude/agents/`), so test code and test output
stay out of the main context ([`.claude/rules/testing.md`](../.claude/rules/testing.md)).

| Command | Runs |
|---|---|
| `bun run test` | Vitest UI tests (`test:ui`), then the Rust tests (`test:rust`) |
| `cargo test --manifest-path src-tauri/Cargo.toml -- --ignored` | the slow `#[ignore]` tests on temp folders (none yet); the `ci` agent's integration suite |
| `bun run lint` | regenerates the IPC types (`types`: the ts-rs `export_bindings` tests) and fails when `src/api/types.gen.ts` changed, then `tsc --noEmit` (app and tests), `eslint .` (`eslint.config.js`: `@eslint/js` and type-checked `typescript-eslint` recommended plus `react-hooks/rules-of-hooks` and `exhaustive-deps`; test files and `apiMock.ts` turn off the rules that fight mocks) and `cargo clippy --lib --bins -- -D warnings` (production code only) |
| `cargo fmt --manifest-path src-tauri/Cargo.toml` | Rust formatting (run before lint) |

The project rules for tests:
- Every change ships with tests of its observable behavior, and they run green before every commit.
- Tests pin behavior, not internals: command results, CLI parse results, stored files, rendered UI.
- No log assertions, no call counts, no private helpers.
- Tests never touch the vault, the real config dir or the network; they use temp folders.

## Agents

| Agent | Does |
|---|---|
| `write-rust-unit-test` | writes Rust unit tests (`mod tests`) and integration tests; never runs them |
| `write-ts-unit-test` | writes Vitest tests next to components, mocking only `src/api`; never runs them |
| `ci` | runs one suite (UI, Rust unit, integration, lint, format check, all unit tests) and replies with a pass/fail summary |
| `review-code-cleanliness` | reviews a changed file for structure, comment noise, boundaries and over-engineering |

The `ship-step` skill (`.claude/skills/ship-step/`) chains cleanup, review, `cargo fmt`, `ci`, build,
launch and commit at the end of every step.

## Rust

- **Unit tests** sit next to the code in `#[cfg(test)] mod tests`. Stores and sockets use `tempfile`
  directories; nothing touches the real config dir, the vault or the network.
- **Helpers** are in `src/test_support/` (`fixtures.rs`): `temp_config_dir`, a fresh 0700 temp dir standing
  in for the config dir. They compile with `cfg(test)` or the `test-util` feature
  ([ADR 0006](adr/0006-test-doubles-behind-test-util.md)); the crate lists itself as a dev-dependency with
  `test-util`, so integration tests in `src-tauri/tests/` (none yet) see them.
- **Prototype oracles:** when a step ports a prototype module, its Vitest tests (`P/tests/`) become Rust tests
  of the ported module and are its acceptance test.
- **Socket tests** (`ipc.rs`) bind a socket in a temp dir, serve it with a handler that answers or records
  messages, and check the wire shape, acknowledgements, replies, oversized and malformed lines and concurrent
  clients.
- **Generated types:** each `#[ts(export)]` type gets an `export_bindings_*` test that writes
  `src/api/types.gen.ts` (`bun run types`).
- **Contracts:** `identifier_matches_the_tauri_config` (`lib.rs`) compares `IDENTIFIER` with
  `tauri.conf.json`; `accents_match_the_frontend_list` (`store/settings/mod.rs`) compares `ACCENTS` with
  `src/api/types.ts`; `src/commands/menu.test.ts` compares `MENU_ITEMS` with the page's commands and their default bindings.

## Frontend

The setup, mocking and fixtures are described in [frontend.md](frontend.md#tests). In short:
- Tests are `*.test.ts(x)` files next to their code; `src/test-setup.ts` registers the core scopes and
  settings pages before each test and pretends to be a Mac.
- They mock the `src/api` module with `createApiMock()` (`src/test/apiMock.ts`); `src/test/settings.ts`
  builds `Settings` values.
- They use Testing Library queries by role or text and `userEvent`.
- Guards: `src/test/boundary.test.ts` (Tauri imports only in `api.ts` files), `src/styles/colors.test.ts`
  (no color literal outside the palettes, matching palettes) and `src/styles/contrast.test.ts` (text contrast
  per palette).

## Driving the app

The dev scripts work without stealing focus, so the app can run on a second screen while you or an agent
keep typing in the terminal. They are macOS-only (zsh, Swift, `screencapture`, and `python3` in `js.sh`).

| Script | Does |
|---|---|
| `bun run build:app` | builds the debug `.app` bundle (`src-tauri/target/debug/bundle/macos/Daybook (Debug).app`, named apart by `tauri.debug.conf.json`) |
| `scripts/dev/launch.sh [cli args]` | signs the debug bundle, stops a running `daybook` and starts the debug build next to the terminal window, without focus. The log goes to `$TMPDIR/daybook-dev.log`. The terminal app is set by `DAYBOOK_DEV_TERMINAL` (default `Terminal`) |
| `scripts/dev/js.sh '<js>'` | runs JavaScript in the running debug app over the IPC socket (`IpcMessage::Eval`, debug builds only). Fire-and-forget: no return value, and top-level `const` declarations collide across calls, so wrap them in an IIFE. A first line `@<label>` targets another window |
| `scripts/dev/screenshot.sh out.png` | captures the largest Daybook window, also while it is in the background |
| `scripts/dev/screenshot-looks.sh <dir>` | captures the window in each scheme of the current macOS mode as `<scheme>-<mode>.png`, then restores the settings; switch the system appearance and run it again for the other mode ([ux.md](ux.md#scheme-audit)) |
| `bun run app` | builds and `open`s the app; this takes focus |
| `bun run release` | builds the release `.app` and `.dmg` (`src-tauri/target/release/bundle/`), signed ad hoc, then installs it (`scripts/install.sh`) |

The store is available as `window.daybook` in the debug bundle: `bun run build:app` sets
`VITE_DEV_HOOKS=1` at build time, and `main.tsx` exposes the store only then. A release build has no
`window.daybook`.

```sh
scripts/dev/js.sh 'daybook.getState().openDialog({ type: "settings", page: "shortcuts" })'
scripts/dev/js.sh 'daybook.getState().toggleSidebar()'
```

`launch.sh` sets `DAYBOOK_WINDOW_FRAME` and `DAYBOOK_BACKGROUND=1`. Rust reads both in `dev.rs`, a module
compiled into debug builds only. JavaScript eval over the socket is compiled out of release builds as well.
Never use AppleScript clicks or keystrokes to drive the app; they need focus.

### Signing

macOS keys privacy grants (Accessibility, Automation, Calendars, which later steps need) to the app's
designated requirement: with a certificate that is the identifier and the certificate, so a grant survives
rebuilds; ad hoc it is the build's hash, so every rebuild asks again. `scripts/create-signing-cert.sh` creates
the self-signed "Daybook Local Signing" certificate once per Mac; `scripts/sign-app.sh` signs with
`DAYBOOK_SIGN_IDENTITY`, else that certificate, else ad hoc. `launch.sh` signs the debug bundle
(`com.philippstracker.daybook.debug`), `scripts/install.sh` the release bundle
(`com.philippstracker.daybook`). The two are separate entries in Privacy & Security, each granted once; both
share the config dir.
