/** The editor's ⌘B, ⌘I and ⌘L as pure edits of the text and the selection, applied as one change. */

/** Replace `from`–`to` with `text`, then select `select`. */
export interface Edit {
  from: number;
  to: number;
  text: string;
  select: [number, number];
}

/** How many `char`s run directly before `index` (backwards) or from it (forwards). */
function runBefore(value: string, index: number, char: string): number {
  let count = 0;
  while (value[index - 1 - count] === char) count++;
  return count;
}
function runFrom(value: string, index: number, char: string): number {
  let count = 0;
  while (value[index + count] === char) count++;
  return count;
}

/** Does a run of markers carry this emphasis? Bold needs 2 or more, italic an odd count. */
const carries = (run: number, marker: string) => (marker.length === 2 ? run >= 2 : run % 2 === 1);

/**
 * ⌘B / ⌘I: wrap the selection in `**` / `*`, or unwrap it when it's already wrapped (markers
 * just outside or included in the selection). Without a selection, the markers go around the caret.
 */
export function toggleWrap(value: string, start: number, end: number, marker: "**" | "*"): Edit {
  const char = marker[0];
  const size = marker.length;
  const before = runBefore(value, start, char);
  const after = runFrom(value, end, char);
  if (start < end && carries(before, marker) && carries(after, marker)) {
    const text = value.slice(start, end);
    return { from: start - size, to: end + size, text, select: [start - size, end - size] };
  }
  const selected = value.slice(start, end);
  const lead = runFrom(selected, 0, char);
  const trail = runBefore(selected, selected.length, char);
  if (selected.length > 2 * size && carries(lead, marker) && carries(trail, marker)) {
    const text = selected.slice(size, -size);
    return { from: start, to: end, text, select: [start, start + text.length] };
  }
  return {
    from: start,
    to: end,
    text: `${marker}${selected}${marker}`,
    select: [start + size, end + size],
  };
}

const TASK = /^(\s*[-*+]\s+)\[([ xX])\]/;
const ITEM = /^(\s*)([-*+]|\d+[.)])\s+/;

function toggledTask(line: string, blankToo: boolean): string {
  const task = TASK.exec(line);
  if (task) return `${task[1]}[${task[2] === " " ? "x" : " "}]${line.slice(task[0].length)}`;
  const item = ITEM.exec(line);
  if (item) return `${item[0]}[ ] ${line.slice(item[0].length)}`;
  if (!line.trim() && !blankToo) return line;
  const indent = line.slice(0, line.length - line.trimStart().length);
  return `${indent}- [ ] ${line.slice(indent.length)}`;
}

/**
 * ⌘L, like Obsidian: a line becomes a todo (`- [ ] `), a list item gains a checkbox, and a todo
 * is ticked or unticked. With several lines selected, each non-empty line changes.
 */
export function toggleTask(value: string, start: number, end: number): Edit {
  const from = value.lastIndexOf("\n", start - 1) + 1;
  const lastEnd = end > start && value[end - 1] === "\n" ? end - 1 : end;
  const newline = value.indexOf("\n", lastEnd);
  const to = newline === -1 ? value.length : newline;
  const lines = value.slice(from, to).split("\n");
  const text = lines.map((line) => toggledTask(line, lines.length === 1)).join("\n");
  if (lines.length > 1) return { from, to, text, select: [from, from + text.length] };
  const shift = text.length - (to - from);
  const clamp = (index: number) => Math.min(from + text.length, Math.max(from, index + shift));
  return { from, to, text, select: [clamp(start), clamp(end)] };
}
