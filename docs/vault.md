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
| `journalGroup { group, answers }` | rewrites one journal group's callout (only answered questions; unknown questions kept; the group and an empty section removed when nothing is answered) | `ops/journal/` |
| `planStep { title, done }` | ticks or unticks a routine item in the plan section, adding it if needed ([routines.md](routines.md)) | `ops/plan.rs` |
| `planList { titles }` | adds a starting routine's missing items unticked | `ops/plan.rs` |
| `focus { body }` | replaces the focus section's body | `ops/focus.rs` |
| `noteSection { title, level, newTitle, body, expected? }` | rewrites (or adds, at the end of the notes section) one `###` section below the notes heading, maybe renaming it; with `expected`, fails unless the section still holds that text (`section_holds`) | `ops/notes.rs` |

The modules also read what they write (`read_plan_tasks`, `read_focus`, `parse_journal_groups` and
`group_answers`, `list_note_sections` and `read_note_section`) for the features that show it.

**Adding a kind:** add the variant to `NoteOp` (serde tag `kind`, camelCase), its pure writer in `ops/`, the
arm in `NoteOp::edit` and the line in `describe`. Waiting entries of a kind this version can't read are
dropped when `pending-entries.json` loads.

## Journal

Settings → Journal holds the question groups (`settings.journal`: title, callout type, folded, questions with
a hint and a kind, timing). Each group is one callout in the journal section, in Settings order:

```text
> [!NOTE]+ Dawn
> **What am I grateful for?** - _Small things count too_
> 1. Sunshine
>
> **The "One Thing"** - _What would I do today even without pay?_
> - [ ] Finish the draft
```

A list answer is numbered, a task answer a task line, a text answer plain lines (blank lines dropped, since a
blank `>` line ends the answer); a group without questions is free text. Only answered questions are written,
`?` placeholders count as unanswered, questions the app doesn't know are kept, an existing callout keeps its own
header (fold state), and a group without answers loses its callout (the section too once empty). Titles match
case- and quote-insensitively.

`journal/` reads it: `progress.rs` (pure) fills the groups with a note's answers (`journal_groups`) and counts
them (`JournalProgress`: answered of total, `has_note` for a written free-text group); `mod.rs` has the commands
and `journal:open` / `journal:open:<group>` (the journal overlay); `rename.rs` moves a renamed group's reminder
rule along on save (`follow_renamed_groups`, called from `update_settings`). The progress feeds the board's
journal chips (`HubView.journal`, for the shown day), the tool snapshot (the Journal menu and palette entries
with `1/3` or `Done`) and the reminder rule of each group (due while its `timing.due` window is open).

