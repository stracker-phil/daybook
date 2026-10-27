import type { ReactNode } from "react";
import type { AnswerKind, JournalGroup, JournalQuestion } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { AddButton, ControlRow, EntryFooter, Group, Switch, TextField } from "./controls";
import { CommitInput } from "./fields";
import { listEditor } from "./listEditor";
import { TimingFields } from "./TimingFields";

const KINDS: { value: AnswerKind; label: string }[] = [
  { value: "list", label: "List (1, 2, 3 …)" },
  { value: "task", label: "Task (optional)" },
  { value: "text", label: "Text" },
];

function QuestionRow({ question, actions, onChange }: { question: JournalQuestion; actions: ReactNode; onChange: (patch: Partial<JournalQuestion>) => void }) {
  return (
    <ControlRow stacked>
      <div className="item-line">
        <CommitInput label="Question" className="grow" placeholder="Question" required value={question.question} onCommit={(text) => onChange({ question: text })} />
        <select className="input" aria-label="Answer" value={question.kind} onChange={(e) => onChange({ kind: e.target.value as AnswerKind })}>
          {KINDS.map((kind) => (
            <option key={kind.value} value={kind.value}>
              {kind.label}
            </option>
          ))}
        </select>
        {actions}
      </div>
      <CommitInput label="Hint" placeholder="Hint (optional), shown in italics" value={question.hint} onCommit={(hint) => onChange({ hint })} />
    </ControlRow>
  );
}

function GroupCard({ group, actions, onChange }: { group: JournalGroup; actions: ReactNode; onChange: (patch: Partial<JournalGroup>) => void }) {
  const questions = group.questions;
  const edit = listEditor(questions, (next) => onChange({ questions: next }));
  return (
    <Group title={group.title} description={questions.length === 0 ? "No questions: a free-text entry." : undefined}>
      <TextField label="Name" description="The callout's title in the note." required value={group.title} onCommit={(title) => onChange({ title: title.trim() })} />
      <TextField label="Callout type" description="NOTE, HEART, TIP, QUOTE … as in > [!NOTE]." required value={group.callout} onCommit={(callout) => onChange({ callout: callout.trim().toUpperCase() })} />
      <Switch label="Folded" description="Written as a collapsed callout (> [!HEART]-)." checked={group.folded} onChange={(folded) => onChange({ folded })} />
      <TimingFields timing={group.timing} contextEvent="When switching contexts" onChange={(timing) => onChange({ timing })} />
      {questions.map((question, i) => (
        <QuestionRow key={i} question={question} actions={edit.actions(i, "Remove Question")} onChange={(patch) => edit.change(i, patch)} />
      ))}
      <EntryFooter>
        <button type="button" className="btn" onClick={() => edit.add({ question: "New question", hint: "", kind: "text" })}>
          Add Question
        </button>
        {actions}
      </EntryFooter>
    </Group>
  );
}

const newGroup = (): JournalGroup => ({ title: "New group", callout: "NOTE", folded: false, questions: [], timing: { due: null, openWhenDue: false, openOn: [] } });

/** A renamed group keeps its reminder; Rust matches it on save. */
export function JournalPage({ s, set }: SettingsPageProps) {
  const groups = s.journal;
  const edit = listEditor(groups, (journal) => set({ journal }));
  return (
    <>
      {groups.map((group, i) => (
        <GroupCard key={i} group={group} actions={edit.actions(i, "Remove Group")} onChange={(patch) => edit.change(i, patch)} />
      ))}
      <AddButton label="Add Group" onClick={() => edit.add(newGroup())} />
    </>
  );
}
