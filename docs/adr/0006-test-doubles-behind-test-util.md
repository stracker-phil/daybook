# 0006. Test doubles ship behind a `test-util` feature

## Status

Accepted (2026-10-24), implemented. Inherited from TimeTracker ADR 0015.

## Context

Unit tests need temp config dirs today and will need fake vaults and fakes for other apps (calendar, Slack,
TimeTracker) later. Integration tests in `src-tauri/tests/` are a separate crate and cannot see
`#[cfg(test)]` items. Compiling the doubles unconditionally would put dead code into the release binary.

## Decision

Doubles (`test_support::temp_config_dir`, later fakes and fixture builders) compile under
`cfg(any(test, feature = "test-util"))`. The crate lists itself as a dev-dependency with the `test-util`
feature, so integration tests get the doubles while release builds never include them.

## Consequences

- Doubles are available wherever tests run and absent from the app.
- Every new double uses the same `cfg` gate.
- The self dev-dependency is an unusual Cargo pattern; `Cargo.toml` comments point here.
