# 0030. Global shortcuts only in release builds

## Status

Accepted (2026-10-27), implemented. Supersedes [0018](0018-global-shortcuts-only-in-debug-builds.md).

## Context

ADR 0018 kept the global shortcuts in the debug build while the Glaze prototype ran next to the installed app.
The installed Daybook now replaces the prototype, and the owner found it confusing when the debug app, started
for development, took the keys of the app they use all day.

## Decision

`shortcuts::REGISTERED` is true only in release builds. The installed app registers the global shortcuts from
Settings and the menus show their hints; the debug build registers none and shows none. Development runs the
commands through the CLI, the palette or `scripts/dev/js.sh`.

## Consequences

- Settings → Shortcuts in the debug build says the keys take effect in the installed app; recording and saving
  still work there.
- Until the prototype quits, whichever app registers a key first keeps it; a key Daybook couldn't get is logged.
