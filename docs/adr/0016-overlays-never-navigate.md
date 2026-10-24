# 0016. Overlays are a non-activating panel that never navigates

## Status

Accepted (2026-10-24), implemented. Extends TimeTracker's non-activating timer picker to every overlay.

## Context

Overlays (status, activity, journal, routines, the palette) open over whatever the owner works in, often on
another Space or over a full-screen app. The prototype's overlay activated the app, so closing it had to hand
focus back by bringing the previous app forward; that switched back to the Space the overlay was opened on
while the owner had moved elsewhere, which they called "a clear overstepping of boundaries". Electron also
forced workarounds for flashes of the previous view and for windows that reappeared while closed.

## Decision

- Every overlay is one window turned into a non-activating `NSPanel` (objc2 class swap, as in TimeTracker). It
  takes the keyboard without activating Daybook, so the app in front stays the active app and gets the
  keyboard back when the panel goes away.
- Closing hands focus back only to an app with a window on the current Space, and only when Daybook became
  active meanwhile. It never activates an app on another Space; when there is nothing to hand back to, it
  hides Daybook and lets macOS choose.
- Opening is a session handshake with the page (ready, then painted) and the window is shown at alpha 0 until
  the new view painted. A closed overlay is never on screen.
- Sticky views stay on every Space until closed and follow the app in front, so closing lands where the owner
  is now.

## Consequences

- No overlay can switch Spaces or bring back an app the owner left.
- The panel depends on tao's window layout matching the panel class (`make_panel` checks it and falls back to
  a plain window with a warning).
- AppKit calls must run on the main thread; the window layer routes them there.
- The rules (`panel/session.rs`, `front_app::hand_back`, `display.rs`) are pure and unit-tested; the AppKit
  side is checked by hand on two Spaces and two screens.
