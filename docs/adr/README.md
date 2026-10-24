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
| [0003](0003-errors-as-kinds.md) | Errors cross IPC as `{kind, message}` | Accepted (2026-10-24), implemented |
| [0004](0004-generated-ipc-types.md) | TypeScript IPC types are generated from Rust with ts-rs | Accepted (2026-10-24), implemented |
| [0005](0005-panic-unwind.md) | Keep `panic = "unwind"` | Accepted (2026-10-24), implemented |
| [0006](0006-test-doubles-behind-test-util.md) | Test doubles ship behind a `test-util` feature | Accepted (2026-10-24), implemented |
| [0007](0007-color-palettes.md) | Color schemes are complete token sets; no color literal outside a palette | Accepted (2026-10-24), implemented |
| [0008](0008-cli-asks-the-running-app.md) | The CLI is a second process that asks the running app over the socket | Accepted (2026-10-24), implemented |
| [0009](0009-three-schemes-four-palettes.md) | Appearance is one of three schemes built from four palettes | Accepted (2026-10-24), implemented |
| [0010](0010-a-scheme-per-system-mode.md) | Appearance follows macOS, with one scheme per mode | Accepted (2026-10-24), implemented |
| [0011](0011-native-macos-look.md) | The app looks native | Accepted (2026-10-24), implemented |
| [0012](0012-vault-and-json-no-database.md) | Data is the vault plus small JSON files; no database to start | Accepted (2026-10-24), implemented |
| [0013](0013-one-settings-model-imported-once.md) | Settings are one JSON model; the prototype is imported once | Accepted (2026-10-24), implemented |
| [0014](0014-debug-build-has-its-own-data.md) | The debug build has its own data folder and socket | Accepted (2026-10-24), implemented |