| Command | Arguments | Returns |
|---|---|---|
| `journal_info` | – | `JournalInfo`: today's groups with their answers, `noteExists` |
| `journal_save` | `group`, `answers` | –; writes `NoteOp::JournalGroup` through the writer (waits for the note when it doesn't exist); `not_found` for an unknown group, `invalid_input` past 50 answers or entries or 20,000 characters |

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
batch once the folder has been quiet for 300 ms. `DailyNotes::watch_daily_notes` follows the daily-notes
folder and everything below it (`recursive: true`, so year archives and month folders count): it starts at
startup and is re-pointed after a settings save and on every `edit_today` (so a folder is picked up once it
exists). Every batch also reaches the [open-todo index](#open-todos) (`NotesIndex::changed`).

Every change reaches the listener `commands::install` sets, which emits `note_changed { day }`: `day` is
today (`YYYY-MM-DD`) when today's note changed or the waiting changes or the creating state changed, `null` for
another note in the folder.

## Open todos

`todos/` keeps an index over the daily-notes folder (`NotesIndex`, `index.rs`): the notes whose file name holds
a `YYYY-MM-DD` date, found once by walking the folder (dot folders skipped), and each note's open todos, scanned
once and kept until the watcher reports the note (`changed`); a note that appeared or went makes the folder be
walked again. A changed vault, format or focus heading starts over.

- `scan.rs` (pure, `scan_open_todos`): every unticked `- [ ]` (`*` and `+` too), also inside callouts, with the
  headings from level 2 down ("Notes › Meeting Two"); not `- [ ] ?`, not inside code fences, not in the focus
  section (up to the next heading of its level).
- `mod.rs` `open_todos`: newest note first, the same text (case and spacing ignored) on the same day once,
  routine items left out (the wizard ticks them). Each `OpenTodo` has its text, `detail` ("12 Oct · Notes ›
  Meeting Two"), date and vault-relative file.
- `groups.rs` (pure): Today, This Week (from Monday), Last Week, then each month of this year and each earlier
  year; empty groups never appear.
- `note_days`: the days whose daily note exists (a dated note that isn't that day's daily note, in the folder
  or its year archive, doesn't count); the Daybook window's ‹ › step through them.

Daybook never ticks a todo; a click opens its note in Obsidian (`todo_open`).

## Quick notes

`quick_notes/` writes one topic of today's note at a time: the `###` (and deeper) headings inside the notes
section (Settings → Daily Note → Notes, `## Notes` by default), outside code blocks. A topic's text is
everything up to the next heading of its level or higher, its own subheadings included, without the blank lines
around it.

- **Topics.** `QuickNotes` holds the topic the editor shows and the editor's frame (`quick_notes.json`).
  `topic.rs` (pure) checks a topic (a one-line title of at most 200 characters, 2 to 6 `#`), reads
  `notes:topic:`'s argument `<level>:<title>`, and builds the picker's rows (`rows_for`): the topics whose
  title contains the query, ignoring case, then a new `###` topic named after it unless one has that title.
- **New topics** are written as an empty section at once (`quick_notes_open` with `create`), at the end of the
  notes section; the notes section itself is placed by the configured order when missing. A topic that already
  has text keeps it.
- **Saving** (`quick_note_save`) writes the body verbatim (only blank lines around it are dropped) and renames
  the heading to the new title. It sends `base`, the text the editor started from, as the op's `expected`: when
  the note holds another text for the topic (edited in Obsidian meanwhile), nothing is written and the answer
  is `changed` with the note's text, so neither side is lost. Without today's note the save waits like every
  other write.

## Commands

| Command | Args | Returns | Errors |
|---|---|---|---|
| `daily_note_state` | – | `DailyNoteState` | – |
| `daily_note_open` | `day: string \| null` (`YYYY-MM-DD`, default today), `heading: string \| null` | – (opens the note in Obsidian through `obsidian://open`, at `heading` when given; today's missing note is created instead) | `invalid_input` (no vault, not a day), `not_found` (no note that day), `io` |
| `todo_open` | `file` (vault-relative) | – (opens the todo's note in Obsidian) | `invalid_input` (no vault, outside the vault), `io` |
| `daily_note_create` | – | `bool`: whether today's note exists afterwards | `invalid_input`, `io` |
| `quick_notes_list` | `query` | `QuickNoteTopics`: the picker's rows, whether today has any topic, the notes heading | `invalid_input` (no vault), `io` |
| `quick_notes_open` | `topic: NoteSectionRef`, `create: bool` | – (closes the picker, writes a new topic, opens the editor on it) | `invalid_input` (bad topic) |
| `quick_note_get` | – | `QuickNote \| null`: the editor's topic and its text | `invalid_input`, `io` |
| `quick_note_save` | `topic`, `newTitle` (empty keeps it), `body`, `base` | `QuickNoteSaved`: `saved { topic }` (its heading now) or `changed { note }` | `invalid_input` (bad topic or title, over 200 000 characters, no vault), `io` |
| `quick_notes_close` | – | – (hides the editor, keeping its frame) | – |
| `quick_notes_presentation` | – | `QuickNotePresentation { presenting }` (the event `quick_note_presentation` carries the same on every change; [windows.md](windows.md#quick-notes-editor)) | – |
| `vault_info` | – | `VaultInfo` (`DailyNotes::info`, for Settings → Vault): the `DailyNoteFormat` in use (`null` without a vault), today's note path and whether it exists, the Obsidian CLI's path | – |
| `dev_edit_today_note` (debug builds, `dev.rs`) | `op: NoteOp` | `WriteOutcome` | as `edit_today` |

Drive the write path from a debug build with `scripts/dev/js.sh`, e.g.
`window.__TAURI_INTERNALS__.invoke("dev_edit_today_note", { op: { kind: "planStep", title: "Test", done: false } })`.
