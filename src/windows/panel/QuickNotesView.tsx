import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { NotebookPen, Plus } from "lucide-react";
import { api, toAppError, type QuickNoteTopics, type TopicRow } from "../../api";

const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
/** How far each `#` past `###` indents a topic. */
const INDENT_REM = 0.923;

/** The quick-notes picker: today's topics and a new one for the typed title; ↵ opens it in the editor. */
export function QuickNotesView() {
  const [query, setQuery] = useState("");
  const [data, setData] = useState<QuickNoteTopics | null>(null);
  const [active, setActive] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const activeRef = useRef<HTMLLIElement>(null);

  useEffect(() => {
    let current = true;
    api.quickNotesList(query).then(
      (next) => {
        if (!current) return;
        setData(next);
        setActive(0);
      },
      (e) => {
        if (current) setError(toAppError(e).message);
      },
    );
    return () => {
      current = false;
    };
  }, [query]);

  useEffect(() => {
    // jsdom has no scrollIntoView.
    activeRef.current?.scrollIntoView?.({ block: "nearest" });
  }, [active]);

  const rows = data?.rows ?? [];
  const open = (row: TopicRow | undefined) => {
    if (row) api.quickNotesOpen(row.topic, row.create).catch((e) => setError(toAppError(e).message));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    const step = STEPS[e.key];
    if (step && rows.length > 0) {
      e.preventDefault();
      setActive((i) => (i + step + rows.length) % rows.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      open(rows[active]);
    }
  };

  return (
    <div className="panel-card palette-view">
      <div className="palette-input">
        <NotebookPen size="1.077rem" aria-hidden="true" />
        <input
          autoFocus
          aria-label="Topic"
          placeholder={`Pick a topic under ${data?.notesHeading ?? "## Notes"} or type a new title…`}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
        />
      </div>
      <ul className="palette-list quick-topics" role="listbox" aria-label="Topics">
        {rows.map((row, index) => (
          <li
            key={`${row.create ? "new" : "old"}:${row.topic.level}:${row.topic.title}`}
            ref={index === active ? activeRef : undefined}
            role="option"
            aria-selected={index === active}
            className={index === active ? "active" : undefined}
            onMouseEnter={() => setActive(index)}
            onClick={() => open(row)}
          >
            {row.create ? (
              <Plus size="1.077rem" className="o3" aria-hidden="true" />
            ) : (
              <span className="o3 mono quick-topic-level" style={{ paddingLeft: `${(row.topic.level - 3) * INDENT_REM}rem` }}>
                {"#".repeat(row.topic.level)}
              </span>
            )}
            <span className="grow-text">{row.create ? `New topic “${row.topic.title}”` : row.topic.title}</span>
          </li>
        ))}
        {data && !data.hasTopics && rows.length === 0 && <li className="o3">No topics today yet. Type a title to start one.</li>}
      </ul>
      <div className="palette-foot o3">{error ? <span className="palette-error">{error}</span> : <span>↵ open · esc close</span>}</div>
    </div>
  );
}
