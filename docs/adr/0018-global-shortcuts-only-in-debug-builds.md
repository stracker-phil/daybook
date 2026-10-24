# 0018. Release builds register no global shortcuts while the prototype runs

## Status

Accepted (2026-10-24), implemented. Temporary: the step that retires the prototype supersedes it.

## Context

Both Daybook builds imported the prototype's global shortcuts (⌘F1, ⌃⌥⌘F, …). The prototype keeps running
until Daybook reaches parity, and the installed release app runs all day next to it. If the release app,
the debug app and the prototype all claimed the same keys, whichever registered first would win and the
owner's daily tool could lose its keys.

## Decision

`shortcuts::REGISTERED` is true only in debug builds. Release builds register no global shortcuts and the
menus show no global shortcut hints; the debug app registers them while it runs, for testing.

## Consequences

- The prototype keeps its keys; the installed Daybook is reached through the menu bar, the app menu and the
  palette until the prototype retires.
- While the debug app runs it may take keys from the prototype; quitting it gives them back.
