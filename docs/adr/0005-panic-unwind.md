# 0005. Keep `panic = "unwind"`

## Status

Accepted (2026-10-24), implemented. Inherited from TimeTracker ADR 0014.

## Context

Daybook runs in the menu bar for days and will do work in the background: reminders, routines, presence and
call detection. With `panic = "abort"` (the Tauri template default) a panic in any command or task kills the
process, and the day's guidance stops until the owner notices. Tauri and tokio isolate unwinding panics per
command and per task.

## Decision

Use the default unwinding strategy. Runtime-reachable `unwrap`/`expect` calls with a plausible failure become
an `AppError` or a fallback. `std::sync` locks are taken through `lock::MutexExt::lock_unpoisoned` and
`RwLockExt::{read,write}_unpoisoned`, so a poisoned lock keeps working. Every long-running background task
is supervised: when it ends with a panic, it is logged and restarted after a short delay.

## Consequences

- A failing command returns an error; the app survives bugs in one view.
- Slightly larger binary (unwind tables).
- Guarded sections are kept to single updates so a panic cannot leave state inconsistent.
