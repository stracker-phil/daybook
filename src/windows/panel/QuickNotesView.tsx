import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { NotebookPen, Plus } from "lucide-react";
import { api, toAppError, type QuickNoteTopics, type TopicRow } from "../../api";
import { ESCAPE_HINT, PanelFooter, PanelInput, PanelList, PanelOption } from "./parts";

const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
/** How far each `#` past `###` indents a topic, in px. */
const INDENT = 14;

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
      <PanelInput
        icon={NotebookPen}
        stageLabel="Quick note topic"
        aria-label="Topic"
        placeholder={`Pick a topic under ${data?.notesHeading ?? "## Notes"} or type a new title…`}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={onKeyDown}
      />
      <PanelList className="quick-topics" aria-label="Topics">
        {rows.map((row, index) => (
          <PanelOption
            key={`${row.create ? "new" : "old"}:${row.topic.level}:${row.topic.title}`}
            ref={index === active ? activeRef : undefined}
            selected={index === active}
            icon={row.create ? Plus : undefined}
            iconSecondary
            onMouseMove={() => setActive(index)}
            onClick={() => open(row)}
          >
            {!row.create && (
              <span className="o3 quick-topic-level" style={{ paddingLeft: (row.topic.level - 3) * INDENT }}>
                {"#".repeat(row.topic.level)}
              </span>
            )}
            <span className="grow-text">{row.create ? `New topic “${row.topic.title}”` : row.topic.title}</span>
          </PanelOption>
        ))}
        {data && !data.hasTopics && rows.length === 0 && <li className="panel-empty">No topics today yet. Type a title to start one.</li>}
      </PanelList>
      <PanelFooter hints={[{ keys: ["↑", "↓"], label: "Topic" }, { keys: ["↵"], label: "Open" }, ESCAPE_HINT]} error={error} />
    </div>
  );
}
