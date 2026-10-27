import type { Routine, RoutineStep } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { ControlRow, EntryFooter, Group, Row, TextField } from "./controls";
import { CommitInput, DaysField, ItemActions, ToggleChips, contextChoices, moveItem, replaceItem, type Choice } from "./fields";
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
    <ControlRow stacked>
      <div className="item-line">
        <CommitInput label="Item" className="grow" placeholder="What to do" required value={step.title} onCommit={(title) => onChange({ title })} />
        <ItemActions index={index} count={count} onMove={onMove} onRemove={onRemove} removeLabel="Remove Item" />
      </div>
      <div className="item-line">
        <CommitInput label="Link" className="grow" placeholder="Link to open with ⌘O (optional): https://… or a vault path" value={step.link} onCommit={(link) => onChange({ link })} />
        <DaysField label={`${step.title}: due on`} days={step.days} onChange={(days) => onChange({ days })} />
      </div>
    </ControlRow>
  );
}

function RoutineCard({ routine, contexts, index, count, onChange, onMove, onRemove }: {
  routine: Routine;
  contexts: Choice[];
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
        <Row label="Contexts" description="None selected: every context with Routines turned on.">
          <ToggleChips label="Contexts" choices={contexts} chosen={routine.contexts} onChange={(ids) => onChange({ contexts: ids })} />
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
      <EntryFooter>
        <button type="button" className="btn" onClick={() => setSteps([...steps, { title: "New item", link: "", days: [] }])}>
          Add Item
        </button>
        <ItemActions index={index} count={count} onMove={onMove} onRemove={onRemove} removeLabel="Remove Routine" />
      </EntryFooter>
    </Group>
  );
}

/** Rust gives a new routine its id from the name on save; the id stays when the routine is renamed. */
const newRoutine = (): Routine => ({ id: "", name: "New routine", contexts: [], steps: [], timing: { due: null, openWhenDue: false, openOn: [] } });

/** A routine's reminder in Settings → Reminders follows it by its id. */
export function RoutinesPage({ s, set }: SettingsPageProps) {
  const routines = s.routines;
  const contexts = contextChoices(s.contexts);
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
