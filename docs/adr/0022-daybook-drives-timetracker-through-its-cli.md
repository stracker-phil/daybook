# 0022. Daybook drives TimeTracker through its CLI and never tracks time itself

## Status

Accepted (2026-10-27), implemented.

## Context

The prototype logged activities into the daily note, asked for check-ins and computed day recaps. TimeTracker now
owns all of that (README decisions 1 to 3 and 5). Daybook still needs to know what is being timed (menu-bar title,
Slack text, link rules) and to start, stop and suggest timers.

## Decision

- Daybook talks to TimeTracker only through the `timetracker` command line tool (`--json` replies, exit codes), and
  follows it with `timetracker watch`; it never reads TimeTracker's database or files.
- Daybook records no time and computes no durations of its own. It may show TimeTracker's numbers read-only.
- The client sits behind one trait so tests use a fake; debug builds never send commands that change TimeTracker's
  data, so development can't touch the owner's real timers.

## Consequences

- TimeTracker's CLI is a contract: new needs are added there as general features (step 23).
- When TimeTracker isn't running, Daybook keeps working and says so; nothing is queued.
