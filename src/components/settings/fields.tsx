/** Fields shared by the card-per-item settings pages. */
import { useEffect, useState } from "react";
import { ArrowDown, ArrowUp, Trash2 } from "lucide-react";
import type { Context, ContextColor } from "../../api";
import { BufferedInput } from "./controls";

export const CONTEXT_COLORS: ContextColor[] = ["blue", "purple", "pink", "red", "orange", "yellow", "green", "teal", "graphite"];

/** Colours come from the palette hues in styles/context.css. */
export function ColorField({ label, value, onChange }: { label: string; value: ContextColor; onChange: (color: ContextColor) => void }) {
  return (
    <div className="accent-swatches" role="radiogroup" aria-label={label}>
      {CONTEXT_COLORS.map((color) => (
        <button
          key={color}
          type="button"
          role="radio"
          aria-checked={color === value}
          aria-label={color}
          title={color}
          data-context={color}
          onClick={() => onChange(color)}
        />
      ))}
    </div>
  );
}

export interface Choice {
  id: string;
  title: string;
}

/** A context's label (an emoji) and name. */
export const titleOf = (context: Context) => [context.label.trim(), context.name].filter(Boolean).join(" ");

export const contextChoices = (contexts: Context[]): Choice[] => contexts.map((context) => ({ id: context.id, title: titleOf(context) }));

/** Multi-select toggle chips; the chosen ones are pressed. */
export function ToggleChips({ label, choices, chosen, onChange }: { label: string; choices: Choice[]; chosen: string[]; onChange: (ids: string[]) => void }) {
  const toggle = (id: string) => onChange(chosen.includes(id) ? chosen.filter((other) => other !== id) : [...chosen, id]);
  return (
    <div className="segmented chips" role="group" aria-label={label}>
      {choices.map((choice) => (
        <button key={choice.id} type="button" aria-pressed={chosen.includes(choice.id)} onClick={() => toggle(choice.id)}>
          {choice.title}
        </button>
      ))}
    </div>
  );
}

/** Monday first; stored as weekdays with 0 = Sunday. */
const WEEKDAYS: Choice[] = [
  { id: "1", title: "Mo" },
  { id: "2", title: "Tu" },
  { id: "3", title: "We" },
  { id: "4", title: "Th" },
  { id: "5", title: "Fr" },
  { id: "6", title: "Sa" },
  { id: "0", title: "Su" },
];

/** Weekday chips; an empty list means every day (all shown on; none or all saved as []). */
export function DaysField({ label, days, onChange }: { label: string; days: number[]; onChange: (days: number[]) => void }) {
  const shown = days.length > 0 ? days.map(String) : WEEKDAYS.map((day) => day.id);
  const change = (ids: string[]) => onChange(ids.length === 0 || ids.length === WEEKDAYS.length ? [] : ids.map(Number).sort((x, y) => x - y));
  return <ToggleChips label={label} choices={WEEKDAYS} chosen={shown} onChange={change} />;
}

/** An `HH:MM` field that saves on blur or ↵; saving per keystroke would garble the time mid-entry. */
function TimeInput({ label, value, onCommit, openEnd }: { label: string; value: string; onCommit: (value: string) => void; openEnd?: boolean }) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const commit = () => {
    if (draft === value) return;
    if (draft || openEnd) onCommit(draft);
    else setDraft(value);
  };
  return (
    <input
      type="time"
      className="input time"
      aria-label={label}
      value={draft}
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => e.key === "Enter" && e.currentTarget.blur()}
    />
  );
}

/** With `openEnd`, the end may be cleared: the range then runs to the end of the day. */
export function TimeRangeField({ label, from, to, onChange, openEnd }: { label: string; from: string; to: string; onChange: (range: { from: string; to: string }) => void; openEnd?: boolean }) {
  return (
    <div className="time-range">
      <TimeInput label={`${label} from`} value={from} onCommit={(next) => onChange({ from: next, to })} />
      <span className="o2">to</span>
      <TimeInput label={`${label} until`} value={to} openEnd={openEnd} onCommit={(next) => onChange({ from, to: next })} />
    </div>
  );
}

/** Up and down (given `onMove`) and remove (given `onRemove`): icon buttons named by their action. */
export function ItemActions({ index = 0, count = 0, onMove, onRemove, removeLabel = "Remove" }: { index?: number; count?: number; onMove?: (from: number, to: number) => void; onRemove?: () => void; removeLabel?: string }) {
  return (
    <div className="item-actions">
      {onMove && (
        <>
          <button type="button" className="icon-btn" aria-label="Move Up" title="Move Up" disabled={index === 0} onClick={() => onMove(index, index - 1)}>
            <ArrowUp size="1.077rem" aria-hidden="true" />
          </button>
          <button type="button" className="icon-btn" aria-label="Move Down" title="Move Down" disabled={index === count - 1} onClick={() => onMove(index, index + 1)}>
            <ArrowDown size="1.077rem" aria-hidden="true" />
          </button>
        </>
      )}
      {onRemove && (
        <button type="button" className="icon-btn" aria-label={removeLabel} title={removeLabel} onClick={onRemove}>
          <Trash2 size="1.077rem" aria-hidden="true" />
        </button>
      )}
    </div>
  );
}

export function moveItem<T>(list: T[], from: number, to: number): T[] {
  const next = [...list];
  const [item] = next.splice(from, 1);
  next.splice(to, 0, item);
  return next;
}

export function replaceItem<T>(list: T[], index: number, patch: Partial<T>): T[] {
  return list.map((item, i) => (i === index ? { ...item, ...patch } : item));
}

/** `QRS, Initech` ↔ `["QRS", "Initech"]`. */
export const parseList = (text: string) =>
  text
    .split(",")
    .map((word) => word.trim())
    .filter(Boolean);
export const joinList = (words: string[]) => words.join(", ");

/** A bare text input that saves its trimmed text on blur or ↵; an empty value isn't saved when `required`. */
export function CommitInput({ label, value, onCommit, placeholder, required, className }: {
  label: string;
  value: string;
  onCommit: (value: string) => void;
  placeholder?: string;
  required?: boolean;
  className?: string;
}) {
  return (
    <BufferedInput
      className={["input", className].filter(Boolean).join(" ")}
      aria-label={label}
      placeholder={placeholder}
      required={required}
      value={value}
      onCommit={(text) => {
        if (text.trim() !== value) onCommit(text.trim());
      }}
    />
  );
}
