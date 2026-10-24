# 0015. Every vault write goes through the note writer; the app never touches frontmatter

## Status

Accepted (2026-10-24), implemented. Daybook only; TimeTracker has no vault.

## Context

The vault is the owner's own writing, edited in Obsidian and synced by other tools while Daybook runs. A
companion that rewrites notes as documents, writes in several places, or keeps app data in frontmatter would
fight the editor, lose text on a concurrent change and leave app-specific clutter in human-written notes. The
prototype learned this and settled on one write path and pure line edits.

## Decision

- `vault::writer::NoteWriter::edit_note` is the only code that writes into the vault. It serializes writes per
  file, reads fresh, refuses paths outside the vault, writes a temp file and renames it, and starts over when
  the file changed between the read and the write.
- A change is a `NoteOp`: data applied by a pure function over the note's text that adds or replaces lines in
  a known section and leaves every other line as it was. Changes for a note that doesn't exist yet wait in
  `pending-entries.json` and are written in order once Obsidian has created it.
- Daily notes are created by Obsidian (its CLI), never by Daybook, so the owner's template applies.
- Nothing app-specific goes into frontmatter or vault files; all configuration lives in Settings.

## Consequences

- A review flags any other write into the vault (`.claude/agents/review-code-cleanliness.md`).
- Each feature that writes adds a `NoteOp` kind with its own pure writer and tests on note text.
- A change can be delayed (queued) instead of written at once; callers get `WriteOutcome::Queued`.
