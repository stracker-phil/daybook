import { useEffect, useState } from "react";
import { FolderOpen, X } from "lucide-react";
import { api, toAppError, type LauncherProjects, type LinkedProject, type Project } from "../../api";
import { LinkAdder, linkChoices } from "./LinkAdder";

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
  const { linkable, ownerOf } = linkChoices(project, projects, "launcherProjects", known);

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
        <LinkAdder
          candidates={linkable}
          label="Add a Launcher project"
          noun="Launcher projects"
          placeholder="Add: type part of a project’s name, ↵ adds it"
          renderMatch={(entry) => {
            const owner = ownerOf(entry.id);
            return (
              <>
                <FolderOpen size="0.923rem" className="o2" aria-hidden="true" />
                <span className="grow-text">{entry.name}</span>
                {owner && <span className="o3 launcher-note">also in {owner}</span>}
              </>
            );
          }}
          onAdd={(entry) => onChange([...linked, { id: entry.id, name: entry.name }])}
        />
      )}
    </div>
  );
}
