# 0019. Daybook owns the status; it is never read from the note

## Status

Accepted (2026-10-24), implemented.

## Context

In the prototype the status was partly read back from the daily note: `noteState()` derived the status, its
start, whether you worked today and the last activity from the activity section, and each status change could
write a line there. Time tracking moved to TimeTracker, and the activity section with it (plan decisions 1
and 5). Reading the status from lines that no longer exist, or from frontmatter the app must not touch
([ADR 0015](0015-one-write-path-into-the-vault.md)), isn't possible any more.

## Decision

- The status (Working, Break, Off) lives only in `status-memory.json`: date, status, since and whether you
  worked today. A new day starts without a status.
- A status change writes nothing to the note. It is announced as `status_changed`, and the features that
  react (Slack, focus, the wizard, music, later the timer) listen to that event.
- One rule decides every effect: picking the status you have changes nothing; the first status of a day
  always counts.

## Consequences

- Editing the note can't change the status, and the status can't disturb the note.
- A status set elsewhere (by the prototype, by hand in a note) isn't seen; the owner sets it in Daybook.
- Effects stay out of the status module: each later step adds a listener instead of changing `set_status`.
