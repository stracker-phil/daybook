# 0003. Errors cross IPC as `{kind, message}`

## Status

Accepted (2026-10-24), implemented. Inherited from TimeTracker ADR 0003.

Amended (2026-10-27): later steps added the kinds `network` ([ADR 0020](0020-network-only-to-listed-hosts.md))
and `timetracker_not_running`, `timetracker_refused`, `timetracker_missing`
([ADR 0022](0022-daybook-drives-timetracker-through-its-cli.md)); the page's known kinds are `ERROR_KINDS` in
`src/api/index.ts`.

## Context

The page reacts to classes of errors: a cancelled panel is no failure, a missing note offers to create it, an
invalid field marks that field. Parsing English messages for that is fragile. TimeTracker's error also
carried a database result `code` and conflict `details`; Daybook needs neither.

## Decision

`AppError` serializes to a stable `kind` (`io`, `invalid_input`, `not_found`, `cancelled`), the display
`message` and, for form errors (`InvalidField`), `field: { path, message }` (`ErrorJson` in `error.rs`). The
UI branches on `kind` only; `toAppError` accepts only known kinds and turns anything else into `io`. The
socket carries the same pair as `IpcError { kind, message }`, so `daybook --json` prints it. A new kind is
added when the page has to branch on it, not for every new failure.

## Consequences

- UI flows that depend on the class of an error are plain branches, testable with a mock.
- Adding a kind touches both sides (`ErrorKind` and the page's known kinds).
- Messages may change freely without breaking the UI.
