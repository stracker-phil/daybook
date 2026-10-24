# 0007. Color schemes are complete token sets; no color literal outside a palette

## Status

Accepted (2026-10-24), implemented. Inherited from TimeTracker ADR 0016.

## Context

The app runs in light and dark mode, with a sidebar that can be dark beside light content. A missing dark
value shows only when someone looks, and a color written into a component breaks the moment the look changes.

## Decision

Colors come from four palettes in `src/styles/palettes/`: `content-light.css`, `content-dark.css`,
`sidebar-light.css` and `sidebar-dark.css`. Each is a complete token set for its surface: surfaces, text,
lines, fills, shadows and the nine accent hues. Palettes of a kind define the same custom properties.
`theme.css` holds only derived tokens. No color literal appears outside a palette file; a Vitest scan
(`src/styles/colors.test.ts`) enforces it and checks that both palettes of a kind match, and
`contrast.test.ts` checks text contrast per palette. Free-form colors (a hex picker) were rejected because
they break on a change of look.

## Consequences

- Nine accent hues; anything that needs a color picks one of them.
- Every missing token fails the scan.
- Every new style needs a token first.
