import { useEffect, useRef, type ReactNode, type Ref } from "react";
import { Hash } from "lucide-react";
import type { SearchResult, TextPart } from "../../../api";

/** Rust marks the matched terms; this only renders them. */
function Parts({ parts }: { parts: TextPart[] }) {
  return parts.map((part, i) => (part.hit ? <mark key={i}>{part.text}</mark> : part.text));
}

function Row({ active, activeRef, onClick, children }: { active: boolean; activeRef: Ref<HTMLLIElement>; onClick: () => void; children: ReactNode }) {
  return (
    <li ref={active ? activeRef : undefined} role="option" aria-selected={active} className={active ? "active" : undefined} onMouseDown={(e) => e.preventDefault()} onClick={onClick}>
      {children}
    </li>
  );
}

interface ResultListProps {
  /** Scope names for the `@token` being typed; while there are any, they replace the results. */
  suggestions: string[];
  results: SearchResult[];
  /** The highlighted item: a result's path or a suggestion's name. */
  active: string;
  emptyText: string;
  onSuggestion: (name: string) => void;
  onResult: (result: SearchResult) => void;
}

/** Hovering never moves the highlight; a row's mouse-down keeps the focus in the field. */
export function ResultList({ suggestions, results, active, emptyText, onSuggestion, onResult }: ResultListProps) {
  const activeRef = useRef<HTMLLIElement>(null);
  useEffect(() => {
    // jsdom has no scrollIntoView.
    activeRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [active]);

  if (suggestions.length > 0) {
    return (
      <ul className="palette-list search-list" role="listbox" aria-label="Scopes">
        <li role="presentation" className="palette-section">
          Scopes
        </li>
        {suggestions.map((name) => (
          <Row key={name} active={name === active} activeRef={activeRef} onClick={() => onSuggestion(name)}>
            <Hash size="1.077rem" className="o3" aria-hidden="true" />
            <span className="grow-text">{name}</span>
          </Row>
        ))}
      </ul>
    );
  }

  return (
    <ul className="palette-list search-list" role="listbox" aria-label="Results">
      {results.length === 0 && <li className="o3 search-empty">{emptyText}</li>}
      {results.map((result) => (
        <Row key={result.path} active={result.path === active} activeRef={activeRef} onClick={() => onResult(result)}>
          <div className="search-result">
            <div className="search-result-title">
              <span className="search-result-name">
                <Parts parts={result.nameParts} />
              </span>
              <span className="search-result-folder o3">{result.folder}</span>
            </div>
            {result.lineParts.length > 0 && (
              <div className="search-result-line o2">
                <Parts parts={result.lineParts} />
              </div>
            )}
          </div>
        </Row>
      ))}
    </ul>
  );
}
