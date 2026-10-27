# 0029. Every call goes to TimeTracker, with or without a project

## Status

Accepted (2026-10-27), implemented. Supersedes the calls part of [0023](0023-status-drives-the-timer-calls-are-suggestions.md).

## Context

ADR 0023 sent a finished call to TimeTracker only when its title named a project linked to TimeTracker; every other
call was dropped. The owner wants complete tracking data that is easy to review: a call that isn't in TimeTracker is
time missing from the day. TimeTracker now stores a suggestion without a project for the owner to pick one in its Day
view (TimeTracker's ADR 0030).

## Decision

- A finished call that passes the other checks (Working, the context's Status feature, at least 3 minutes) is always
  sent as a stored suggestion: on the named project's first TimeTracker link, else without `--project`.
- The 2-minute restart merge applies to every call, as before.
- The Activity tab reads suggestions without a project and shows them as "No project".

## Consequences

- Calls that named no project now show up in TimeTracker's Day view as suggestions without a project; "Approve all"
  skips them until a project is picked.
- A project with several TimeTracker links still gets its first one; the owner corrects it in TimeTracker.
