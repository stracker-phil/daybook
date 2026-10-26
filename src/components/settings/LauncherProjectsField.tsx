import { useEffect, useState, type KeyboardEvent } from "react";
import { FolderOpen, X } from "lucide-react";
import { api, toAppError, type LauncherProjects, type LinkedProject, type LauncherProject, type Project } from "../../api";

const failed = (e: unknown): LauncherProjects => ({ found: true, projects: [], error: toAppError(e).message });

/** Project Launcher's list, asked for once per page; a failed call reads as Project Launcher not answering. */
export function useLauncherProjects() {
  const [list, setList] = useState<LauncherProjects | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    let alive = true;
    api
      .launcherProjects(false)
      .then((next) => alive && setList(next), (e: unknown) => alive && setList(failed(e)))
      .finally(() => alive && setLoading(false));
    return () => {
      alive = false;
    };
  }, []);

  const reload = () => {
    setLoading(true);
    api
      .launcherProjects(true)
      .then(setList, (e: unknown) => setList(failed(e)))
      .finally(() => setLoading(false));
  };
  return { list, loading, reload };
}

export type Launcher = ReturnType<typeof useLauncherProjects>;

const MAX_MATCHES = 8;
const STEPS: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };

/** Names containing every typed word, names starting with the whole text first. */
function matches(projects: LauncherProject[], query: string): LauncherProject[] {
  const text = query.trim().toLowerCase();
  if (!text) return [];
  const words = text.split(/\s+/);
  const late = (project: LauncherProject) => Number(!project.name.toLowerCase().startsWith(text));
  return projects
    .filter((project) => words.every((word) => project.name.toLowerCase().includes(word)))
    .sort((a, b) => late(a) - late(b) || a.name.localeCompare(b.name))
    .slice(0, MAX_MATCHES);
}

/** Type part of a name, ↵ adds the highlighted match; the field keeps the focus for the next one. */
function ProjectAdder({ candidates, ownerOf, onAdd }: { candidates: LauncherProject[]; ownerOf: (id: string) => string | null; onAdd: (project: LauncherProject) => void }) {
  const [query, setQuery] = useState("");
  const [active, setActive] = useState(0);
  const found = matches(candidates, query);
  const current = Math.min(active, Math.max(0, found.length - 1));
  const add = (project: LauncherProject | undefined) => {
    if (!project) return;
    onAdd(project);
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
        aria-label="Add a Launcher project"
        placeholder="Add: type part of a project’s name, ↵ adds it"
        value={query}
        onChange={(e) => {
          setQuery(e.target.value);
          setActive(0);
        }}
        onKeyDown={onKeyDown}
      />
      {query.trim() && (
        <ul className="choice-matches" role="listbox" aria-label="Matching Launcher projects">
          {found.length === 0 && <li className="o3">No matching project.</li>}
          {found.map((project, i) => {
            const owner = ownerOf(project.id);
            return (
              <li
                key={project.id}
                role="option"
                aria-selected={i === current}
                className={i === current ? "launcher-match active" : "launcher-match"}
                onMouseDown={(e) => e.preventDefault()}
                onMouseEnter={() => setActive(i)}
                onClick={() => add(project)}
              >
                <FolderOpen size="0.923rem" className="o2" aria-hidden="true" />
                <span className="grow-text">{project.name}</span>
                {owner && <span className="o3 launcher-note">also in {owner}</span>}
              </li>
            );
          })}
        </ul>
      )}
    </>
  );
}

interface LauncherProjectsFieldProps {
  project: Project;
  projects: Project[];
  launcher: Launcher;
  onChange: (linked: LinkedProject[]) => void;
}

/** The project's linked Launcher projects, each unlinked with one click, and a field to link more. */
export function LauncherProjectsField({ project, projects, launcher, onChange }: LauncherProjectsFieldProps) {
  const { list, loading, reload } = launcher;
  const linked = project.launcherProjects;
  const known = list?.projects ?? [];
  const listed = (id: string) => known.some((entry) => entry.id === id);
  const linkable = known.filter((entry) => !entry.archived && !linked.some((other) => other.id === entry.id));
  const ownerOf = (id: string) => projects.find((other) => other !== project && other.launcherProjects.some((entry) => entry.id === id))?.name ?? null;

  let problem: string | null = null;
  if (list && !list.found) problem = "Install Project Launcher’s command line tool first: Project Launcher → Settings → Command Line.";
  else if (list?.error) problem = `Project Launcher didn’t answer: ${list.error}`;

  return (
    <div className="choice-list">
      {linked.length > 0 && (
        <ul aria-label="Launcher projects">
          {linked.map((entry) => (
            <li key={entry.id}>
              <FolderOpen size="0.923rem" className="o2" aria-hidden="true" />
              <span className="grow-text">{entry.name}</span>
              {list && !list.error && !listed(entry.id) && <span className="o3 launcher-note">Not in Project Launcher anymore</span>}
              <button type="button" className="icon-btn tiny" aria-label={`Unlink ${entry.name}`} onClick={() => onChange(linked.filter((other) => other.id !== entry.id))}>
                <X size="0.846rem" aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
      {problem ? (
        <div className="launcher-problem">
          <span className="o2">{problem}</span>
          <button type="button" className="btn" disabled={loading} onClick={reload}>
            Try Again
          </button>
        </div>
      ) : loading && !list ? (
        <span className="o3">Asking Project Launcher…</span>
      ) : (
        <ProjectAdder
          candidates={linkable}
          ownerOf={ownerOf}
          onAdd={(entry) => onChange([...linked, { id: entry.id, name: entry.name }])}
        />
      )}
    </div>
  );
}
