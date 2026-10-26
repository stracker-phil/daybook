import type { JournalAnswer, JournalGroupAnswers, JournalGroupView } from "../../api";

export const EMPTY_ANSWER: JournalAnswer = { items: [], tasks: [], text: "" };

export function draftFrom(group: JournalGroupView): JournalGroupAnswers {
  return { answers: Object.fromEntries(group.questions.map((q) => [q.question, q.answer])), note: group.note };
}

function clean(text: string): string {
  return text
    .split("\n")
    .map((line) => line.trimEnd())
    .join("\n")
    .trim();
}

/** The draft as the note stores it: trimmed, without empty rows. Equal keys mean the note holds the draft. */
export function contentKey(draft: JournalGroupAnswers): string {
  const answers = Object.entries(draft.answers)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([question, answer]) => [
      question,
      answer.items.map(clean).filter(Boolean),
      answer.tasks
        .map((task) => [clean(task.text), task.done] as const)
        .filter(([text]) => text),
      clean(answer.text),
    ]);
  return JSON.stringify({ note: clean(draft.note), answers });
}

/** The rows with one empty row at the end, where the next answer goes. */
export function withTrailingRow<T>(rows: T[], isEmpty: (row: T) => boolean, empty: T): T[] {
  return rows.length > 0 && isEmpty(rows[rows.length - 1]) ? rows : [...rows, empty];
}

/** The rows to store: without the trailing empty row. Empty rows in between stay until ⌫ removes them. */
export function withoutTrailingRow<T>(rows: T[], isEmpty: (row: T) => boolean): T[] {
  return rows.filter((row, i) => !isEmpty(row) || i < rows.length - 1);
}
