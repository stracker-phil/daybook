import { useRef } from "react";
import type { CalendarInfo, Context, Feature, MusicEntry, WorkHours } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { ContextCalendars } from "./ContextCalendars";
import { ContextMusicFields } from "./ContextMusicFields";
import { Checkbox, EntryFooter, Group, NumberField, Row, Switch, TextField } from "./controls";
import { CONTEXT_COLORS, ColorField, DaysField, ItemActions, TimeRangeField, moveItem, replaceItem } from "./fields";
import { useCalendarInfo } from "./useCalendarInfo";

const FEATURES: { id: Feature; label: string }[] = [
  { id: "status", label: "Status" },
  { id: "slack", label: "Slack status" },
  { id: "jira", label: "Jira" },
  { id: "routines", label: "Routines" },
];

/** Rust clamps `checkInMinutes` to 0…240 (`store/settings/contexts.rs`). */
const MAX_CHECK_IN_MINUTES = 240;

const NEW_WORK_HOURS: WorkHours = { days: [1, 2, 3, 4, 5], from: "09:00", to: "18:00" };

const title = (context: Context) => [context.label.trim(), context.name].filter(Boolean).join(" ");

function WorkHoursRows({ context, onChange }: { context: Context; onChange: (patch: Partial<Context>) => void }) {
  const hours = context.workHours;
  // Switching work hours off and on again brings back the times you had.
  const last = useRef(hours ?? NEW_WORK_HOURS);
  if (hours) last.current = hours;
  return (
    <>
      <Switch
        label="Work hours"
        description="Tinted in the Daybook window's timeline, e.g. your workday or until bedtime."
        checked={!!hours}
        onChange={(on) => onChange({ workHours: on ? last.current : null })}
      />
      {hours && (
        <>
          <Row label="Days and hours" nested>
            <div className="work-hours">
              <DaysField label="Work days" days={hours.days} onChange={(days) => onChange({ workHours: { ...hours, days } })} />
              <TimeRangeField label="Work hours" from={hours.from} to={hours.to} onChange={(range) => onChange({ workHours: { ...hours, ...range } })} />
            </div>
          </Row>
          <Switch
          label="Switch automatically during work hours"
          description="Becomes current when the work hours start; a switch by hand holds until they start again."
          checked={context.autoSwitch}
            onChange={(autoSwitch) => onChange({ autoSwitch })}
          />
        </>
      )}
    </>
  );
}

function ContextCard({ context, scopes, calendars, music, index, count, onChange, onMove, onRemove }: {
  context: Context;
  scopes: string[];
  calendars: CalendarInfo | null;
  music: MusicEntry[];
  index: number;
  count: number;
  onChange: (patch: Partial<Context>) => void;
  onMove: (from: number, to: number) => void;
  onRemove: () => void;
}) {
  const toggle = (feature: Feature, on: boolean) => onChange({ features: on ? [...context.features, feature] : context.features.filter((f) => f !== feature) });
  return (
    <Group title={title(context)}>
      <TextField label="Name" value={context.name} onCommit={(name) => name.trim() && onChange({ name: name.trim() })} />
      <TextField label="Emoji" description="Before the context's name everywhere, and in the menu bar." value={context.label} onCommit={(label) => onChange({ label })} />
      <Row label="Features" stacked>
        <div className="check-row">
          {FEATURES.map((feature) => (
            <Checkbox key={feature.id} label={feature.label} checked={context.features.includes(feature.id)} onChange={(on) => toggle(feature.id, on)} />
          ))}
        </div>
      </Row>
      {context.features.includes("status") && (
        <NumberField
          label="Check-ins"
          description="While Working, asks what you're working on this long after the timer last changed; 0: never."
          suffix="min"
          min={0}
          max={MAX_CHECK_IN_MINUTES}
          value={context.checkInMinutes}
          onCommit={(minutes) => onChange({ checkInMinutes: Math.round(minutes) })}
          nested
        />
      )}
      <Row label="Color" description="Marks this context in overlays, the Daybook window and its timeline.">
        <ColorField label={`${context.name} color`} value={context.color} onChange={(color) => onChange({ color })} />
      </Row>
      <Row label="Search scope" description="What search covers by default in this context.">
        <select className="input" aria-label="Search scope" value={context.searchScope} onChange={(e) => onChange({ searchScope: e.target.value })}>
          {scopes.map((scope) => (
            <option key={scope} value={scope}>
              {scope}
            </option>
          ))}
        </select>
      </Row>
      <WorkHoursRows context={context} onChange={onChange} />
      {calendars && <ContextCalendars info={calendars} chosen={context.calendars} onChange={(next) => onChange({ calendars: next })} />}
      <ContextMusicFields context={context} entries={music} onChange={onChange} />
      <EntryFooter>
        <ItemActions index={index} count={count} onMove={onMove} onRemove={onRemove} removeLabel="Remove Context" />
      </EntryFooter>
    </Group>
  );
}

function newContext(contexts: Context[]): Context {
  return {
    id: "",
    label: "",
    name: "New context",
    features: [],
    searchScope: "all",
    color: CONTEXT_COLORS.find((color) => !contexts.some((context) => context.color === color)) ?? "blue",
    workHours: null,
    autoSwitch: false,
    calendars: [],
    checkInMinutes: 15,
    music: { onCall: "lower", callVolume: 2, pauseWhenAway: true, resume: true, startWith: "" },
  };
}

export function ContextsPage({ s, set }: SettingsPageProps) {
  const contexts = s.contexts;
  const scopes = [...new Set(["all", ...s.search.scopes.map((scope) => scope.name)])];
  const calendars = useCalendarInfo().data;
  const save = (next: Context[]) => set({ contexts: next });
  return (
    <>
      {contexts.map((context, i) => (
        <ContextCard
          key={context.id || i}
          context={context}
          scopes={scopes}
          calendars={calendars}
          music={s.music}
          index={i}
          count={contexts.length}
          onChange={(patch) => save(replaceItem(contexts, i, patch))}
          onMove={(from, to) => save(moveItem(contexts, from, to))}
          onRemove={() => save(contexts.filter((_, j) => j !== i))}
        />
      ))}
      <div>
        <button type="button" className="btn" onClick={() => save([...contexts, newContext(contexts)])}>
          Add Context
        </button>
      </div>
    </>
  );
}
