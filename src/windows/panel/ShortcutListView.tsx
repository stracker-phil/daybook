import { Fragment, useState, type KeyboardEvent } from "react";
import type { LucideIcon } from "lucide-react";
import { api, toAppError } from "../../api";
import { cycle, useActiveScroll } from "./listKeys";

export interface CommandRow {
  key: string;
  title: string;
  /** Rows of one group follow each other; the heading shows before the first. */
  group: string;
  /** Matched by the typed words along with the title. */
  detail?: string;
  /** The bus command that opens the row. */
  command: string;
  /** `data-*` attributes, e.g. for Stage. */
  data?: Record<string, string>;
}

interface ShortcutListViewProps {
  icon: LucideIcon;
  inputLabel: string;
  placeholder: string;
  label: string;
  rows: CommandRow[] | null;
  /** Shown when there are no rows at all. */
  emptyText: string;
  /** Shown when the typed words hide every row. */
  noMatchText: string;
  hints: string;
  loadError: string | null;
}

/** Rows numbered ⌘1…⌘9. */
const SHORTCUT_ROWS = 9;
const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };

/** ⌘1…⌘9 as 1…9, else 0; ⌘ alone, so ⌃ and ⌥ combinations stay free. */
function commandDigit(e: KeyboardEvent): number {
  if (!e.metaKey || e.ctrlKey || e.altKey) return 0;
  const digit = Number(/^Digit(\d)$/.exec(e.code)?.[1] ?? e.key);
  return Number.isInteger(digit) && digit >= 1 && digit <= SHORTCUT_ROWS ? digit : 0;
}

function matching(rows: CommandRow[], query: string): CommandRow[] {
  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  return rows.filter((row) => {
    const text = `${row.title} ${row.detail ?? ""}`.toLowerCase();
    return words.every((word) => text.includes(word));
  });
}

/**
 * A filterable list under group headings; the first rows open with ⌘1…⌘9. A click selects a row, a click on the
 * selected one runs its command. Rust closes the overlay.
 */
export function ShortcutListView({ icon: Icon, inputLabel, placeholder, label, rows, emptyText, noMatchText, hints, loadError }: ShortcutListViewProps) {
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [runError, setRunError] = useState<string | null>(null);
  const shown = matching(rows ?? [], query);
  const current = Math.min(selected, Math.max(0, shown.length - 1));
  const activeRef = useActiveScroll<HTMLLIElement>(current);
  const error = runError ?? loadError;

  const open = (row: CommandRow | undefined) => {
    if (row) api.runCommand(row.command).catch((e) => setRunError(toAppError(e).message));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) return;
    const step = STEPS[e.key];
    const digit = commandDigit(e);
    if (step && shown.length > 0) {
      e.preventDefault();
      setSelected(cycle(current, step, shown.length));
    } else if (e.key === "Enter") {
      e.preventDefault();
      open(shown[current]);
    } else if (digit) {
      e.preventDefault();
      open(shown[digit - 1]);
    }
  };

  return (
    <div className="panel-card palette-view">
      <div className="palette-input">
        <Icon size="1.077rem" aria-hidden="true" />
        <input
          autoFocus
          aria-label={inputLabel}
          placeholder={placeholder}
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setSelected(0);
          }}
          onKeyDown={onKeyDown}
        />
      </div>
      <ul className="palette-list shortcut-list" role="listbox" aria-label={label}>
        {rows && shown.length === 0 && <li className="o3">{rows.length === 0 ? emptyText : noMatchText}</li>}
        {shown.map((row, index) => (
          <Fragment key={row.key}>
            {(index === 0 || shown[index - 1].group !== row.group) && (
              <li role="presentation" className="palette-section">
                {row.group}
              </li>
            )}
            <li
              ref={index === current ? activeRef : undefined}
              role="option"
              aria-selected={index === current}
              className={index === current ? "active" : undefined}
              {...row.data}
              // The second click of a double-click lands on the selected row and opens it.
              onClick={() => (index === current ? open(row) : setSelected(index))}
            >
              <span className="context-number o3">{index < SHORTCUT_ROWS ? `⌘${index + 1}` : ""}</span>
              <span className="grow-text">{row.title}</span>
            </li>
          </Fragment>
        ))}
      </ul>
      <div className="palette-foot o3">{error ? <span className="palette-error">{error}</span> : <span>{hints}</span>}</div>
    </div>
  );
}
