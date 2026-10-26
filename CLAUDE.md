# Daybook

The owner's menu-bar companion for the Obsidian vault `~/Obsidian/ThoughtForge`: it guides the day (status,
focus, routines, journal, meetings, reminders) and gives fast access to the vault (quick notes, search, open
todos, links), while Obsidian stays a plain editor. Daybook does not track time; TimeTracker does.

TimeTracker (`../timetracker`) is the blueprint for architecture, process and the
look: the same stack, conventions, docs layout and design system. Daybook differs only where its job differs;
each deviation has an ADR.

## Rules

- Every session or step ends in a commit, so it can be reverted. Commit each finished part of a step as soon as
  it is green, not only at the end.
- Never push. Only commit locally.
- Write unit tests for every change and run them (through the `ci` agent) before every commit; tests and
  lint must be green.
- Tests pin behavior, not internals: public interfaces only, no log assertions, no call counts.
- Refactor continuously: when a file grows large (over ~400 lines of production code) or complex, simplify it
  and split it into focused modules in the same step, without asking.
- Never write to the vault, the Glaze prototype or its data folder outside a step that says so; tests use temp
  folders.

## Plan
The port of the Glaze prototype lives in `docs/plan/` (gitignored): `README.md` (product, decisions,
deviations, step list) plus one file per step and `reference/`. Read the plan's README first, every time. Work
one step per session, in its order; when a step is done, set its status from PENDING to DONE in
`docs/plan/README.md` (the only change a step makes there). Report back item by item: what was built, what was
left out and why, open questions. Ask before assuming a workflow the prototype does not show.

## Docs
Technical docs live in `docs/` (index: `docs/README.md`). Read `docs/architecture.md` and the page of the
layer you change before changing it; `docs/adapting.md` lists everything macOS- or name-specific. Decisions
are in `docs/adr/` (never edited once accepted; a new record supersedes). Update the affected docs page in the
same commit when behavior, commands, files or extension points change. Document by file and symbol, never by
line number.

## Stack
- Rust backend in `src-tauri/` (rustup stable; Homebrew rustup is keg-only: `/opt/homebrew/opt/rustup/bin`)
- Tauri 2 + React 19 + TypeScript in `src/`, bun as package manager, zustand, lucide-react
- macOS 27, Apple Silicon only

## Architecture
- Rust owns all logic; React only renders. Only `api.ts` files call `invoke`/`listen` (`src/api/index.ts`
  for the core). Tests mock them.
- `src-tauri/src/`: `lib.rs` (startup), `instance.rs` (handoff to the running app), `app.rs` (Tauri builder),
  `handlers.rs` (command registration), `cli/` (argv, replies), `ipc.rs` (single-instance socket),
  `pending.rs`, `launch.rs`, `remote.rs` (requests the page answers), `deep_link.rs`, `events.rs` (every event
  name), `state.rs` (`AppState`), `store/` (JSON files: settings, keybindings, ui state), `autostart.rs`,
  `commands.rs`, `bus/` (the command bus, docs/commands.md), `contexts/`, `projects/`, `status/` (Working, Break, Off; docs/status.md), `calendar/` (EventKit, the day's events; docs/calendar.md), `reminders/`, `presence/` (away), `calls/` (call detection, joining; docs/presence-and-calls.md), `slack/` (Slack status, channel messages, the day's message; docs/slack.md), `focus/` (the daily focus list), `journal/` (journal groups, progress, the overlay's commands; docs/vault.md#journal), `routines/` (checklists in the plan section, the fired triggers; docs/routines.md), `todos/` (the daily-notes index: open todos, days with a note; docs/vault.md#open-todos), `quick_notes/` (topics of today's notes for the editor window; docs/vault.md#quick-notes), `search/` (the vault index, ranking, scopes, the note preview; docs/search.md), `wizard/` (opens routines and journal groups by itself), `scheduler.rs` (the clock tick), `status_rules/` (absence heads-ups), `net.rs` (the only HTTP client, ADR 0020), `secrets.rs` (Keychain), `tools/`, `hub/` (the Daybook window's view model), `palette/`, `shortcuts/` (global shortcuts),
  `menu/` (app menu, menu-bar menu model), `help.rs`, `tray.rs`, `notify.rs`, `window/` (Daybook window, Settings window, overlay, quick-notes editor), `error.rs`,
  `lock.rs`, `dev.rs` (debug builds), `test_support/` (tests).
- Errors cross the boundary as `{ kind, message }` (`error.rs`). IPC types are generated with ts-rs.
- The data is the vault plus small JSON files; no database (ADR 0012).
- Frontend state: zustand store in `src/state/store.ts`; components in `src/components/`.

## Commands
Test, lint and format-check commands run through the `ci` agent, never in the main context.
- `bun run test`: Vitest and Rust unit tests
- `bun run lint`: IPC types drift check (`src/api/types.gen.ts`), tsc, ESLint and clippy on production code (warnings are errors)
- `bun run release`: release `.app` and `.dmg` in `src-tauri/target/release/bundle/`, signed, then installed
- `scripts/install.sh` (the end of `bun run release`): signs the release build (`scripts/sign-app.sh`), links it
  as the app and the `daybook` command and restarts a running Daybook on it; debug builds are never linked.
  `scripts/create-signing-cert.sh` (once per Mac) makes the certificate that keeps privacy grants
- `bun run build:app` + `scripts/dev/launch.sh`: debug build, restarted next to the terminal without stealing focus
- `scripts/dev/js.sh` / `scripts/dev/screenshot.sh`: drive (store: `window.daybook`) and capture the running window in the background

## Workflow
Finish every step with the `ship-step` skill. Rules in `.claude/rules/`; `testing.md` says which agents in
`.claude/agents/` write and run tests.
