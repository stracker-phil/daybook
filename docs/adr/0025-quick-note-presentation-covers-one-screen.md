# 0025. Quick note presentation covers one screen, not macOS full screen

## Status

Accepted (2026-10-27), implemented.

## Context

The owner wants a quick note to fill its screen in large text while they keep typing, with the other screens
unchanged. macOS full screen (`toggleFullScreen:`) makes a new Space with a slide animation, takes the window off
the Space the owner looks at, fights the editor's floating level, and with "Displays have separate Spaces" off
blanks the other screens.

## Decision

- Presentation sets the editor's frame to the whole frame of the screen it is on, at `NSMainMenuWindowLevel + 1`
  (above that screen's menu bar and Dock), with the window buttons hidden and resizing off. Leaving restores the
  frame and level.
- ⌘⌥P is a page key in the editor (rebindable), not a global shortcut.
- The font fit runs in the page, which measures text; Rust owns entering, leaving, the frames and the note.

## Consequences

- Entering and leaving are instant and touch no other screen or Space.
- The menu bar and Dock of that screen are covered while presenting.
