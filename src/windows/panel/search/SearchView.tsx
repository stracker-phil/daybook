import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Search } from "lucide-react";
import { api, type SearchResult } from "../../../api";
import { useApiResult } from "../../../lib/useApiResult";
import { NotePreview } from "./NotePreview";
import { ResultList } from "./ResultList";
import { usePreviewHistory } from "./usePreviewHistory";
import { useResultActions } from "./useResultActions";
import { useScopeChips } from "./useScopeChips";

/** While the index builds, the results are asked for again this often. */
const INDEXING_POLL_MS = 700;

const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
const CTRL_STEPS: Record<string, number> = { n: 1, j: 1, p: -1, k: -1 };

/** How far a key moves the highlight in a list of `count`; null for any other key. */
function stepFor(e: KeyboardEvent, index: number, count: number): number | null {
  if (e.metaKey || e.altKey) return null;
  if (e.ctrlKey) return CTRL_STEPS[e.key] ?? null;
  if (e.key === "Home") return -index;
  if (e.key === "End") return count - 1 - index;
  return STEPS[e.key] ?? null;
}

/** ⌘C copies the selected text instead of the link when there is any. */
function hasTextSelection(target: EventTarget): boolean {
  if (target instanceof HTMLInputElement) return target.selectionStart !== target.selectionEnd;
  return !!window.getSelection()?.toString();
}

function hintsFor(canBack: boolean, canForward: boolean): string {
  const history = [canBack && "⌘[ back", canForward && "⌘] forward"].filter(Boolean);
  return ["↵ open", "⌘↵ open preview", "⌘C copy link", ...history, "esc close"].join(" · ");
}

/** Results for the text and chips; the last ones stay while the next load, and they refresh while indexing. */
function useSearch(text: string, scopes: string[]) {
  const search = useApiResult(JSON.stringify([text, scopes]), () => api.searchQuery(text, scopes), { keepData: true });
  const { data, reload } = search;
  useEffect(() => {
    if (!data?.indexing) return;
    const timer = setTimeout(reload, INDEXING_POLL_MS);
    return () => clearTimeout(timer);
  }, [data, reload]);
  return search;
}

/** Chips are reachable with Tab; ⌫ or Delete removes the focused one. */
function ScopeChips({ scopes, onRemove }: { scopes: string[]; onRemove: (scope: string) => void }) {
  return scopes.map((scope) => (
    <button
      key={scope}
      type="button"
      className="scope-chip"
      aria-label={`Scope ${scope}. Press Delete to remove it.`}
      onKeyDown={(e) => {
        if (e.key !== "Backspace" && e.key !== "Delete") return;
        e.preventDefault();
        e.stopPropagation();
        onRemove(scope);
      }}
    >
      @{scope}
    </button>
  ));
}

/**
 * Vault search: all words must appear, "phrases" in quotes, `@scope` narrows to folders. The pointer never moves
 * the highlight: a click highlights a result, a click on the highlighted one opens it.
 */
export function SearchView({ scope }: { scope: string | null }) {
  const [text, setText] = useState("");
  const [highlighted, setHighlighted] = useState("");
  const input = useRef<HTMLInputElement>(null);
  const chips = useScopeChips(scope, text, setText);
  const { status, setStatus, open, copyLink } = useResultActions();
  const search = useSearch(text, chips.scopes);

  const response = search.data;
  const results = response?.results ?? [];
  const selected: SearchResult | undefined = results.find((result) => result.path.toLowerCase() === highlighted.toLowerCase()) ?? results[0];
  const preview = usePreviewHistory(selected?.path ?? null);
  const suggesting = chips.suggestions.length > 0;
  const items = suggesting ? chips.suggestions : results.map((result) => result.path);
  const active = suggesting ? (items.find((item) => item === highlighted) ?? items[0]) : (selected?.path ?? "");

  const changeText = (value: string) => {
    setStatus(null);
    if (!chips.consumeTypedScope(value)) setText(value);
  };

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.defaultPrevented) return;
    const index = Math.max(0, items.indexOf(active));
    const step = stepFor(e, index, items.length);
    if (step !== null && items.length > 0) {
      e.preventDefault();
      setHighlighted(items[(index + step + items.length) % items.length]);
    } else if (e.key === "Enter" && !e.metaKey) {
      e.preventDefault();
      if (suggesting) chips.addChip(active);
      else if (selected) open(selected.path);
    } else if (e.key === "Enter" && e.metaKey) {
      e.preventDefault();
      if (preview.shown) open(preview.shown);
    } else if (e.metaKey && e.key.toLowerCase() === "c" && preview.shown && !hasTextSelection(e.target)) {
      e.preventDefault();
      copyLink(preview.shown);
    } else if (e.metaKey && e.key === "[") {
      e.preventDefault();
      preview.back();
    } else if (e.metaKey && e.key === "]") {
      e.preventDefault();
      preview.forward();
    }
  };

  const clickResult = (result: SearchResult) => {
    if (result.path === selected?.path) open(result.path);
    else setHighlighted(result.path);
  };

  const indexing = response?.indexing ? `Indexing… ${response.notes} notes so far` : "";
  return (
    // The root takes the focus of a click into the preview, so the keys keep working.
    <div className="panel-card search-view" tabIndex={-1} onKeyDown={onKeyDown}>
      <div className="palette-input search-field">
        <Search size="1.385rem" aria-hidden="true" />
        <ScopeChips
          scopes={chips.scopes}
          onRemove={(name) => {
            chips.remove(name);
            input.current?.focus();
          }}
        />
        <input
          ref={input}
          autoFocus
          aria-label="Search"
          placeholder={chips.scopes.length > 0 ? "Search notes…" : "Search the whole vault… (@scope to narrow)"}
          value={text}
          onChange={(e) => changeText(e.target.value)}
          onKeyDown={(e) => chips.onFieldKeyDown(e, suggesting ? active : undefined)}
        />
      </div>
      <div className="search-split">
        <ResultList
          suggestions={chips.suggestions}
          results={results}
          active={active}
          emptyText={text.trim() ? "No notes found." : "Type to search."}
          onSuggestion={chips.addChip}
          onResult={clickResult}
        />
        {preview.shown && selected ? (
          <NotePreview path={preview.shown} onNavigate={preview.navigate} onError={setStatus} />
        ) : (
          <p className="search-placeholder o3">Results show up here as you type.</p>
        )}
      </div>
      <div className="palette-foot o3 search-foot">
        <span className="search-status">{status ?? (search.error?.message || indexing)}</span>
        <span>{hintsFor(preview.canBack, preview.canForward)}</span>
      </div>
    </div>
  );
}
