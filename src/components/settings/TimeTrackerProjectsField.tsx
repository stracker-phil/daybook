import { X } from "lucide-react";
import type { LinkedProject, Project, TimeTrackerProjects, TtProject } from "../../api";
import { AccentDot } from "../Accent";
import { LinkAdder, linkChoices } from "./LinkAdder";
import { openTimeTracker, type TimeTrackerProjectsResult } from "./useTimeTracker";

/** Why nothing can be picked, and whether opening TimeTracker would help; `null` when it can. */
function problemOf(list: TimeTrackerProjects | null, error: string | null): { text: string; canOpen: boolean } | null {
  if (error) return { text: `TimeTracker didn't answer: ${error}`, canOpen: false };
  if (list?.state === "notRunning") return { text: "TimeTracker isn't running: start it to pick its projects.", canOpen: true };
  if (list?.state === "missing") return { text: "TimeTracker isn't installed: check its command in Settings → TimeTracker.", canOpen: false };
  return null;
}

function ProjectLabel({ name, client }: { name: string; client: string | null }) {
  return (
    <>
      <span className="grow-text">{name}</span>
      {client && <span className="o3 launcher-note">{client}</span>}
    </>
  );
}

interface TimeTrackerProjectsFieldProps {
  project: Project;
  projects: Project[];
  timetracker: TimeTrackerProjectsResult;
  onChange: (linked: LinkedProject[]) => void;
}

/** The project's linked TimeTracker projects, each unlinked with one click, and a field to link more (active ones only). */
export function TimeTrackerProjectsField({ project, projects, timetracker, onChange }: TimeTrackerProjectsFieldProps) {
  const { data: list, error, state, retry } = timetracker;
  const known = list?.projects ?? [];
  const linked = project.timetrackerProjects;
  const { linkable, ownerOf } = linkChoices(project, projects, "timetrackerProjects", known);
  const listedNow = list?.state === "available" && !error;
  const problem = problemOf(list, error?.message ?? null);

  return (
    <div className="choice-list">
      {linked.length > 0 && (
        <ul aria-label="TimeTracker projects">
          {linked.map((entry) => {
            const found = known.find((other) => other.id === entry.id);
            const gone = listedNow && !found;
            return (
              <li key={entry.id}>
                {found && <AccentDot accent={found.color} shade={found.shade} />}
                <ProjectLabel name={found?.name ?? entry.name} client={found?.client ?? null} />
                {found?.archived && <span className="o3 launcher-note">(archived)</span>}
                {gone && <span className="o3 launcher-note">Not in TimeTracker anymore</span>}
                <button type="button" className="icon-btn tiny" aria-label={`Unlink ${entry.name}`} onClick={() => onChange(linked.filter((other) => other.id !== entry.id))}>
                  <X size="0.846rem" aria-hidden="true" />
                </button>
              </li>
            );
          })}
        </ul>
      )}
      {problem ? (
        <div className="launcher-problem">
          <span className="o2">{problem.text}</span>
          {problem.canOpen ? (
            <button type="button" className="btn" onClick={openTimeTracker}>
              Open TimeTracker
            </button>
          ) : (
            <button type="button" className="btn" disabled={state === "loading"} onClick={retry}>
              Try Again
            </button>
          )}
        </div>
      ) : !list ? (
        <span className="o3">Asking TimeTracker…</span>
      ) : (
        <LinkAdder<TtProject>
          candidates={linkable}
          label="Add a TimeTracker project"
          noun="TimeTracker projects"
          placeholder="Type to link a TimeTracker project or client"
          searchText={(entry) => `${entry.name} ${entry.client ?? ""}`}
          renderMatch={(entry) => {
            const owner = ownerOf(entry.id);
            return (
              <>
                <AccentDot accent={entry.color} shade={entry.shade} />
                <ProjectLabel name={entry.name} client={entry.client} />
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
