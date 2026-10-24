import { Fragment, useEffect, useRef, useState, type KeyboardEvent, type Ref } from "react";
import { Command, FolderOpen, Layers, Link2, Music, type LucideIcon } from "lucide-react";
import { api, toAppError, type PaletteEntry, type PaletteKind, type PalettePrefix, type PaletteResults } from "../../api";
import { KeyCap } from "../../components/Field";
import { shortcutKeys } from "../../lib/shortcuts";

const ICONS: Record<PaletteKind, LucideIcon> = {
  command: Command,
  link: Link2,
  project: FolderOpen,
  music: Music,
  context: Layers,
};

const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };

interface RowProps {
  entry: PaletteEntry;
  icon: LucideIcon;
  active: boolean;
  activeRef: Ref<HTMLLIElement>;
  onHover: () => void;
  onRun: () => void;
}

function PaletteRow({ entry, icon: Icon, active, activeRef, onHover, onRun }: RowProps) {
  return (
    <li ref={active ? activeRef : undefined} role="option" aria-selected={active} className={active ? "active" : undefined} onMouseEnter={onHover} onClick={onRun}>
      <Icon size="1.077rem" className="o3" aria-hidden="true" />
      <span className="grow-text">{entry.label}</span>
      {entry.detail && <span className="o3 palette-detail">{entry.detail}</span>}
      <KeyCap keys={shortcutKeys(entry.shortcut)?.join("") ?? ""} />
    </li>
  );
}

/** One button per prefix; the active one clears it again. */
function PrefixFooter({ prefixes, active, onPick }: { prefixes: PalettePrefix[]; active: PalettePrefix | null; onPick: (text: string) => void }) {
  return (
    <>
      {prefixes.map((candidate) => {
        const on = active?.char === candidate.char;
        return (
          <button
            key={candidate.char}
            type="button"
            tabIndex={-1}
            className={on ? "palette-prefix on" : "palette-prefix"}
            onMouseDown={(e) => e.preventDefault()}
            onClick={() => onPick(on ? "" : candidate.char)}
          >
            <kbd className="keycap">{candidate.char}</kbd>
            {candidate.label}
          </button>
        );
      })}
    </>
  );
}

/** Everything Daybook can do, searchable; Rust lists and ranks it, a prefix narrows it to one kind. */
export function PaletteView() {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<PaletteResults | null>(null);
  const [active, setActive] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const activeRef = useRef<HTMLLIElement>(null);

  useEffect(() => {
    let current = true;
    api.paletteQuery(query).then(
      (next) => {
        if (!current) return;
        setResults(next);
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
  }, [active, results]);

  const groups = results?.groups ?? [];
  const entries = groups.flatMap((group) => group.entries);
  const starts = groups.map((_, i) => groups.slice(0, i).reduce((sum, group) => sum + group.entries.length, 0));
  const prefix = results?.prefix ?? null;

  const run = (entry: PaletteEntry | undefined) => {
    if (entry) api.paletteRun(entry.command).catch((e) => setError(toAppError(e).message));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    const step = STEPS[e.key];
    if (step && entries.length > 0) {
      e.preventDefault();
      setActive((i) => (i + step + entries.length) % entries.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      run(entries[active]);
    }
  };

  return (
    <div className="panel-card palette-view">
      <div className="palette-input">
        <Command size="1.077rem" aria-hidden="true" />
        <input
          autoFocus
          aria-label="Search"
          placeholder={prefix ? `Search ${prefix.label.toLowerCase()}…` : "Search commands, links, projects, music and contexts…"}
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          onKeyDown={onKeyDown}
        />
      </div>
      <ul className="palette-list palette-results" role="listbox" aria-label="Results">
        {groups.map((group, g) => (
          <Fragment key={group.kind}>
            <li role="presentation" className="palette-section">
              {group.heading}
            </li>
            {group.entries.map((entry, i) => {
              const index = starts[g] + i;
              return (
                <PaletteRow
                  key={entry.command}
                  entry={entry}
                  icon={ICONS[group.kind]}
                  active={index === active}
                  activeRef={activeRef}
                  onHover={() => setActive(index)}
                  onRun={() => run(entry)}
                />
              );
            })}
          </Fragment>
        ))}
        {results && entries.length === 0 && <li className="o3">No results.</li>}
      </ul>
      <div className="palette-foot o3">
        {error ? <span className="palette-error">{error}</span> : <PrefixFooter prefixes={results?.prefixes ?? []} active={prefix} onPick={setQuery} />}
      </div>
    </div>
  );
}
