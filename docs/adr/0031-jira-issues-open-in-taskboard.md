# 0031. Jira issues open in Taskboard, the browser is the fallback

## Status

Accepted (2026-10-29), implemented.

## Context

Every Jira issue opened in the browser at `jira.baseUrl` + the key. The owner now has Taskboard, a local Jira
client with a command line tool (`taskboard open <KEY>` shows the issue and starts the app when needed). Taskboard
tracks only some Jira spaces, and may be missing on another Mac.

## Decision

- Every way of opening an issue (the overlay, `jira:issue:<key>`, the CLI, links, the HTTP API) runs
  `taskboard open <KEY>` first, found like `plaunch` in `~/.local/bin`, `/usr/local/bin` or `/opt/homebrew/bin`.
  No setting for its path.
- When Taskboard is missing or fails (a space it doesn't track, a timeout), the issue opens in the browser if the
  Jira URL is set; the action log says why. Without a URL the error says what is missing.
- Daybook talks to Taskboard only through its CLI, as it does with TimeTracker
  ([0022](0022-daybook-drives-timetracker-through-its-cli.md)).

## Consequences

- The Jira URL stays in Settings → Connections → Jira as the browser fallback; Settings → Status shows whether
  Taskboard is installed.
- An issue Taskboard doesn't know costs one failed `taskboard open` before the browser opens.
