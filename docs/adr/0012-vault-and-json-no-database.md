# 0012. Data is the vault plus small JSON files; no database to start

## Status

Accepted (2026-10-24), implemented.

## Context

TimeTracker keeps its time data in SQLite, because it writes a span every few seconds and queries years of
them. Daybook does not track time. Its content is the Obsidian vault `~/Obsidian/ThoughtForge`,
which stays human-written and readable in Obsidian; everything else is configuration and a little state
(settings, keybindings, window layout, later the status and routine progress). The prototype ran on exactly
that, the vault plus small JSON files, without a database.

## Decision

No SQLite, no sqlx and no migrations to start. Notes are read from and written to the vault as Markdown
files. The app's own data are JSON files in `~/Library/Application Support/com.philippstracker.daybook/`
(`settings.json`, `keybindings.json`, `ui_state.json`), each behind a `JsonStore` that writes atomically
with private permissions, loads unknown values as defaults and moves a file it cannot parse aside. A database is added only when a
step needs one (for example a search index that is too slow to rebuild), with its own record.

## Consequences

- No schema, migrations, backups or data-folder setting to maintain; the vault is backed up and synced the
  way the owner already does it.
- A JSON file is read and written whole, which is fine for small files and wrong for logs; a store that
  grows without bound needs another design.
- Queries over the vault (search, open todos) scan files; their speed is checked when those steps land.
