# 0008. The CLI is a second process that asks the running app over the socket

## Status

Accepted (2026-10-24), implemented. Inherited from TimeTracker ADR 0017.

## Context

The owner drives Daybook from scripts, launchers and other tools: show the window, ask its state, and later
run its commands. The app holds the settings stores and, later, the only path that writes to the vault; a CLI
that wrote files itself would race the app and bypass that path.

## Decision

`daybook <subcommand>` parses with clap into an `IpcMessage`, connects to `daybook.sock` in the config dir,
sends one JSON line, prints the reply and exits; the socket is 0600 and the app refuses peers of another
user. The running app answers every request through the same functions the UI uses (`remote::handle` maps
each message to one module function). `show`, `focus` and a bare `daybook` start the app when none runs;
every other subcommand exits 2 with "Daybook is not running". `--json` prints the reply as JSON for scripts.

## Consequences

- One implementation of every action; the CLI is a thin table of mappings.
- The CLI needs the app running for anything but starting it.
- Any process that speaks the line protocol can integrate without the CLI binary.
