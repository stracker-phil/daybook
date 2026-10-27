import { useState, type KeyboardEvent, type ReactNode } from "react";
import type { LinkedProject, Project } from "../../api";

const MAX_MATCHES = 8;
const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };

interface Named {
  id: string;
  name: string;
}

/** Items whose `searchText` holds every typed word, names starting with the whole text first. */
function matches<T extends Named>(items: T[], query: string, searchText: (item: T) => string): T[] {
  const text = query.trim().toLowerCase();
  if (!text) return [];
  const words = text.split(/\s+/);
  const late = (item: T) => Number(!item.name.toLowerCase().startsWith(text));
  return items
    .filter((item) => {
      const haystack = searchText(item).toLowerCase();
      return words.every((word) => haystack.includes(word));
    })
    .sort((a, b) => late(a) - late(b) || a.name.localeCompare(b.name))
    .slice(0, MAX_MATCHES);
}

/** Which of a project's lists holds its links to one app. */
type LinkKey = "launcherProjects" | "timetrackerProjects";

/** For a project's links to an app: the app's projects still offered (active, not linked yet), and which other project links one. */
export function linkChoices<T extends Named & { archived: boolean }>(project: Project, projects: Project[], key: LinkKey, known: T[]) {
  const linked: LinkedProject[] = project[key];
  return {
    linkable: known.filter((entry) => !entry.archived && !linked.some((other) => other.id === entry.id)),
    ownerOf: (id: string) => projects.find((other) => other !== project && other[key].some((entry) => entry.id === id))?.name ?? null,
  };
}

interface LinkAdderProps<T extends Named> {
  candidates: T[];
  /** The field's accessible name; the matches are "Matching <noun>". */
  label: string;
  noun: string;
  placeholder: string;
  /** What a typed word is looked for in; the name by default. */
  searchText?: (item: T) => string;
  /** A match's content, inside its `.launcher-match` row. */
  renderMatch: (item: T) => ReactNode;
  onAdd: (item: T) => void;
}

/** Type part of a name, ↵ adds the highlighted match; the field keeps the focus for the next one. */
export function LinkAdder<T extends Named>({ candidates, label, noun, placeholder, searchText = (item) => item.name, renderMatch, onAdd }: LinkAdderProps<T>) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const found = matches(candidates, query, searchText);
  const current = Math.min(active, Math.max(0, found.length - 1));
  const add = (item: T | undefined) => {
    if (!item) return;
    onAdd(item);
    setQuery("");
    setActive(0);
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    const step = STEPS[e.key];
    if (step && found.length > 0) {
      e.preventDefault();
      setActive((current + step + found.length) % found.length);
    } else if (e.key === "Enter") {
      e.preventDefault();
      add(found[current]);
    } else if (e.key === "Escape" && query) {
      // Clears the text instead of closing the window.
      e.preventDefault();
      e.stopPropagation();
      setQuery("");
      setActive(0);
    }
  };

  return (
    <>
      <input
        className="input"
        aria-label={label}
        placeholder={placeholder}
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setActive(0);
        }}
        onKeyDown={onKeyDown}
      />
      {query.trim() && (
        <ul className="choice-matches" role="listbox" aria-label={`Matching ${noun}`}>
          {found.length === 0 && <li className="o3">No matching project.</li>}
          {found.map((item, i) => (
            <li
              key={item.id}
              role="option"
              aria-selected={i === current}
              className={i === current ? "launcher-match active" : "launcher-match"}
              onMouseDown={(e) => e.preventDefault()}
              onMouseEnter={() => setActive(i)}
              onClick={() => add(item)}
            >
              {renderMatch(item)}
            </li>
          ))}
        </ul>
      )}
    </>
  );
}
