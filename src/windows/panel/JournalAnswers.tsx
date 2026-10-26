import { useLayoutEffect, useRef, type KeyboardEvent } from "react";
import type { JournalAnswer, JournalQuestionView, JournalTask } from "../../api";
import { withTrailingRow, withoutTrailingRow } from "./journalDraft";

/** Moves the caret to the next (or previous) answer field of the form, in reading order. */
function focusSibling(from: HTMLElement, step: 1 | -1) {
  const form = from.closest("[data-journal-form]");
  if (!form) return;
  const fields = [...form.querySelectorAll<HTMLElement>("[data-journal-field]")];
  const next = fields[fields.indexOf(from) + step];
  if (!next) return;
  next.focus();
  if (next instanceof HTMLInputElement) next.setSelectionRange(next.value.length, next.value.length);
}

/** ↵ goes to the next row (an empty one appears as soon as you type), ↑↓ move, ⌫ on an empty row removes it. */
function rowKeys(e: KeyboardEvent<HTMLInputElement>, value: string, isTrailing: boolean, remove: () => void) {
  if (e.nativeEvent.isComposing || e.metaKey) return;
  const field = e.currentTarget;
  if (e.key === "Enter") {
    e.preventDefault();
    // The new trailing row renders first.
    requestAnimationFrame(() => focusSibling(field, 1));
  } else if (e.key === "ArrowDown" || e.key === "ArrowUp") {
    e.preventDefault();
    focusSibling(field, e.key === "ArrowDown" ? 1 : -1);
  } else if (e.key === "Backspace" && value === "" && !isTrailing) {
    e.preventDefault();
    focusSibling(field, -1);
    remove();
  }
}

/** Only the last, empty row gets a placeholder. */
function placeholder(index: number, count: number, first: string): string | undefined {
  if (index !== count - 1) return undefined;
  return index === 0 ? first : "Another one";
}

type AnswerProps = { question: string; answer: JournalAnswer; onChange: (answer: JournalAnswer) => void };

function ListAnswer({ question, answer, onChange }: AnswerProps) {
  const isEmpty = (item: string) => item === "";
  const rows = withTrailingRow(answer.items, isEmpty, "");
  const update = (items: string[]) => onChange({ ...answer, items: withoutTrailingRow(items, isEmpty) });
  return (
    <ol className="journal-rows">
      {rows.map((item, index) => (
        <li key={index} className="journal-row">
          <span className="journal-number o3">{index + 1}.</span>
          <input
            data-journal-field
            aria-label={`${question} ${index + 1}`}
            value={item}
            placeholder={placeholder(index, rows.length, "Answer")}
            onChange={(e) => update(rows.map((row, i) => (i === index ? e.target.value : row)))}
            onKeyDown={(e) => rowKeys(e, item, index === rows.length - 1, () => update(rows.filter((_, i) => i !== index)))}
          />
        </li>
      ))}
    </ol>
  );
}

function TaskAnswer({ question, answer, onChange }: AnswerProps) {
  const isEmpty = (task: JournalTask) => task.text === "";
  const rows = withTrailingRow(answer.tasks, isEmpty, { text: "", done: false });
  const update = (tasks: JournalTask[]) => onChange({ ...answer, tasks: withoutTrailingRow(tasks, isEmpty) });
  const patch = (index: number, change: Partial<JournalTask>) => update(rows.map((row, i) => (i === index ? { ...row, ...change } : row)));
  return (
    <ul className="journal-rows">
      {rows.map((task, index) => (
        <li key={index} className="journal-row" data-done={task.done}>
          <input type="checkbox" aria-label="Done" checked={task.done} disabled={!task.text} onChange={(e) => patch(index, { done: e.target.checked })} />
          <input
            data-journal-field
            aria-label={`${question} ${index + 1}`}
            value={task.text}
            placeholder={placeholder(index, rows.length, "Optional")}
            onChange={(e) => patch(index, { text: e.target.value })}
            onKeyDown={(e) => rowKeys(e, task.text, index === rows.length - 1, () => update(rows.filter((_, i) => i !== index)))}
          />
        </li>
      ))}
    </ul>
  );
}

/** A text field that grows with its text. */
export function GrowingText({ label, value, placeholder, onChange }: { label: string; value: string; placeholder: string; onChange: (value: string) => void }) {
  const field = useRef<HTMLTextAreaElement>(null);
  useLayoutEffect(() => {
    const element = field.current;
    if (!element) return;
    element.style.height = "auto";
    element.style.height = `${element.scrollHeight}px`;
  }, [value]);
  return (
    <textarea
      ref={field}
      data-journal-field
      className="journal-text"
      aria-label={label}
      rows={2}
      value={value}
      placeholder={placeholder}
      onChange={(e) => onChange(e.target.value)}
    />
  );
}

export function QuestionField({ question, answer, onChange }: { question: JournalQuestionView; answer: JournalAnswer; onChange: (answer: JournalAnswer) => void }) {
  const props = { question: question.question, answer, onChange };
  return (
    <section className="journal-question" aria-label={question.question}>
      <div className="journal-question-title">{question.question}</div>
      {question.hint && <div className="journal-hint o3">{question.hint}</div>}
      {question.kind === "list" && <ListAnswer {...props} />}
      {question.kind === "task" && <TaskAnswer {...props} />}
      {question.kind === "text" && <GrowingText label={question.question} value={answer.text} placeholder="Answer" onChange={(text) => onChange({ ...answer, text })} />}
    </section>
  );
}
