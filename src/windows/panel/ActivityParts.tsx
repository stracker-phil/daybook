import { Fragment, type MouseEvent, type Ref } from "react";
import { History } from "lucide-react";
import type { ActivityProject, ActivitySuggestion, ActivityTtProject, SuggestionGroup } from "../../api";

/** ⌘1…⌘9 pick a TimeTracker project by position. */
export const MAX_PROJECT_KEYS = 9;

export const GROUPS: { group: SuggestionGroup; heading: string }[] = [
  { group: "meetings", heading: "Meetings" },
  { group: "ticketsToday", heading: "Tickets Today" },
  { group: "recentTimers", heading: "Recent Timers" },
];

/** The TimeTracker projects in picker order: Tab and ⌘1–9 go through these. */
export const pickable = (projects: ActivityProject[]): ActivityTtProject[] => projects.flatMap((project) => project.ttProjects);

// Keeps the keyboard in the note field.
export const keepFocus = (e: MouseEvent) => e.preventDefault();

/** A group's own name says nothing new when its only TimeTracker project has the same name. */
const namesItself = (project: ActivityProject) => project.ttProjects.length === 1 && project.ttProjects[0].name === project.name;

/** TimeTracker's projects grouped under the Daybook projects; an unlinked one shows its hint on click. */
export function ProjectPicker({ projects, picked, onPick, onHint }: {
  projects: ActivityProject[];
  picked: string;
  onPick: (ttProjectId: string) => void;
  onHint: (hint: string) => void;
}) {
  if (projects.length === 0) return null;
  return (
    <div className="activity-projects" data-stage="panel:projects" data-stage-label="Project picker (⌘1…⌘9)">
      {projects.map((project) => (
        <div key={project.name} className="activity-project" role="group" aria-label={project.name}>
          {!namesItself(project) && <span className="o3">{project.name}</span>}
          {project.ttProjects.length > 0 ? (
            <div className="segmented">
              {project.ttProjects.map((tt) => (
                <button
                  key={tt.id}
                  type="button"
                  aria-pressed={tt.id === picked}
                  onMouseDown={keepFocus}
                  onClick={() => onPick(tt.id)}
                >
                  {tt.name}
                </button>
              ))}
            </div>
          ) : (
            <button type="button" className="activity-unlinked" title={project.hint ?? undefined} onMouseDown={keepFocus} onClick={() => project.hint && onHint(project.hint)}>
              Not linked
            </button>
          )}
        </div>
      ))}
    </div>
  );
}

/** "⌘1 QRS ⌘2 Zeno …" under the suggestions. */
export function ProjectKeys({ projects }: { projects: ActivityTtProject[] }) {
  const shown = projects.slice(0, MAX_PROJECT_KEYS);
  if (shown.length === 0) return null;
  return (
    <div className="project-keys o3" aria-label="Project keys">
      {shown.map((tt, index) => (
        <span key={tt.id}>
          ⌘{index + 1} {tt.name}
        </span>
      ))}
    </div>
  );
}

/** Suggestions under their group's heading, in `items` order; a click starts one right away. */
export function SuggestionList({ items, highlighted, activeRef, onPick }: {
  items: ActivitySuggestion[];
  highlighted: number;
  activeRef: Ref<HTMLLIElement>;
  onPick: (suggestion: ActivitySuggestion) => void;
}) {
  return (
    <ul className="palette-list activity-suggestions" role="listbox" aria-label="Suggestions">
      {items.length === 0 && <li className="o3">Press ↵ to start it.</li>}
      {items.map((suggestion, index) => (
        <Fragment key={`${suggestion.group}\u0000${suggestion.display}`}>
          {(index === 0 || items[index - 1].group !== suggestion.group) && (
            <li role="presentation" className="palette-section">
              {GROUPS.find((group) => group.group === suggestion.group)?.heading}
            </li>
          )}
          <li
            ref={index === highlighted ? activeRef : undefined}
            role="option"
            aria-selected={index === highlighted}
            className={index === highlighted ? "active activity-suggestion" : "activity-suggestion"}
            onMouseDown={keepFocus}
            onClick={() => onPick(suggestion)}
          >
            <History size="1.077rem" className="o3" aria-hidden="true" />
            <span className="grow-text">{suggestion.display}</span>
          </li>
        </Fragment>
      ))}
    </ul>
  );
}
