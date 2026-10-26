import type { Routine, RoutineStep } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, Row, TextField } from "./controls";
import { ChoiceList, CommitInput, DaysField, ItemActions, moveItem, replaceItem } from "./fields";
import { TimingFields } from "./TimingFields";

function StepRow({ step, index, count, onChange, onMove, onRemove }: {
  step: RoutineStep;
  index: number;
  count: number;
  onChange: (patch: Partial<RoutineStep>) => void;
  onMove: (from: number, to: number) => void;
  onRemove: () => void;
}) {
  return (
    <Row label={`Item ${index + 1}`} stacked>
      <div className="routine-step-fields">
        <CommitInput label="Item" placeholder="What to do" required value={step.title} onCommit={(title) => onChange({ title: title.trim() })} />
        <CommitInput label="Link" placeholder="Link to open with ⌘O (optional): https://… or a vault path" value={step.link} onCommit={(link) => onChange({ link: link.trim() })} />
        <div className="routine-step-line">
          <DaysField label={`${step.title}: due on`} days={step.days} onChange={(days) => onChange({ days })} />
          <ItemActions index={index} count={count} onMove={onMove} onRemove={onRemove} removeLabel="Remove Item" />
        </div>
      </div>
    </Row>
  );
}

function RoutineCard({ routine, contexts, index, count, onChange, onMove, onRemove }: {
  routine: Routine;
  contexts: { id: string; title: string }[];
  index: number;
  count: number;
  onChange: (patch: Partial<Routine>) => void;
  onMove: (from: number, to: number) => void;
  onRemove: () => void;
}) {
  const steps = routine.steps;
  const setSteps = (next: RoutineStep[]) => onChange({ steps: next });
  return (
    <Group title={routine.name} description="Items are due every day unless you pick weekdays, e.g. only Fr for a weekly task.">
      <TextField label="Name" required value={routine.name} onCommit={(name) => onChange({ name: name.trim() })} />
      {contexts.length > 1 && (
        <Row label="Contexts" description="Runs only while one of these is current. None: every context with Routines turned on." stacked>
          <ChoiceList label="Contexts" choices={contexts} chosen={routine.contexts} onChange={(ids) => onChange({ contexts: ids })} placeholder="Add: type part of a context's name, ↵ adds it" />
        </Row>
      )}
      <TimingFields timing={routine.timing} contextEvent="When switching to one of its contexts" onChange={(timing) => onChange({ timing })} />
      {steps.map((step, i) => (
        <StepRow
          key={i}
          step={step}
          index={i}
          count={steps.length}
          onChange={(patch) => setSteps(replaceItem(steps, i, patch))}
          onMove={(from, to) => setSteps(moveItem(steps, from, to))}
          onRemove={() => setSteps(steps.filter((_, j) => j !== i))}
        />
      ))}
      <Row label="">
        <div className="journal-group-actions">
          <button type="button" className="btn" onClick={() => setSteps([...steps, { title: "New item", link: "", days: [] }])}>
            Add Item
          </button>
          <ItemActions index={index} count={count} onMove={onMove} onRemove={onRemove} removeLabel="Remove Routine" />
        </div>
      </Row>
    </Group>
  );
}

/** Rust gives a new routine its id from the name on save; the id stays when the routine is renamed. */
const newRoutine = (): Routine => ({ id: "", name: "New routine", contexts: [], steps: [], timing: { due: null, openWhenDue: false, openOn: [] } });

/** A routine's reminder in Settings → Reminders follows it by its id. */
export function RoutinesPage({ s, set }: SettingsPageProps) {
  const routines = s.routines;
  const contexts = s.contexts.map((context) => ({ id: context.id, title: [context.label.trim(), context.name].filter(Boolean).join(" ") }));
  const save = (next: Routine[]) => set({ routines: next });
  return (
    <>
      {routines.map((routine, i) => (
        <RoutineCard
          key={i}
          routine={routine}
          contexts={contexts}
          index={i}
          count={routines.length}
          onChange={(patch) => save(replaceItem(routines, i, patch))}
          onMove={(from, to) => save(moveItem(routines, from, to))}
          onRemove={() => save(routines.filter((_, j) => j !== i))}
        />
      ))}
      <div>
        <button type="button" className="btn" onClick={() => save([...routines, newRoutine()])}>
          Add Routine
        </button>
      </div>
    </>
  );
}
