# 0024. Daybook shows TimeTracker's day read-only

## Status

Accepted (2026-10-27), implemented.

## Context

The prototype's Day Recap and activity log kept their own record of the day. Since TimeTracker owns time
([ADR 0022](0022-daybook-drives-timetracker-through-its-cli.md)), a second record would drift from it. The owner
still wants to see the day next to the todos and copy notes and durations for timesheets.

## Decision

- The inspector's Activity tab shows TimeTracker's recordings and suggestions for the shown day, read through
  `recordings` and `suggestions --json`, as a timeline and grouped by project.
- Daybook never edits, approves or deletes recordings there; "Open in TimeTracker" hands off.
- Day Recap is not ported; "Day Activity" opens the tab instead.

## Consequences

- The tab is empty while TimeTracker isn't running and says so.
- Rust follows TimeTracker's `watch` stream to tell the page when to reload.
