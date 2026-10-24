# 0013. Settings are one JSON model; the prototype is imported once

## Status

Accepted (2026-10-24), implemented.

## Context

The Glaze prototype keeps the owner's whole configuration (contexts, projects, links, routines, journal
groups, Slack mappings, the rail, shortcuts) in one `settings.json`, normalized on every read by
`config-normalize.ts` so a hand edit or an older file never breaks the app. Daybook replaces the prototype and
must start with that configuration, without the keys that only served time tracking, which TimeTracker owns
now. TimeTracker's settings are a small struct with a lenient serde field each; that style does not carry
rules such as "drop list items without a name", "give a missing colour the next of the palette" or "every
routine has one reminder rule".

## Decision

`Settings` is one typed Rust model in `settings.json`, grouped by feature (`vault`, `dailyNote`, `slack`,
`contexts`, `projects`, `rail`, …), camelCase, generated into TypeScript. Its `Deserialize` hands any JSON to
`Settings::from_json`, a rule-by-rule port of the prototype's normalizing: every load and every save from the
page repairs instead of rejecting. Renames happen once, here: clients become projects, modes contexts, the
`tracking` feature `status`.

On a launch without `settings.json`, when the prototype's data folder has one, `store/import/` maps it into
Daybook's shape, drops the tracking keys, repairs it and saves it, after copying the prototype's state files
in Daybook's shapes. It only reads the prototype's folder, runs once and reports what it did in the log and a
notice. The Slack token, encrypted for the prototype by Electron, has to be entered again.

## Consequences

- One model and one repair path: the frontend sends back the whole settings and cannot store anything Rust
  would not load.
- Later steps add their settings pages on top of fields that already exist; a field nobody ports stays in the
  model until a step drops it.
- The import is owner-specific and removable (`docs/adapting.md`); once the prototype is retired (step 28) the
  module can go.
- Each save writes the whole file; two quick saves from the page can overwrite each other (accepted, the page
  saves rarely).
