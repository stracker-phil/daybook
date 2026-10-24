# Daybook

My menu-bar companion for my Obsidian vault. It guides my day (status, focus, routines, journal, meetings,
reminders) and gives me fast access to the vault (quick notes, search, open todos, links), while Obsidian stays
a plain editor: Daybook only adds lines to known sections of a note and never rewrites one. It does not track
time; my TimeTracker app does that, and Daybook drives it.

It is a local tool for one person on one Mac. Published as source, as-is: no releases, no binaries, no
support. To build it or port it, point a coding agent at [AGENTS.md](AGENTS.md).

> Work in progress: Daybook is being ported from a prototype step by step. Right now it is the empty shell: a
> menu-bar icon, a window with Settings, the command palette and the CLI below.

## Install

```sh
bun install
bun run release
```

This builds `Daybook.app`, links it into `~/Applications` and the `daybook` command into `~/.local/bin`. It runs
on macOS on Apple Silicon only.

## CLI

`daybook` without arguments starts Daybook or brings it to the front; every subcommand asks the running app.

| Command | Does |
|---|---|
| `daybook --hidden` | start without opening the window (launch at login uses it) |
| `daybook show` | show the window without taking focus, starting Daybook if needed |
| `daybook focus` | show the window and bring it to the front, starting Daybook if needed |
| `daybook hide` | hide the window; Daybook keeps running in the menu bar |
| `daybook quit` | quit Daybook |
| `daybook status` | the version and whether the window shows; `--json` prints the reply as JSON |

`daybook://show` opens the window from a link.

## Docs

[docs/README.md](docs/README.md) is the index of the technical documentation.

## License

MIT, see [LICENSE](LICENSE).
