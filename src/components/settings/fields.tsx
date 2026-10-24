/** Fields shared by the card-per-item settings pages. */
import { useEffect, useState, type KeyboardEvent } from "react";
import { X } from "lucide-react";
import type { ContextColor } from "../../api";

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

/** Monday first; stored as weekdays with 0 = Sunday. */
const WEEKDAYS: { value: number; label: string }[] = [
  { value: 1, label: "Mo" },
  { value: 2, label: "Tu" },
  { value: 3, label: "We" },
  { value: 4, label: "Th" },
  { value: 5, label: "Fr" },
  { value: 6, label: "Sa" },
  { value: 0, label: "Su" },
];

/** Weekday toggles; an empty list means every day (all shown on; none or all saved as []). */
export function DaysField({ label, days, onChange }: { label: string; days: number[]; onChange: (days: number[]) => void }) {
  const shown = days.length > 0 ? days : WEEKDAYS.map((day) => day.value);
  const toggle = (value: number) => {
    const next = shown.includes(value) ? shown.filter((day) => day !== value) : [...shown, value];
    onChange(next.length === 0 || next.length === WEEKDAYS.length ? [] : next.sort((a, b) => a - b));
  };
  return (
    <div className="segmented" role="group" aria-label={label}>
      {WEEKDAYS.map((day) => (
        <button key={day.value} type="button" aria-pressed={shown.includes(day.value)} onClick={() => toggle(day.value)}>
          {day.label}
        </button>
      ))}
    </div>
  );
}

/** An `HH:MM` field that saves on blur or ↵; saving per keystroke would garble the time mid-entry. */
function TimeInput({ label, value, onCommit }: { label: string; value: string; onCommit: (value: string) => void }) {
  const [draft, setDraft] = useState(value);
  useEffect(() => setDraft(value), [value]);
  const commit = () => {
    if (draft === value) return;
    if (draft) onCommit(draft);
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

export function TimeRangeField({ label, from, to, onChange }: { label: string; from: string; to: string; onChange: (range: { from: string; to: string }) => void }) {
  return (
    <div className="time-range">
      <TimeInput label={`${label} from`} value={from} onCommit={(next) => onChange({ from: next, to })} />
      <span className="o2">to</span>
      <TimeInput label={`${label} until`} value={to} onCommit={(next) => onChange({ from, to: next })} />
    </div>
  );
}

export interface Choice {
  id: string;
  title: string;
}

const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };

/**
 * Several picks: the chosen ones inline, each removed with one click, and a type-to-add field that keeps the
 * focus (↑↓ pick a match, ↵ adds it).
 */
export function ChoiceList({ label, choices, chosen, onChange, placeholder }: { label: string; choices: Choice[]; chosen: string[]; onChange: (ids: string[]) => void; placeholder: string }) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const text = query.trim().toLowerCase();
  const matches = text ? choices.filter((choice) => !chosen.includes(choice.id) && choice.title.toLowerCase().includes(text)) : [];
  const current = Math.min(active, Math.max(0, matches.length - 1));
  const add = (choice: Choice | undefined) => {
    if (!choice) return;
    onChange([...chosen, choice.id]);
    setQuery("");
    setActive(0);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    const step = STEPS[e.key];
    if (step && matches.length > 0) {
      e.preventDefault();
      setActive((current + step + matches.length) % matches.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      add(matches[current]);
    }
  };
  const titleOf = (id: string) => choices.find((choice) => choice.id === id)?.title ?? id;

  return (
    <div className="choice-list">
      {chosen.length > 0 && (
        <ul aria-label={label}>
          {chosen.map((id) => (
            <li key={id}>
              <span className="grow-text">{titleOf(id)}</span>
              <button type="button" className="icon-btn tiny" aria-label={`Remove ${titleOf(id)}`} onClick={() => onChange(chosen.filter((other) => other !== id))}>
                <X size="0.846rem" aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
      <input
        className="input"
        aria-label={`Add to ${label}`}
        placeholder={placeholder}
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setActive(0);
        }}
        onKeyDown={onKeyDown}
      />
      {text && (
        <ul className="choice-matches" role="listbox" aria-label={`Matching ${label}`}>
          {matches.length === 0 && <li className="o3">No match.</li>}
          {matches.map((choice, i) => (
            <li
              key={choice.id}
              role="option"
              aria-selected={i === current}
              className={i === current ? "active" : undefined}
              onMouseDown={(e) => e.preventDefault()}
              onMouseEnter={() => setActive(i)}
              onClick={() => add(choice)}
            >
              {choice.title}
            </li>
          ))}
        </ul>
      )}
    </div>
  );
}

/** Up, down and, for a list you can add to, remove. */
export function ItemActions({ index, count, onMove, onRemove, removeLabel }: { index: number; count: number; onMove: (from: number, to: number) => void; onRemove?: () => void; removeLabel?: string }) {
  return (
    <div className="item-actions">
      <button type="button" className="btn btn-quiet" disabled={index === 0} onClick={() => onMove(index, index - 1)}>
        Move Up
      </button>
      <button type="button" className="btn btn-quiet" disabled={index === count - 1} onClick={() => onMove(index, index + 1)}>
        Move Down
      </button>
      {onRemove && (
        <button type="button" className="btn btn-danger" onClick={onRemove}>
          {removeLabel}
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
