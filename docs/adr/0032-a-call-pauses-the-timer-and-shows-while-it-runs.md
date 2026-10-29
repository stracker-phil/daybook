# 0032. A call pauses the timer and shows in TimeTracker while it runs

## Status

Accepted (2026-10-29), implemented. Supersedes when [0029](0029-every-call-goes-to-timetracker.md) sends a call
(once it is final) and the last consequence of [0023](0023-status-drives-the-timer-calls-are-suggestions.md).

## Context

A call reached TimeTracker only after it ended plus the 2-minute restart wait, and the timer kept running through
it. TimeTracker shows only the part of a suggestion no recording covers, so a call made while a timer ran mostly
never showed, and during the call TimeTracker still showed the timer from before. The owner wants the call visible
while it runs and the timer to reflect it.

## Decision

- **The call pauses the timer.** A call that starts while Working, in a context with the Status feature and with
  "Start and stop the timer" on, stops the running timer. When the call ends, Daybook starts the same project, note
  and tag again (TimeTracker continues today's recording), unless the status is no longer Working or another timer
  runs by then. A restart within 2 minutes pauses it again.
- **The suggestion grows while the call runs.** Once the call passes the checks (Working, the Status feature, 3
  minutes), Daybook records the suggestion from the call's start to now, moves its end every minute
  (`edit ID --span`), and at the call's end moves it to the end. When the call is final it gets its final note and
  project; one that no longer passes (the status changed) is dismissed (`delete ID`).
- The minute updates stay out of the action log; the first record, the final update and a dismissal are logged.

## Consequences

- The call shows in TimeTracker's Day view within a few minutes and is no longer hidden behind the timer.
- A call under 3 minutes still sends nothing, but it does pause the timer, so that time is left out of the recording.
- The pause is kept in memory: when Daybook quits during a call, the timer isn't resumed.
