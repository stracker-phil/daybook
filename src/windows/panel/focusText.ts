/**
 * The focus editor's keys as text edits: each takes the text and the caret and returns both. A task is
 * a bullet with its checkbox as inline code (``- `[ ]` Task``), so no Obsidian plugin counts it as a
 * todo; a plain `- [ ]` is still understood.
 */

export interface TextEdit {
  text: string;
  caret: number;
}

/** Open, in progress, done, cancelled. */
const MARKS = [" ", "/", "x", "-"];
const CHECKBOX = /^(\s*[-*+]\s`?\[)(.)(\]`?)/;
const ITEM = /^(\s*)([-*+])\s(`?\[.\]`?\s?)?(.*)$/;
/** A task with its checkbox, its project emoji (if any) and its text. */
const TASK = /^\s*[-*+]\s`?\[(.)\]`?\s*(?:(:[\w+-]+:)\s*)?(.*)$/;
const LEADING_EMOJI = /^(:[\w+-]+:)\s*/;
/** An item with nothing typed yet (at most its project emoji). */
const NO_TEXT = /^\s*(:[\w+-]+:)?\s*$/;
const INDENT = "    ";
const OPEN_TASK = "- `[ ]` ";

function lineAt(text: string, caret: number) {
  const start = text.lastIndexOf("\n", caret - 1) + 1;
  const next = text.indexOf("\n", caret);
  const end = next < 0 ? text.length : next;
  return { start, end, line: text.slice(start, end) };
}

function replaceLine(text: string, start: number, end: number, line: string): string {
  return text.slice(0, start) + line + text.slice(end);
}

/** ⌘L: the line's checkbox moves to its next state; a line without one becomes an open task. */
export function cycleCheckbox(text: string, caret: number): TextEdit {
  const { start, end, line } = lineAt(text, caret);
  const box = CHECKBOX.exec(line);
  if (box) {
    const mark = MARKS[(MARKS.indexOf(box[2].toLowerCase()) + 1) % MARKS.length];
    return { text: replaceLine(text, start, end, line.replace(CHECKBOX, `$1${mark}$3`)), caret };
  }
  const task = line.replace(/^\s*(?:[-*+]\s+)?/, OPEN_TASK);
  return { text: replaceLine(text, start, end, task), caret: Math.max(start, caret + task.length - line.length) };
}

/**
 * ⌘1…9: the line becomes a task of the project with this emoji, keeping its checkbox and text. The
 * project the task already has turns it into a plain sub-item instead (the same key turns it back).
 */
export function setLineProject(text: string, caret: number, emoji: string): TextEdit {
  const { start, end, line } = lineAt(text, caret);
  const task = TASK.exec(line);
  const prefix = emoji ? `${OPEN_TASK}${emoji} ` : OPEN_TASK;
  let next: string;
  if (!task) next = prefix + line.replace(/^\s*(?:[-*+]\s+)?/, "");
  else if ((task[2] ?? "") === emoji) next = `${INDENT}- ${task[3]}`;
  else next = `- \`[${task[1]}]\` ${emoji ? `${emoji} ` : ""}${task[3]}`;
  // The caret keeps its distance from the line's end, so it stays in the text.
  const moved = start + next.length - (end - caret);
  return { text: replaceLine(text, start, end, next), caret: Math.min(start + next.length, Math.max(start, moved)) };
}

/** ↵: the next item of the same kind, a task with the line's emoji; on an item without text the list ends. `null`: a plain line break. */
export function continueList(text: string, caret: number): TextEdit | null {
  const { start, end, line } = lineAt(text, caret);
  const item = ITEM.exec(line);
  if (!item || caret !== end) return null;
  if (NO_TEXT.test(item[4])) return { text: replaceLine(text, start, end, ""), caret: start };
  const emoji = item[3] ? LEADING_EMOJI.exec(item[4])?.[1] : undefined;
  const prefix = `${item[1]}${item[2]} ${item[3] ? "`[ ]` " : ""}${emoji ? `${emoji} ` : ""}`;
  return { text: `${text.slice(0, end)}\n${prefix}${text.slice(end)}`, caret: end + 1 + prefix.length };
}

/** Tab: the line moves in (an empty line becomes a sub-item); ⇧Tab moves it out again. */
export function indentLine(text: string, caret: number, outdent: boolean): TextEdit {
  const { start, end, line } = lineAt(text, caret);
  if (outdent) {
    const next = line.replace(/^(?: {1,4}|\t)/, "");
    return { text: replaceLine(text, start, end, next), caret: Math.max(start, caret - (line.length - next.length)) };
  }
  const next = line.trim() ? INDENT + line : `${INDENT}- `;
  return { text: replaceLine(text, start, end, next), caret: caret + next.length - line.length };
}
