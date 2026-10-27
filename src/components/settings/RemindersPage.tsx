import type { ReminderRule, Settings, TimeWindow, TodoSpan } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { BufferedInput, ControlRow, EntryFooter, FormRow, Group, LeadSwitch } from "./controls";
import { TimeRangeField } from "./fields";
import { listEditor } from "./listEditor";

type AddableRule = Extract<ReminderRule, { kind: "meeting" | "todos" }>;

const ADDABLE: Record<AddableRule["kind"], string> = { meeting: "Upcoming meeting", todos: "Open todos" };

const isAddable = (rule: ReminderRule): rule is AddableRule => rule.kind in ADDABLE;

const SPANS: { span: TodoSpan; label: string }[] = [
  { span: "today", label: "from today" },
  { span: "week", label: "from this week" },
  { span: "month", label: "from this month" },
  { span: "all", label: "from any day" },
];

function newRule(kind: AddableRule["kind"]): AddableRule {
  if (kind === "meeting") return { kind, enabled: true, minutesBefore: 60 };
  return { kind, enabled: true, span: "today", from: "16:00", to: "" };
}

function dueText(due: TimeWindow | null | undefined, where: string): string {
  if (!due) return `Never due: set a due time in ${where}.`;
  return `While due, ${due.from} to ${due.to || "the end of the day"}, and not done.`;
}

function title(rule: ReminderRule, s: Settings): string {
  if (isAddable(rule)) return ADDABLE[rule.kind];
  switch (rule.kind) {
    case "routine":
      return `Routine: ${s.routines.find((routine) => routine.id === rule.routine)?.name ?? rule.routine}`;
    case "journal":
      return `Journal: ${rule.group}`;
  }
}

/** Routines and journal groups remind while due (set on the routine or group); meetings and todos have their own settings. */
function RuleFields({ rule, s, onChange }: { rule: ReminderRule; s: Settings; onChange: (rule: ReminderRule) => void }) {
  switch (rule.kind) {
    case "meeting":
      return (
        <FormRow label="Minutes before" hint="Minutes before the meeting starts, then while it runs.">
          <BufferedInput
            className="input number"
            type="number"
            aria-label="Minutes before"
            min={0}
            max={240}
            value={String(rule.minutesBefore)}
            onCommit={(minutes) => onChange({ ...rule, minutesBefore: Math.round(Number(minutes)) })}
          />
          <span className="muted">min</span>
        </FormRow>
      );
    case "todos":
      return (
        <>
          <FormRow label="Todos">
            <select className="input" aria-label="Todos from" value={rule.span} onChange={(e) => onChange({ ...rule, span: e.target.value as TodoSpan })}>
              {SPANS.map(({ span, label }) => (
                <option key={span} value={span}>
                  {label}
                </option>
              ))}
            </select>
          </FormRow>
          <FormRow label="Between" hint="An empty end runs to the end of the day.">
            <TimeRangeField label="Reminder" from={rule.from} to={rule.to} openEnd onChange={(range) => onChange({ ...rule, ...range })} />
          </FormRow>
        </>
      );
    case "routine":
      return <FormRow label="Due" hint={dueText(s.routines.find((routine) => routine.id === rule.routine)?.timing.due, "Settings → Routines")} />;
    case "journal":
      return <FormRow label="Due" hint={dueText(s.journal.find((group) => group.title === rule.group)?.timing.due, "Settings → Journal")} />;
  }
}

/** The first enabled rule that applies wins, so the list order is the precedence. */
export function RemindersPage({ s, set }: SettingsPageProps) {
  const rules = s.reminders;
  const edit = listEditor(rules, (reminders) => set({ reminders }));
  return (
    <Group>
      <ControlRow stacked>
        {rules.map((rule, i) => (
          <LeadSwitch
            key={i}
            label={title(rule, s)}
            checked={rule.enabled}
            onChange={(enabled) => edit.change(i, { enabled })}
            accessory={edit.actions(i, isAddable(rule) ? "Remove Reminder" : undefined)}
          >
            {rule.enabled && <RuleFields rule={rule} s={s} onChange={(next) => edit.change(i, next)} />}
          </LeadSwitch>
        ))}
      </ControlRow>
      <EntryFooter>
        {(Object.keys(ADDABLE) as AddableRule["kind"][]).map((kind) => (
          <button key={kind} type="button" className="btn" onClick={() => edit.add(newRule(kind))}>
            Add {ADDABLE[kind]}
          </button>
        ))}
      </EntryFooter>
    </Group>
  );
}
