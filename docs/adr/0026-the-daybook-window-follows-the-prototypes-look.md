# 0026. The Daybook window follows the prototype's look

## Status

Accepted (2026-10-27), implemented. Supersedes parts of [ADR 0011](0011-native-macos-look.md) for the Daybook
window.

## Context

The owner compared the native Daybook window with the prototype side by side and preferred the prototype: its
sizes, spacing, colours and layout (the Glaze components on a 13px rem). ADR 0011 had taken TimeTracker's native
look instead: rem-based type and spacing, a quiet inspector pane, a rule under every toolbar, deep accent hues
with one text colour on them, and a read-only todo list.

## Decision

- The Daybook window (sidebar, board, inspector) takes the prototype's values in px: a 220px sidebar on
  `#212121` with white labels and icons, 32px tool rows, an 18px semibold weekday, a 680px board column with
  32px between blocks, 11px medium section titles, 36px chips, 16px checkboxes, the inspector on the content
  surface with an accent-filled tab, no rule under the toolbars, the inspector toggle in the lower right corner,
  light hairlines that fade out at the window's vertical edges, and Glaze's inactive-window dimming.
- The type stays the system font (SF Pro); the prototype's Inter is not bundled.
- The accent and context hues are Apple's system colours (light and dark) as in the prototype's
  `context-color.ts`. Text on a hue's fill is white on the deep ones and near-black on the light ones (yellow,
  orange, green, teal), derived in CSS from the hue's lightness, instead of one `--on-accent` per palette.
- Dates in the window follow the macOS locale (`AppleLocale`), as the prototype's `toLocale*String` did.
- Todos tick from the inspector, as in the prototype, through the note writer
  ([ADR 0015](0015-one-write-path-into-the-vault.md)).
- The rest of ADR 0011 stands: the Settings window, overlays, controls and the unified toolbar elsewhere keep
  the native look.

## Consequences

- The sidebar no longer scales with the text size; the board's px values don't either.
- No single text colour reaches 4.5:1 on every Apple hue; text on fills reaches at least about 3:1, and the
  contrast checks change accordingly. On the light sidebar the light hues are only readable as capped text
  (`--context-text`, `--accent-text`).
- `docs/ux.md` holds the window's values.
