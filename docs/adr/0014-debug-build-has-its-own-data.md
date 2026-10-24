# 0014. The debug build has its own data folder and socket

## Status

Accepted (2026-10-24), implemented. Deviates from TimeTracker, where both builds share one folder.

## Context

The release app runs all day; the debug build is started and stopped many times while a step is built. With
one config folder both builds share the socket, the lock and every data file: a debug launch hands off to a
running release app (or stops it), and a debug build with half-finished code reads and writes the owner's
settings and state.

## Decision

Debug builds use their own identifier, `com.philippstracker.daybook.debug` (`IDENTIFIER` in `lib.rs`, the
same as `tauri.debug.conf.json`). Everything that hangs off it is separate: the config folder with its socket,
lock and JSON files, the logs, macOS privacy grants and the launch-at-login agent. The release app keeps
running while the debug app runs; `scripts/stop-app.sh` stops only the build it is given, `scripts/dev/js.sh`
talks to the debug socket, and the debug app is driven through its own binary.

## Consequences

- Trying a change never touches the owner's settings or state, and both apps can run side by side.
- The debug app starts from its own data: on its first launch it imports the prototype into its own folder
  (ADR 0013), later it keeps whatever the debug runs saved.
- The vault is shared: steps that write to it still need care in debug runs.
