import type { ReactNode } from "react";
import type { Project } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { AddButton, EntryFooter, Group, Row, Switch, TextField } from "./controls";
import { ToggleChips, contextChoices, joinList, parseList, type Choice } from "./fields";
import { listEditor } from "./listEditor";
import { LauncherProjectsField, useLauncherProjects, type Launcher } from "./LauncherProjectsField";
import { TimeTrackerProjectsField } from "./TimeTrackerProjectsField";
import { useTimetrackerProjects, type TimeTrackerProjectsResult } from "./useTimeTracker";

function ProjectCard({ project, projects, contexts, launcher, timetracker, actions, onChange }: {
  project: Project;
  projects: Project[];
  contexts: Choice[];
  launcher: Launcher;
  timetracker: TimeTrackerProjectsResult;
  actions: ReactNode;
  onChange: (patch: Partial<Project>) => void;
}) {
  return (
    <Group title={project.active ? project.name : `${project.name} (inactive)`}>
      <TextField label="Name" value={project.name} onCommit={(name) => name.trim() && onChange({ name: name.trim() })} />
      <Switch label="Active" description="Off: no longer offered for new entries." checked={project.active} onChange={(active) => onChange({ active })} />
      {contexts.length > 1 && (
        <Row label="Contexts" description="Offered only while one of these is current. None selected: every context.">
          <ToggleChips label="Contexts" choices={contexts} chosen={project.contexts} onChange={(ids) => onChange({ contexts: ids })} />
        </Row>
      )}
      <TextField
        label="Meeting words"
        description="A call goes to the first project whose name or one of these words is in its title."
        placeholder="e.g. QRS, Initech"
        value={joinList(project.keywords)}
        onCommit={(text) => onChange({ keywords: parseList(text) })}
      />
      <Row label="Launcher projects" description="Opened from the Projects tool (Daybook window, menu bar, ⌘K) in Project Launcher." stacked>
        <LauncherProjectsField project={project} projects={projects} launcher={launcher} onChange={(launcherProjects) => onChange({ launcherProjects })} />
      </Row>
      <Row label="TimeTracker projects" description="A TimeTracker timer on one of these shows this project." stacked>
        <TimeTrackerProjectsField project={project} projects={projects} timetracker={timetracker} onChange={(timetrackerProjects) => onChange({ timetrackerProjects })} />
      </Row>
      <TextField label="Slack emoji" description="Starts the project's lines in the focus list, e.g. :oscorp:." placeholder=":emoji:" value={project.slackEmoji} onCommit={(slackEmoji) => onChange({ slackEmoji: slackEmoji.trim() })} />
      <EntryFooter>{actions}</EntryFooter>
    </Group>
  );
}

const newProject = (): Project => ({ name: "New project", slackEmoji: "", active: true, contexts: [], keywords: [], launcherProjects: [], timetrackerProjects: [] });

/** One card per project. Links bound to a project follow a rename and drop a removed one (Rust does that on save). */
export function ProjectsPage({ s, set }: SettingsPageProps) {
  const projects = s.projects;
  const contexts = contextChoices(s.contexts);
  const edit = listEditor(projects, (next) => set({ projects: next }));
  const launcher = useLauncherProjects();
  const timetracker = useTimetrackerProjects();
  return (
    <>
      {projects.map((project, i) => (
        <ProjectCard
          key={i}
          project={project}
          projects={projects}
          contexts={contexts}
          launcher={launcher}
          timetracker={timetracker}
          actions={edit.actions(i, "Remove Project")}
          onChange={(patch) => edit.change(i, patch)}
        />
      ))}
      <AddButton label="Add Project" onClick={() => edit.add(newProject())} />
    </>
  );
}
