# 0001. Rust owns the logic, React only renders

## Status

Accepted (2026-10-24), implemented. Inherited from TimeTracker ADR 0001.

## Context

Daybook decides which section of a daily note a line goes into, how a routine or a reminder is due, which
projects are current, what status follows a call and what the menu bar says. Each rule must be right once and
testable without a browser. The prototype kept much of this in page code, where it could only be tested by
pulling pure modules out by hand. The webview also renders note text and titles from the vault and other apps,
so it is the part most exposed to surprises.

## Decision

Every rule lives in Rust: reading and writing the vault, settings sanitizing, scheduling, matching and the text
the app shows or writes. The page asks for views and results, renders them and forwards user intent (a key, a
click, a field value); it never decides on its own what a note contains or what a command does. Splitting
logic between the two sides was rejected because the same rule would then exist twice, or only in the half
that cannot be unit tested without a DOM.

## Consequences

- The logic is unit tested in Rust; the prototype's tests port as Rust tests.
- More IPC types and round trips, even for small things.
- A UI feature usually needs a Rust change first.
