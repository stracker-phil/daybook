# 0010. Appearance follows macOS, with one scheme per mode

## Status

Accepted (2026-10-24), implemented. Inherited from TimeTracker ADR 0022.

## Context

The owner wants the app to follow macOS and to pick a look for each macOS mode, for example all light even
while macOS is dark. A Light/Dark/System mode setting with one scheme cannot express that.

## Decision

- The app always follows the macOS appearance. There is no mode setting and no app override; the native
  window theme is never set, so it follows the system too.
- Two settings: `lightScheme` (the look while macOS is light; default `default`) and `darkScheme` (while
  macOS is dark; default `dark`). Each is one of Default, Light or Dark ([0009](0009-three-schemes-four-palettes.md)).
- The look is the scheme of the current macOS mode, nothing else: dark mode with `darkScheme` Light is all
  light. `lib/theme.ts` picks the scheme by `prefers-color-scheme` (`resolveLook`).
- Settings → Appearance shows one scheme picker per mode. The accent is one setting for both modes.
- An unknown scheme in `settings.json` loads as that mode's default.

## Consequences

- A mode can only be captured by switching macOS itself; `scripts/dev/screenshot-looks.sh` captures the three
  schemes in the current one.
- Native controls (menus, pickers) follow macOS, so they can be dark beside a light page when `darkScheme` is
  Light or Default; the page's own controls follow the content palette's `color-scheme`.
