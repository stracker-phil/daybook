import type { Project } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, Row, Switch, TextField } from "./controls";
import { ChoiceList, ItemActions, joinList, moveItem, parseList, replaceItem } from "./fields";
import { LauncherProjectsField, useLauncherProjects, type Launcher } from "./LauncherProjectsField";

function ProjectCard({ project, projects, contexts, launcher, index, count, onChange, onMove, onRemove }: {
  project: Project;
  projects: Project[];
  contexts: { id: string; title: string }[];
  launcher: Launcher;
  index: number;
  count: number;
  onChange: (patch: Partial<Project>) => void;
  onMove: (from: number, to: number) => void;
  onRemove: () => void;
}) {
  return (
    <Group title={project.active ? project.name : `${project.name} (inactive)`}>
      <TextField label="Name" value={project.name} onCommit={(name) => name.trim() && onChange({ name: name.trim() })} />
      <Switch label="Active" description="Off: no longer offered for new entries." checked={project.active} onChange={(active) => onChange({ active })} />
      {contexts.length > 1 && (
        <Row label="Contexts" description="Offered only while one of these is current. None: every context." stacked>
          <ChoiceList label="Contexts" choices={contexts} chosen={project.contexts} onChange={(ids) => onChange({ contexts: ids })} placeholder="Add: type part of a context's name, ↵ adds it" />
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
      <TextField label="Slack emoji" description="Starts the project's lines in the focus list, e.g. :oscorp:." placeholder=":emoji:" value={project.slackEmoji} onCommit={(slackEmoji) => onChange({ slackEmoji: slackEmoji.trim() })} />
      <Row label="">
        <ItemActions index={index} count={count} onMove={onMove} onRemove={onRemove} removeLabel="Remove Project" />
      </Row>
    </Group>
  );
}

const newProject = (): Project => ({ name: "New project", slackEmoji: "", active: true, contexts: [], keywords: [], launcherProjects: [] });

/** One card per project. Links bound to a project follow a rename and drop a removed one (Rust does that on save). */
export function ProjectsPage({ s, set }: SettingsPageProps) {
  const projects = s.projects;
  const contexts = s.contexts.map((context) => ({ id: context.id, title: [context.label.trim(), context.name].filter(Boolean).join(" ") }));
  const save = (next: Project[]) => set({ projects: next });
  const launcher = useLauncherProjects();
  return (
    <>
      {projects.map((project, i) => (
        <ProjectCard
          key={i}
          project={project}
          projects={projects}
          contexts={contexts}
          launcher={launcher}
          index={i}
          count={projects.length}
          onChange={(patch) => save(replaceItem(projects, i, patch))}
          onMove={(from, to) => save(moveItem(projects, from, to))}
          onRemove={() => save(projects.filter((_, j) => j !== i))}
        />
      ))}
      <div>
        <button type="button" className="btn" onClick={() => save([...projects, newProject()])}>
          Add Project
        </button>
      </div>
    </>
  );
}
