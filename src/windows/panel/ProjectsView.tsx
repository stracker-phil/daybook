import { FolderOpen } from "lucide-react";
import { api, type ProjectGroup } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { ShortcutListView, type CommandRow } from "./ShortcutListView";

/** Each project's Launcher projects under its name; the name matches too. */
const rowsOf = (groups: ProjectGroup[]): CommandRow[] =>
  groups.flatMap((group) =>
    group.projects.map((project) => ({
      key: `${group.project}:${project.id}`,
      title: project.name,
      group: group.project,
      detail: group.project,
      command: project.command,
      data: { "data-stage": `project:${project.id}`, "data-stage-label": `${project.name} (${group.project})`, "data-stage-list": "projects" },
    })),
  );

/** The current projects' Project Launcher projects; opening one shows it in Project Launcher. */
export function ProjectsView() {
  const view = useApiResult("projects", api.projectsView);
  return (
    <ShortcutListView
      icon={FolderOpen}
      inputLabel="Project"
      placeholder="Show a project in Project Launcher…"
      label="Projects"
      rows={view.data ? rowsOf(view.data) : null}
      emptyText="No Launcher projects yet. Link them in Settings → Projects."
      noMatchText="No matching projects."
      hints="↑↓ project · ↵ show · ⌘1–9 show · esc close"
      loadError={view.error?.message ?? null}
    />
  );
}
