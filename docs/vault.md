# Vault (daily notes)

`src-tauri/src/vault/`. Daybook reads the Obsidian vault and adds lines to known sections of the daily
notes; it never edits a note as a document and never touches frontmatter
([ADR 0015](adr/0015-one-write-path-into-the-vault.md)). Every write into the vault goes through
`vault::writer::NoteWriter::edit_note`. The vault is `settings.vault.path`; without one every vault call fails
with `invalid_input` ("Choose your vault in Settings first.").

| File | Holds |
|---|---|
| `mod.rs` | `DailyNotes`, the service in `AppState.notes`: paths, reading, `edit_today`, draining, the watcher |
| `location.rs` | where a day's note is: `FormatCache`, `DailyNoteFormat`, `format_moment` |
| `note.rs` | pure line edits: `Note`, headings, sections |
| `ops/` | `NoteOp` and the pure writer of each kind: `plan.rs`, `focus.rs`, `journal.rs`, `notes.rs` |
| `writer.rs` | `NoteWriter`: the one write path |
| `pending.rs` | `Pending`: changes waiting in `pending-entries.json` |
| `create.rs` | `DailyNotes::create_today` through the Obsidian CLI, `Timings` |
| `obsidian.rs` | `ObsidianCli` (lookup, run) and `open_url` (`obsidian://open`) |
| `watch.rs` | `FolderWatch`: a debounced `notify` watcher |
| `commands.rs` | the Tauri commands, the `note_changed` event and `install` (startup) |

Child processes run through `process.rs` (`run` with a timeout and the Homebrew folders on the `PATH`,
`run_in_login_shell`), wall-clock helpers are in `clock.rs`.

## Location

`FormatCache::get(vault, &settings.daily_note)` reads `<vault>/.obsidian/daily-notes.json`: `folder` (trimmed,
no leading or trailing `/`) and `format` (moment.js tokens; blank means `YYYY-MM-DD`). Without the file, or
when it isn't JSON, it uses `dailyNote.fallbackFolder` and `fallbackFormat` from Settings
(`from_obsidian: false`). The result is kept until the file's modification time or size, the vault or the
fallback changes, so a lookup costs one `stat`.

`DailyNoteFormat::note_path(vault, day)` is `<vault>/<folder>/<format_moment(day, format)>.md` (the format may
contain `/` for subfolders). `archived_path` is `<folder>/<YYYY>/<last name part>.md`, where older years are
archived; `DailyNotes::find_note(day)` tries both.

`format_moment` knows `[literal]`, `YYYY YY MMMM MMM MM M Do DD D dddd ddd dd d GGGG gggg WW ww W w Q` (weeks
are ISO weeks, also for `gggg`/`ww`/`w`).

## Sections

`note.rs` is pure and works on `Note { lines, eol, trailing_newline }`: `Note::parse` keeps `\n` or `\r\n` and
whether the text ended in a newline, `serialize` writes them back, so untouched lines stay byte-identical.

- **Headings** are matched by level and text, case-insensitive, with punctuation and spaces at either end
  trimmed (`## Notes:` is `## notes`), outside fenced code blocks and outside the frontmatter
  (`find_heading`). A section ends at the next heading of the same or a higher level (`section_end`).
- **Appending** (`append_to_section`) adds lines after the section's last non-empty line, with a blank line
  between them and any text that follows.
- **A missing section** is created at its place in the configured order (`place_heading`): right before the
  next section of the order that the note has, else after the previous one, else below the frontmatter and
  the note's `# Title`. A heading that isn't in the order goes to the end. Blank lines go around it.
- **Replacing a body** (`replace_section`) swaps the lines between the heading and the next heading; a missing
  section is created only once there is a body.

The headings come from Settings → Daily Note: `dailyNote.journal`, `focus` and `plan` are a level and a text
(`SectionPlacement::heading`), `notes` is a heading line, and `dailyNote.order` is the order
(`DailyNote::heading_order`). `NoteLayout::from_settings` collects them with the journal groups.

## Note ops

A change to a daily note is a `NoteOp`, data that can wait on disk. Each kind is a pure function over the
note's text; `NoteOp::apply(text, &layout)` returns the new text, `None` when nothing changes, or an error
when the op can never apply. `describe()` is one line for the action log.

| Kind | Writes | Module |
|---|---|---|
| `journalGroup { group, answers }` | rewrites one journal group's callout (only answered questions; unknown questions kept; the group and an empty section removed when nothing is answered) | `ops/journal.rs` |
| `planStep { title, done }` | ticks or unticks a routine item in the plan section, adding it if needed | `ops/plan.rs` |
| `planList { titles }` | adds a starting routine's missing items unticked | `ops/plan.rs` |
| `focus { body }` | replaces the focus section's body | `ops/focus.rs` |
| `noteSection { title, level, newTitle, body }` | rewrites (or adds, at the end of the notes section) one `###` section below the notes heading, maybe renaming it | `ops/notes.rs` |

The modules also read what they write (`read_plan_tasks`, `read_focus`, `parse_journal_groups` and
`group_answers`, `list_note_sections` and `read_note_section`) for the features that show it.

