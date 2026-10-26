import type { AnswerKind, JournalGroup, JournalQuestion } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, Row, Switch, TextField } from "./controls";
import { CommitInput, ItemActions, moveItem, replaceItem } from "./fields";
import { TimingFields } from "./TimingFields";

const KINDS: { value: AnswerKind; label: string }[] = [
  { value: "list", label: "List (1, 2, 3 …)" },
  { value: "task", label: "Task (optional)" },
  { value: "text", label: "Text" },
];

function QuestionRow({ question, index, count, onChange, onMove, onRemove }: {
  question: JournalQuestion;
  index: number;
  count: number;
  onChange: (patch: Partial<JournalQuestion>) => void;
  onMove: (from: number, to: number) => void;
  onRemove: () => void;
}) {
  return (
    <Row label={`Question ${index + 1}`} stacked>
      <div className="journal-question-fields">
        <div className="journal-question-line">
          <CommitInput label="Question" className="grow" required value={question.question} onCommit={(text) => onChange({ question: text })} />
          <select className="input" aria-label="Answer" value={question.kind} onChange={(e) => onChange({ kind: e.target.value as AnswerKind })}>
            {KINDS.map((kind) => (
              <option key={kind.value} value={kind.value}>
                {kind.label}
              </option>
            ))}
          </select>
        </div>
        <CommitInput label="Hint" placeholder="Hint (optional), shown in italics" value={question.hint} onCommit={(hint) => onChange({ hint })} />
        <ItemActions index={index} count={count} onMove={onMove} onRemove={onRemove} removeLabel="Remove Question" />
      </div>
    </Row>
  );
}

function GroupCard({ group, index, count, onChange, onMove, onRemove }: {
  group: JournalGroup;
  index: number;
  count: number;
  onChange: (patch: Partial<JournalGroup>) => void;
  onMove: (from: number, to: number) => void;
  onRemove: () => void;
}) {
  const questions = group.questions;
  const setQuestions = (next: JournalQuestion[]) => onChange({ questions: next });
  return (
    <Group title={group.title} description={questions.length === 0 ? "No questions: a free-text entry." : undefined}>
      <TextField label="Name" description="The callout's title in the note." required value={group.title} onCommit={(title) => onChange({ title: title.trim() })} />
      <TextField label="Callout type" description="NOTE, HEART, TIP, QUOTE … as in > [!NOTE]." required value={group.callout} onCommit={(callout) => onChange({ callout: callout.trim().toUpperCase() })} />
      <Switch label="Folded" description="Written as a collapsed callout (> [!HEART]-)." checked={group.folded} onChange={(folded) => onChange({ folded })} />
      <TimingFields timing={group.timing} contextEvent="When switching contexts" onChange={(timing) => onChange({ timing })} />
      {questions.map((question, i) => (
        <QuestionRow
          key={i}
          question={question}
          index={i}
          count={questions.length}
          onChange={(patch) => setQuestions(replaceItem(questions, i, patch))}
          onMove={(from, to) => setQuestions(moveItem(questions, from, to))}
          onRemove={() => setQuestions(questions.filter((_, j) => j !== i))}
        />
      ))}
      <Row label="">
        <div className="journal-group-actions">
          <button type="button" className="btn" onClick={() => setQuestions([...questions, { question: "New question", hint: "", kind: "text" }])}>
            Add Question
          </button>
          <ItemActions index={index} count={count} onMove={onMove} onRemove={onRemove} removeLabel="Remove Group" />
        </div>
      </Row>
    </Group>
  );
}

const newGroup = (): JournalGroup => ({ title: "New group", callout: "NOTE", folded: false, questions: [], timing: { due: null, openWhenDue: false, openOn: [] } });

/** A renamed group keeps its reminder; Rust matches it on save. */
export function JournalPage({ s, set }: SettingsPageProps) {
  const groups = s.journal;
  const save = (journal: JournalGroup[]) => set({ journal });
  return (
    <>
      {groups.map((group, i) => (
        <GroupCard
          key={i}
          group={group}
          index={i}
          count={groups.length}
          onChange={(patch) => save(replaceItem(groups, i, patch))}
          onMove={(from, to) => save(moveItem(groups, from, to))}
          onRemove={() => save(groups.filter((_, j) => j !== i))}
        />
      ))}
      <div>
        <button type="button" className="btn" onClick={() => save([...groups, newGroup()])}>
          Add Group
        </button>
      </div>
    </>
  );
}
