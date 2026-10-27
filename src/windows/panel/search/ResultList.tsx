import { useEffect, useRef, type ReactNode, type Ref } from "react";
import { Hash } from "lucide-react";
import type { SearchResult, TextPart } from "../../../api";
import { PanelList, PanelOption, SectionHeader } from "../parts";

/** Rust marks the matched terms; this only renders them. */
function Parts({ parts }: { parts: TextPart[] }) {
  return parts.map((part, i) => (part.hit ? <mark key={i}>{part.text}</mark> : part.text));
}

function Row({ active, activeRef, onClick, children }: { active: boolean; activeRef: Ref<HTMLLIElement>; onClick: () => void; children: ReactNode }) {
  return (
    <PanelOption ref={active ? activeRef : undefined} selected={active} onClick={onClick}>
      {children}
    </PanelOption>
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
      <PanelList selection="tint" className="search-list" aria-label="Scopes">
        <SectionHeader>Scopes</SectionHeader>
        {suggestions.map((name) => (
          <Row key={name} active={name === active} activeRef={activeRef} onClick={() => onSuggestion(name)}>
            <Hash size={16} className="o3" aria-hidden="true" />
            <span className="grow-text">{name}</span>
          </Row>
        ))}
      </PanelList>
    );
  }

  return (
    <PanelList selection="tint" className="search-list" aria-label="Results">
      {results.length === 0 && (
        <li role="presentation" className="search-empty">
          {emptyText}
        </li>
      )}
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
    </PanelList>
  );
}
