import type { ReactNode } from "react";
import type { Routine, RoutineStep } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { AddButton, ControlRow, EntryFooter, Group, Row, TextField } from "./controls";
import { CommitInput, DaysField, ToggleChips, contextChoices, type Choice } from "./fields";
import { listEditor } from "./listEditor";
import { TimingFields } from "./TimingFields";

function StepRow({ step, actions, onChange }: { step: RoutineStep; actions: ReactNode; onChange: (patch: Partial<RoutineStep>) => void }) {
  return (
    <ControlRow stacked>
      <div className="item-line">
        <CommitInput label="Item" className="grow" placeholder="What to do" required value={step.title} onCommit={(title) => onChange({ title })} />
        {actions}
      </div>
      <div className="item-line">
        <CommitInput label="Link" className="grow" placeholder="Link to open with ⌘O (optional): https://… or a vault path" value={step.link} onCommit={(link) => onChange({ link })} />
        <DaysField label={`${step.title}: due on`} days={step.days} onChange={(days) => onChange({ days })} />
      </div>
    </ControlRow>
  );
}

function RoutineCard({ routine, contexts, actions, onChange }: { routine: Routine; contexts: Choice[]; actions: ReactNode; onChange: (patch: Partial<Routine>) => void }) {
  const steps = routine.steps;
  const edit = listEditor(steps, (next) => onChange({ steps: next }));
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
          actions={edit.actions(i, "Remove Item")}
          onChange={(patch) => edit.change(i, patch)}
        />
      ))}
      <EntryFooter>
        <button type="button" className="btn" onClick={() => edit.add({ title: "New item", link: "", days: [] })}>
          Add Item
        </button>
        {actions}
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
  const edit = listEditor(routines, (next) => set({ routines: next }));
  return (
    <>
      {routines.map((routine, i) => (
        <RoutineCard
          key={i}
          routine={routine}
          contexts={contexts}
          actions={edit.actions(i, "Remove Routine")}
          onChange={(patch) => edit.change(i, patch)}
        />
      ))}
      <AddButton label="Add Routine" onClick={() => edit.add(newRoutine())} />
    </>
  );
}
