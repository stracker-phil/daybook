import { useState, type KeyboardEvent } from "react";
import { History, Ticket } from "lucide-react";
import { api, toAppError } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { fuzzyFilter } from "./fuzzy";
import { cycle, useActiveScroll } from "./listKeys";
import { ESCAPE_HINT, PanelFooter, PanelInput, PanelList, PanelOption, PanelPrompt, SectionHeader, type PanelHint } from "./parts";

const MAX_SUGGESTIONS = 8;
const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
const HINTS: PanelHint[] = [{ keys: ["↑", "↓"], label: "Suggestions" }, { keys: ["↵"], label: "Open" }, ESCAPE_HINT];

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
    <div className="panel-card">
      <PanelPrompt>Open Jira issue</PanelPrompt>
      <PanelInput
        icon={Ticket}
        stageLabel="Open Jira issue"
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
      {suggestions && suggestions.length > 0 && (
        <PanelList selection="tint" className="jira-suggestions" aria-label="Last 7 days">
          {visible.length === 0 ? (
            <li role="presentation" className="panel-empty">
              Press ↵ to open it.
            </li>
          ) : (
            <SectionHeader>Last 7 Days</SectionHeader>
          )}
          {visible.map((key, index) => (
            <PanelOption key={key} ref={index === current ? activeRef : undefined} selected={index === current} icon={History} onClick={() => open(key)}>
              <span className="grow-text">{key}</span>
            </PanelOption>
          ))}
        </PanelList>
      )}
      <PanelFooter hints={HINTS} error={error} />
    </div>
  );
}
