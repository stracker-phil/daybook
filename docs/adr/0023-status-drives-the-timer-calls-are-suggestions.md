# 0023. The status drives the timer; calls are suggestions

## Status

Accepted (2026-10-27), implemented. The calls part is superseded by
[ADR 0029](0029-every-call-goes-to-timetracker.md): every call is sent, without a project when none is linked.

## Context

README decisions 1 and 3: Daybook owns the status (Working, Break, Off) and TimeTracker owns time. The owner wants
Working to resume what they were timing and Break and Off to stop it, without a second place to switch. Calls used to
be logged into the note as time; now they are only evidence of what happened.

## Decision

- A status change starts or stops TimeTracker's timer through its CLI: Working continues the last timer (or asks with
  the activity overlay when it doesn't fit the context), Break and Off stop it. TimeTracker's timer changing never
  changes Daybook's status.
- A finished call whose title names a linked project becomes a stored suggestion in TimeTracker (`record --source
  Daybook`) for the owner to approve; Daybook never records time itself ([ADR 0022](0022-daybook-drives-timetracker-through-its-cli.md)).
- Both can be turned off: Settings → Workflows → "Start and stop the timer".

## Consequences

- When TimeTracker isn't running, the status still changes and a notification says the timer wasn't changed.
- A call made while a timer runs is stored, but TimeTracker shows only its uncovered part.
