# 0028. Overlays follow the prototype's look; the activity is not the timer

## Status

Accepted (2026-10-27), implemented. Supersedes parts of [ADR 0011](0011-native-macos-look.md) for the overlay
panel, as [ADR 0026](0026-the-daybook-window-follows-the-prototypes-look.md) did for the Daybook window.

## Context

The owner compared the overlays with the prototype's side by side and preferred the prototype: a translucent
panel, 36px rows, a grey or context-tinted selection instead of an accent fill, outline key caps with labels in
the footer. ADR 0011 had given the panel TimeTracker's native look, opaque `--pop` and rem sizing. The overlays
also couldn't be moved out of the way of what the owner looks at.

The activity overlay picked TimeTracker projects, grouped under Daybook projects. The owner's workflow is the
prototype's: say "this is Oscorp" with as little friction as possible, and decide about the timer only where there
is a real choice. A client in TimeTracker often has several projects, and some work needs no timer at all.

## Decision

- The overlay panel takes the prototype's look in px: popover material, a 12px card with a hairline, the
  prototype's prompt, field, rows, section headers, "Current" marker, footer key caps with Title Case labels,
  project keys and segmented control (`docs/ux.md`, Overlays). Its text no longer scales with the text size.
- Overlays can be dragged by the grabber strip, the prompt and the footer. Dragging never activates Daybook or
  closes the panel ([ADR 0016](0016-overlays-never-navigate.md) holds). Every fresh open is centred again; a
  dragged spot is kept only while the overlay stays open.
- The activity overlay picks two things: the **activity** (a Daybook project and a note) and the **timer** (a
  TimeTracker project, or none). A project linking no TimeTracker project never has a timer; one linking one
  always has a timer on it; one linking several offers "None" (the default) or one of them. No timer stops a
  running one.
- Daybook keeps the current activity for the day, also without a timer, so the surfaces and the next check-in
  show it. Daybook still records no time ([ADR 0022](0022-daybook-drives-timetracker-through-its-cli.md)).
- The rest of ADR 0011 stands: the Settings window, dialogs, toasts and controls outside the panel keep the
  native look.

## Consequences

- The panel's values live in `src/windows/panel/panel.css` and its shared parts in `src/windows/panel/parts/`;
  the window's `.palette-*` rules are gone from `overlays.css`.
- The material follows the macOS mode, so the panel sets its appearance to the picked scheme's content tone.
- An activity without a timer is a state Daybook owns (`activity.json`); it ends with the day and on Off, and
  follows TimeTracker's timer while one runs.
