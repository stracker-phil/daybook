import type { ReminderRule, Settings, TimeWindow, TodoSpan } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, NumberField, Row, Switch } from "./controls";
import { ItemActions, TimeRangeField, moveItem } from "./fields";

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
        <NumberField
          label="Minutes before"
          description="Then while it runs; a running meeting wins over the next."
          min={0}
          max={240}
          suffix="min"
          value={rule.minutesBefore}
          onCommit={(minutes) => onChange({ ...rule, minutesBefore: Math.round(minutes) })}
        />
      );
    case "todos":
      return (
        <>
          <Row label="Todos" nested>
            <select className="input" aria-label="Todos from" value={rule.span} onChange={(e) => onChange({ ...rule, span: e.target.value as TodoSpan })}>
              {SPANS.map(({ span, label }) => (
                <option key={span} value={span}>
                  {label}
                </option>
              ))}
            </select>
          </Row>
          <Row label="Between" description="An empty end runs to the end of the day." nested>
            <TimeRangeField label="Reminder" from={rule.from} to={rule.to} openEnd onChange={(range) => onChange({ ...rule, ...range })} />
          </Row>
        </>
      );
    case "routine":
      return <Row label="Due" description={dueText(s.routines.find((routine) => routine.id === rule.routine)?.timing.due, "Settings → Routines")} nested />;
    case "journal":
      return <Row label="Due" description={dueText(s.journal.find((group) => group.title === rule.group)?.timing.due, "Settings → Journal")} nested />;
  }
}

export function RemindersPage({ s, set }: SettingsPageProps) {
  const rules = s.reminders;
  const save = (reminders: ReminderRule[]) => set({ reminders });
  const replace = (index: number, rule: ReminderRule) => save(rules.map((entry, i) => (i === index ? rule : entry)));
  return (
    <>
      {rules.map((rule, i) => (
        <Group key={i} title={title(rule, s)}>
          <Switch label="Remind" checked={rule.enabled} onChange={(enabled) => replace(i, { ...rule, enabled })} />
          {rule.enabled && <RuleFields rule={rule} s={s} onChange={(next) => replace(i, next)} />}
          <Row label="">
            <ItemActions
              index={i}
              count={rules.length}
              onMove={(from, to) => save(moveItem(rules, from, to))}
              onRemove={isAddable(rule) ? () => save(rules.filter((_, j) => j !== i)) : undefined}
              removeLabel="Remove Reminder"
            />
          </Row>
        </Group>
      ))}
      <div className="button-row">
        {(Object.keys(ADDABLE) as AddableRule["kind"][]).map((kind) => (
          <button key={kind} type="button" className="btn" onClick={() => save([...rules, newRule(kind)])}>
            Add {ADDABLE[kind]}
          </button>
        ))}
      </div>
    </>
  );
}