**Adding a kind:** add the variant to `NoteOp` (serde tag `kind`, camelCase), its pure writer in `ops/`, the
arm in `NoteOp::edit` and the line in `describe`. Waiting entries of a kind this version can't read are
dropped when `pending-entries.json` loads.

## The writer

`NoteWriter::edit_note(vault, file, edit)` is the only function that writes into the vault:

1. It refuses a path outside the vault: lexically, and any `..` component is refused.
2. Edits of one file run one at a time (a lock per path).
3. It reads the file fresh, runs the edit, and before writing checks that the modification time and size
   are still those it read. If not, it starts over once; a file that keeps changing is a
   `WriteError::Conflict`.
4. It writes the text to `.<name>.<pid>.<nanos>.tmp` in the same folder with the original mode, syncs it and
   renames it over the note. A crash before the rename leaves the note as it was; a failed write removes the
   temp file.

It returns whether the file changed. `WriteError::Edit` (the op can't apply) and `OutsideVault` become
`invalid_input` for the page, `Io` becomes `io`.

## Today's note and waiting changes

`DailyNotes::edit_today(settings, op)` writes to today's note and returns a `WriteOutcome`: `written`,
`unchanged`, or `queued`:

- While today's note doesn't exist, or changes for it wait already (to keep their order), the op is queued in
  `pending.rs` and Obsidian is asked to create the note in the background (`create_today`). A note that exists
  gets the queue drained at once.
- A `Conflict` queues the op too.

`pending-entries.json` (config dir) holds `{ entries: [{ createdAt, file, description, op }] }`. Entries never
expire. `drain` writes them in order, each into its own note; a missing note, a conflict or a disk error keeps
that note's entries (in order) for the next drain, while other notes still get theirs; an op that can't apply
is dropped and logged. One drain runs at a time. Drains run at startup (`commands::install`), after
`create_today`, and when the watcher sees a note change while entries wait (`note_appeared`, which first waits
until the note is quiet). The step 02 import copies the prototype's file, without its tracking ops.

`DailyNotes::state` is today's `DailyNoteState { path, exists, creating, waiting }`.

## Creating today's note

Daily notes are created by Obsidian, so its template and Templater fill them. `create_today`:

1. finds the Obsidian CLI (`ObsidianCli::find`): `vault.obsidianCliPath` from Settings when set (it must be
   executable), else `command -v obsidian` in a login zsh, else `/usr/local/bin/obsidian`,
   `/opt/homebrew/bin/obsidian` and `/Applications/Obsidian.app/Contents/MacOS/obsidian`. A found path is
   kept per configured path; a missing one is looked for again next time.
2. runs `obsidian vault=<vault.name> daily` (20 s limit),
3. waits up to 15 s for the file (polling every 500 ms), then until its size and modification time stay the
   same for 1.5 s (at most 10 checks), because Templater fills it right after,
4. drains the waiting changes.

One creation runs at a time; a second call waits for it. `Timings` holds the waits (tests pass short ones
through `DailyNotes::with_clock`, which also fixes "today"). Failures come back as `io` errors with the CLI's
message ("Obsidian CLI not found. Turn it on in …", "Obsidian didn't create … within 15 seconds.").

## Watcher

`watch.rs` wraps a `notify` watcher (FSEvents) on one folder and reports the `.md` files that changed, in one
batch once the folder has been quiet for 300 ms. `DailyNotes::watch_today_folder` follows the folder of
today's note: it starts at startup and is re-pointed after a settings save and on every `edit_today` (so a new
month's folder is picked up once it exists). The recursive watcher for search and todos (steps 15 and 17) uses
the same `FolderWatch::start(folder, recursive: true, …)`.

Every change reaches the listener `commands::install` sets, which emits `note_changed { day }`: `day` is
today (`YYYY-MM-DD`) when today's note changed or the waiting changes or the creating state changed, `null` for
another note in the folder.

## Commands

| Command | Args | Returns | Errors |
|---|---|---|---|
| `daily_note_state` | – | `DailyNoteState` | – |
| `daily_note_open` | `day: string \| null` (`YYYY-MM-DD`, default today) | – (opens the note in Obsidian through `obsidian://open`; today's missing note is created instead) | `invalid_input` (no vault, not a day), `not_found` (no note that day), `io` |
| `daily_note_create` | – | `bool`: whether today's note exists afterwards | `invalid_input`, `io` |
| `vault_info` | – | `VaultInfo` (`DailyNotes::info`, for Settings → Vault): the `DailyNoteFormat` in use (`null` without a vault), today's note path and whether it exists, the Obsidian CLI's path | – |
| `dev_edit_today_note` (debug builds, `dev.rs`) | `op: NoteOp` | `WriteOutcome` | as `edit_today` |

Drive the write path from a debug build with `scripts/dev/js.sh`, e.g.
`window.__TAURI_INTERNALS__.invoke("dev_edit_today_note", { op: { kind: "planStep", title: "Test", done: false } })`.
