# 0002. IPC only through `api.ts` files

## Status

Accepted (2026-10-24), implemented. Inherited from TimeTracker ADR 0002.

## Context

UI tests need to replace the backend. `invoke` and `listen` calls scattered over components make that
impossible without mocking Tauri itself, and they hide the contract between page and backend. Daybook will
have several windows (overlays, the rail, the quick notes editor) that share that contract.

## Decision

Only API files import `@tauri-apps/api` or a `@tauri-apps/plugin-*` package: the core's `src/api/index.ts`,
and one `src/<module>/api.ts` per optional module with its own folder. Everything else imports the `api`
object; other windows import the same file. A boundary test (`src/test/boundary.test.ts`) fails on any other
file that imports Tauri. Tests mock exactly that module with `createApiMock()`.

## Consequences

- Tests mock one module per backend; the contract is visible in one place.
- Every command needs a thin hand-written wrapper.
- Event subscriptions (`api.onLaunch`, `api.onCommand`, `api.onRemote`, …) live in the same file, so tests
  can drive them.
