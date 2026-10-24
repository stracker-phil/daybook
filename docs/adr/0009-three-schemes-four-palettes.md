# 0009. Appearance is one of three schemes built from four palettes

## Status

Accepted (2026-10-24), implemented. Inherited from TimeTracker ADR 0021.

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
