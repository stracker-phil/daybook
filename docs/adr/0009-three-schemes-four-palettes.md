# 0009. Appearance is one of three schemes built from four palettes

## Status

Accepted (2026-10-24), implemented. Inherited from TimeTracker ADR 0021. Partly superseded by
[ADR 0026](0026-the-daybook-window-follows-the-prototypes-look.md).

Amended (2026-10-27): the sidebar no longer has its own accent hues, and the hues are no longer tuned per
palette: both tones use Apple's system colours, and a sidebar takes the hues of its tone (ADR 0026). The three
schemes and four palettes stand.

## Context

Daybook is "a daily companion, not a dev tool", with an always-dark sidebar in its window by default. A
gallery of editor themes makes an app look like a code editor; the apps Daybook sits beside (Calendar,
Fantastical, Obsidian) offer a few designed looks instead.

## Decision

A scheme is one of three looks: Default (dark sidebar, light content), Light (all light) or Dark (all dark).
The colors come from the four palettes of [0007](0007-color-palettes.md), one per surface and tone.
`lib/theme.ts` maps a scheme to a content tone and a sidebar tone (`schemeLook`) and sets `data-content` and
`data-sidebar` on `<html>`. The sidebar has its own token set (`--sidebar-*`) and its own accent hues, so it
can be dark beside light content. The nine accent hues are tuned per palette so each reads as a dot, label
and fill on its surface.

## Consequences

- No scheme gallery: adding a look means a new scheme value and, at most, another palette file.
- Contrast is checked per palette, not per look.
