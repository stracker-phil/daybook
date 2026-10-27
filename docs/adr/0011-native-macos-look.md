# 0011. The app looks native

## Status

Accepted (2026-10-24), implemented. Inherited from TimeTracker ADR 0024. Partly superseded by
[ADR 0026](0026-the-daybook-window-follows-the-prototypes-look.md): the Daybook window (sidebar, board,
inspector) takes the prototype's look; the Settings window, overlays, controls and the unified toolbar
elsewhere keep this record's look.

## Context

The prototype is an Electron app with web-style controls. Daybook sits beside Calendar, Reminders, System
Settings and TimeTracker all day; TimeTracker already went through cards with borders, filled web buttons,
chips and repeated toolbars before arriving at a look that matches those apps. Daybook takes that result.

## Decision

- **Type:** the system font (SF Pro through `-apple-system`) at the macOS text styles (13px body, 11px
  secondary, 15px titles), tabular figures. Mono (SF Mono through `ui-monospace`) only for machine text such
  as paths, URLs and commands. No bundled fonts.
- **Surfaces:** neutral greys, no tint. Content is the window background. Grouped content is a System
  Settings inset group: a translucent quaternary fill (`--fill-quiet`), a 10px radius, no border, hairline
  separators, no hover fill on plain rows. Overlays draw a hairline ring and a soft shadow. The dark sidebar
  is macOS's dark sidebar; where a scheme's sidebar tone matches the macOS mode, the window's sidebar
  material shows through (vibrancy).
- **Window chrome:** a unified toolbar. A view with a title renders `Toolbar`: title, detail and actions in
  one row beside the window buttons. The title bar is `Overlay` with drag regions.
- **Controls:** AppKit metrics: 22px push buttons with a bezel shadow; the accent fill only for a sheet's
  default button; borderless toolbar and inline buttons; native popup buttons; 22px fields with the accent
  focus ring; a fill-track segmented control; a white-thumb switch. Default buttons sit last, Cancel before
  them, destructive actions at the leading end. Labels after a name are plain secondary text, never chips.
- **Accent:** System follows the macOS accent color (WebKit's `AccentColor`) and is the default; the nine
  hues are the alternatives.

## Consequences

- `docs/ux.md` holds the token and control spec; the palettes define the translucent fills and control
  tokens these controls use.
- The window can be dragged by any empty part of a toolbar.
- The vibrant sidebar depends on the macOS mode, so a scheme looks slightly different in each mode; the
  Default scheme's dark sidebar in light mode stays opaque.
- An unknown accent in `settings.json` falls back to System.
