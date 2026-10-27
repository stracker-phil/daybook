# Architecture decision records

Each record (`NNNN-<slug>.md`, Nygard format: Status, Context, Decision, Consequences) explains why, in at
most a page; the docs pages explain how. An accepted decision is never edited: a new record supersedes it,
and a dated amendment under Status only corrects facts that changed.

Records inherited from TimeTracker say so under Status with TimeTracker's number; they state the decision as
it stands in Daybook. The numbers in this table are Daybook's.

| No. | Title | Status |
|---|---|---|
| [0001](0001-rust-owns-the-logic.md) | Rust owns the logic, React only renders | Accepted (2026-10-24), implemented |
| [0002](0002-ipc-in-api-files.md) | IPC only through `api.ts` files | Accepted (2026-10-24), implemented |
| [0003](0003-errors-as-kinds.md) | Errors cross IPC as `{kind, message}` | Accepted (2026-10-24), implemented; amended 2026-10-27 |
| [0004](0004-generated-ipc-types.md) | TypeScript IPC types are generated from Rust with ts-rs | Accepted (2026-10-24), implemented |
| [0005](0005-panic-unwind.md) | Keep `panic = "unwind"` | Accepted (2026-10-24), implemented |
| [0006](0006-test-doubles-behind-test-util.md) | Test doubles ship behind a `test-util` feature | Accepted (2026-10-24), implemented |
| [0007](0007-color-palettes.md) | Color schemes are complete token sets; no color literal outside a palette | Accepted (2026-10-24), implemented; partly superseded by [0026](0026-the-daybook-window-follows-the-prototypes-look.md) |
| [0008](0008-cli-asks-the-running-app.md) | The CLI is a second process that asks the running app over the socket | Accepted (2026-10-24), implemented |
| [0009](0009-three-schemes-four-palettes.md) | Appearance is one of three schemes built from four palettes | Accepted (2026-10-24), implemented; partly superseded by [0026](0026-the-daybook-window-follows-the-prototypes-look.md) |
| [0010](0010-a-scheme-per-system-mode.md) | Appearance follows macOS, with one scheme per mode | Accepted (2026-10-24), implemented |
| [0011](0011-native-macos-look.md) | The app looks native | Accepted (2026-10-24), implemented; partly superseded by [0026](0026-the-daybook-window-follows-the-prototypes-look.md), [0028](0028-overlays-follow-the-prototype.md) |
| [0012](0012-vault-and-json-no-database.md) | Data is the vault plus small JSON files; no database to start | Accepted (2026-10-24), implemented |
| [0013](0013-one-settings-model-imported-once.md) | Settings are one JSON model; the prototype is imported once | Accepted (2026-10-24), implemented |
| [0014](0014-debug-build-has-its-own-data.md) | The debug build has its own data folder and socket | Accepted (2026-10-24), implemented |
| [0015](0015-one-write-path-into-the-vault.md) | Every vault write goes through the note writer; the app never touches frontmatter | Accepted (2026-10-24), implemented |
| [0016](0016-overlays-never-navigate.md) | Overlays are a non-activating panel that never navigates | Accepted (2026-10-24), implemented |
| [0017](0017-commands-live-in-rust.md) | Commands live in Rust | Accepted (2026-10-24), implemented |
| [0018](0018-global-shortcuts-only-in-debug-builds.md) | Release builds register no global shortcuts while the prototype runs | Accepted (2026-10-24), implemented |
| [0019](0019-daybook-owns-the-status.md) | Daybook owns the status; it is never read from the note | Accepted (2026-10-24), implemented |
| [0020](0020-network-only-to-listed-hosts.md) | Daybook talks to Slack (and later Apple's catalog), nothing else | Accepted (2026-10-25), implemented |
| [0021](0021-a-local-http-api-with-guards.md) | A local HTTP API, off by default and guarded | Accepted (2026-10-27), implemented |
| [0022](0022-daybook-drives-timetracker-through-its-cli.md) | Daybook drives TimeTracker through its CLI and never tracks time itself | Accepted (2026-10-27), implemented |
| [0023](0023-status-drives-the-timer-calls-are-suggestions.md) | The status drives the timer; calls are suggestions | Accepted (2026-10-27), implemented |
| [0024](0024-daybook-shows-timetrackers-day-read-only.md) | Daybook shows TimeTracker's day read-only | Accepted (2026-10-27), implemented |
| [0025](0025-quick-note-presentation-covers-one-screen.md) | Quick note presentation covers one screen, not macOS full screen | Accepted (2026-10-27), implemented |
| [0026](0026-the-daybook-window-follows-the-prototypes-look.md) | The Daybook window follows the prototype's look | Accepted (2026-10-27), implemented; partly supersedes 0007, 0009, 0011 |
| [0027](0027-several-windows-on-a-native-window-layer.md) | Several windows on a native window layer | Accepted (2026-10-27), implemented |
| [0028](0028-overlays-follow-the-prototype.md) | Overlays follow the prototype's look; the activity is not the timer | Accepted (2026-10-27), implemented; partly supersedes 0011 |
