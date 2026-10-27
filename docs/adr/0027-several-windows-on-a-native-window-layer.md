# 0027. Several windows on a native window layer

## Status

Accepted (2026-10-27), implemented. Deviates from TimeTracker, which has one window and one non-activating
picker. Records a decision made in step 04 and extended by later steps; the overlay's rules are
[ADR 0016](0016-overlays-never-navigate.md).

## Context

Daybook lives in the menu bar and opens over whatever the owner works in: a window for the day, Settings, the
overlays, a floating quick notes editor and a bar at the screen edge that must stay up on every Space and over
full-screen apps without ever taking focus. Tauri creates, shows, hides and sizes windows, but has no
non-activating or never-key panels, no window levels above the menu bar, no Space behaviour and no view of
which app has a window on the current Space.

## Decision

- Daybook has several windows, each with its own page: the Daybook window (`main`), Settings (`settings`),
  the overlay panel (`panel`), the quick notes editor (`quicknotes`), the rail (`rail`) and its tooltip
  (`railtip`). They share the `api` contract ([ADR 0002](0002-ipc-in-api-files.md)) and the Rust commands
  ([ADR 0017](0017-commands-live-in-rust.md)).
- Tauri builds and shows them; what it lacks is done with objc2 in a small native layer
  (`window/native.rs`, `window/panel/native.rs`, `rail/native.rs`): class swaps to non-activating and inert
  panels, levels, collection behaviour, frames, screens, the pointer and the app in front.
- The rules (which screen, which frame, when to show, where focus goes) are pure and unit-tested; the native
  layer only applies them, always on the main thread.
- Daybook stays a menu-bar app: the Dock icon shows only while the Daybook window or Settings is open.

## Consequences

- The class swaps depend on tao's window layout, so tao is pinned and a mismatch falls back to a plain
  window with a warning (`docs/windows.md`).
- The AppKit side is checked by hand on two Spaces and two screens; each new window documents its kind,
  level and focus in `docs/windows.md`.
- Each window but the Daybook window (`index.html`) is a separate page under `src/windows/`.
