import { useEffect, useRef, useState } from "react";
import { api, type JournalGroupAnswers, type JournalGroupView } from "../../api";
import { useAutosave } from "../../lib/useAutosave";
import { GrowingText, QuestionField } from "./JournalAnswers";
import { contentKey, draftFrom, EMPTY_ANSWER } from "./journalDraft";

const AUTOSAVE_MS = 800;

/**
 * One group as a form, saved when typing stops and when the form goes (another group, the overlay closing).
 * Answers edited in Obsidian replace the draft; the note echoing a save doesn't, so the caret stays.
 */
export function JournalForm({ group, onError }: { group: JournalGroupView; onError: (message: string | null) => void }) {
  const [draft, setDraft] = useState(() => draftFrom(group));
  const shown = useRef(draft);
  const form = useRef<HTMLDivElement>(null);
  const { change, flush, isDirty } = useAutosave<JournalGroupAnswers>({
    delayMs: AUTOSAVE_MS,
    save: async (next) => {
      await api.journalSave(group.title, next);
      onError(null);
    },
    onError,
  });

  useEffect(() => {
    const next = draftFrom(group);
    if (isDirty() || contentKey(next) === contentKey(shown.current)) return;
    shown.current = next;
    setDraft(next);
  }, [group, isDirty]);

  useEffect(() => () => void flush(), [flush]);

  useEffect(() => {
    form.current?.querySelector<HTMLElement>("[data-journal-field]")?.focus();
  }, []);

  const edit = (next: JournalGroupAnswers) => {
    shown.current = next;
    setDraft(next);
    change(next);
  };

  return (
    <div ref={form} data-journal-form className="journal-form">
      {group.questions.length === 0 ? (
        <GrowingText label={group.title} value={draft.note} placeholder={`How is your ${group.title.toLowerCase()} today?`} onChange={(note) => edit({ ...draft, note })} />
      ) : (
        group.questions.map((question) => (
          <QuestionField
            key={question.question}
            question={question}
            answer={draft.answers[question.question] ?? EMPTY_ANSWER}
            onChange={(answer) => edit({ ...draft, answers: { ...draft.answers, [question.question]: answer } })}
          />
        ))
      )}
    </div>
  );
}
