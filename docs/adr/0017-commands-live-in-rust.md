# 0017. Commands live in Rust

## Status

Accepted (2026-10-24), implemented. Deviates from TimeTracker, where the page's registry owns the commands
and the native menu forwards its clicks to the page.

## Context

Daybook's actions are run from many places that are not the page: the menu-bar menu, the app menu, global
shortcuts, the overlay palette, and later the command line, `daybook://` links, the HTTP API, the rail and
Stage. Most of them run while the Daybook window is hidden, so a page-owned command would need the main
window's webview to run it; and the same id must mean the same action everywhere.

## Decision

- Every app-level action is a command on one Rust bus (`bus/`): an id, a title and help line, flags (hidden,
  needs a vault, quits) and an optional enabled check. Families take an argument after a prefix
  (`mode:<context>`). Features register their commands; callers run them by id and never import a feature.
- Menus are built from plain models whose items carry command ids; an item whose command no feature
  registered is left out, a disabled one is greyed. The palette lists the same model.
- The page keeps TimeTracker's registry only for keys inside one window (Esc, list keys, zoom). A window-level
  action the page offers runs the Rust command through `run_command`; it is never written twice.

## Consequences

- Each later step adds its commands in Rust, and they appear in the menus and the palette by themselves.
- The app menu's keys come from one table (`menu/command_shortcuts.rs`); rebinding them comes with the
  Shortcuts page over the Rust commands.
- A command that fails with nobody waiting (a menu click, a global shortcut) is reported as a notification.
