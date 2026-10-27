import { Fragment, useEffect, useRef, useState, type KeyboardEvent, type Ref } from "react";
import { Command, FolderOpen, Layers, Link2, Music, type LucideIcon } from "lucide-react";
import { api, toAppError, type PaletteEntry, type PaletteKind, type PalettePrefix, type PaletteResults } from "../../api";
import { ShortcutCap } from "../../components/Field";
import { PanelFooter, PanelInput, PanelKey, PanelList, PanelOption, SectionHeader } from "./parts";

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
    <PanelOption
      ref={active ? activeRef : undefined}
      selected={active}
      icon={Icon}
      data-stage={`palette:${entry.command.toLowerCase().replace(/[^a-z0-9:]+/g, "-")}`}
      data-stage-label={entry.label}
      onMouseEnter={onHover}
      onClick={onRun}
    >
      <span className="grow-text">{entry.label}</span>
      {entry.detail && <span className="o3 palette-detail">{entry.detail}</span>}
      <ShortcutCap shortcut={entry.shortcut} />
    </PanelOption>
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
            <PanelKey>{candidate.char}</PanelKey>
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
    <div className="panel-card command-palette">
      <PanelInput
        icon={Command}
        stage="palette:search"
        stageLabel="Search commands"
        aria-label="Search"
        placeholder={prefix ? `Search ${prefix.label.toLowerCase()}…` : "Search commands, links, projects, music and contexts…"}
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        onKeyDown={onKeyDown}
      />
      <PanelList selection="tint" className="palette-results" aria-label="Results" data-stage-list="palette:results">
        {groups.map((group, g) => (
          <Fragment key={group.kind}>
            <SectionHeader>{group.heading}</SectionHeader>
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
        {results && entries.length === 0 && <li className="panel-empty">No results.</li>}
      </PanelList>
      <PanelFooter error={error}>{!error && <PrefixFooter prefixes={results?.prefixes ?? []} active={prefix} onPick={setQuery} />}</PanelFooter>
    </div>
  );
}
