import { useState, type KeyboardEvent } from "react";
import { History, Ticket } from "lucide-react";
import { api, toAppError } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { fuzzyFilter } from "./fuzzy";
import { cycle, useActiveScroll } from "./listKeys";

const MAX_SUGGESTIONS = 8;
const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };

/**
 * Opens a Jira issue by number, key or link. Like an address bar: ↑↓ put a recent key into the field, -1 being
 * your own text; ↵ opens what the field shows. Rust closes the overlay once the issue opens.
 */
export function JiraView() {
  const suggestions = useApiResult("jira", api.jiraSuggestions).data;
  const [typed, setTyped] = useState("");
  const [highlighted, setHighlighted] = useState(-1);
  const [error, setError] = useState<string | null>(null);
  const visible = fuzzyFilter(suggestions ?? [], typed).slice(0, MAX_SUGGESTIONS);
  const current = highlighted < visible.length ? highlighted : -1;
  const value = current >= 0 ? visible[current] : typed;
  const activeRef = useActiveScroll<HTMLLIElement>(current);

  const open = (input: string) => {
    if (input.trim()) api.jiraOpen(input).catch((e) => setError(toAppError(e).message));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) return;
    const step = STEPS[e.key];
    if (step) {
      e.preventDefault();
      // The typed text sits before the suggestions as position -1, so the cycle runs over one more place.
      if (visible.length > 0) setHighlighted(cycle(current + 1, step, visible.length + 1) - 1);
    } else if (e.key === "Enter") {
      e.preventDefault();
      open(value);
    }
  };

  return (
    <div className="panel-card palette-view">
      <div className="palette-input" data-stage="panel:input" data-stage-label="Open Jira issue" data-stage-role="field">
        <Ticket size="1.077rem" aria-hidden="true" />
        <span className="jira-prompt">Open Jira issue</span>
        <input
          autoFocus
          aria-label="Open Jira issue"
          placeholder="6805, QRS-6805 or a Jira link"
          value={value}
          onChange={(e) => {
            setTyped(e.target.value);
            setHighlighted(-1);
            setError(null);
          }}
          onKeyDown={onKeyDown}
        />
      </div>
      {suggestions && suggestions.length > 0 && (
        <ul className="palette-list jira-suggestions" role="listbox" aria-label="Last 7 days">
          <li role="presentation" className={visible.length === 0 ? "o3" : "palette-section"}>
            {visible.length === 0 ? "Press ↵ to open it." : "Last 7 Days"}
          </li>
          {visible.map((key, index) => (
            <li
              key={key}
              ref={index === current ? activeRef : undefined}
              role="option"
              aria-selected={index === current}
              className={index === current ? "active jira-suggestion" : "jira-suggestion"}
              // Keeps the focus in the field.
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => open(key)}
            >
              <History size="1.077rem" className="o3" aria-hidden="true" />
              <span className="grow-text">{key}</span>
            </li>
          ))}
        </ul>
      )}
      <div className="palette-foot o3 jira-foot">
        {error && <span className="palette-error">{error}</span>}
        <span>↑↓ suggestions · ↵ open · esc close</span>
      </div>
    </div>
  );
}
