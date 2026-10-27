import { forwardRef, type MouseEvent, type Ref } from "react";
import { Activity, History, Timer, type LucideIcon, type LucideProps } from "lucide-react";
import type { ActivityProject, ActivitySuggestion, SuggestionGroup } from "../../api";
import { PanelList, PanelOption, PanelSegmented, SectionHeader } from "./parts";

// TimeTracker ids are never empty.
const NO_TIMER = "";

export const GROUPS: { group: SuggestionGroup; heading: string }[] = [
  { group: "meetings", heading: "Meetings" },
  { group: "ticketsToday", heading: "Tickets Today" },
  { group: "recentTimers", heading: "Recent Timers" },
];

// Keeps the keyboard in the note field.
export const keepFocus = (e: MouseEvent) => e.preventDefault();

/** An icon whose tooltip says what its row is. */
function titled(Icon: LucideIcon, title: string): LucideIcon {
  const Titled = forwardRef<SVGSVGElement, Omit<LucideProps, "ref">>((props, ref) => (
    <Icon ref={ref} {...props}>
      <title>{title}</title>
    </Icon>
  ));
  Titled.displayName = `Titled(${title})`;
  return Titled;
}

export const ActivityIcon = titled(Activity, "What you work on");
const TimerIcon = titled(Timer, "TimeTracker timer");

/** The timer row's choices, "None" (`null`) first; empty when the project shows no row. */
export const timerChoices = (project: ActivityProject | undefined): (string | null)[] =>
  project?.timerRow ? [null, ...project.ttProjects.map((tt) => tt.id)] : [];

/** The Daybook projects as a segmented control; Tab and ⌘1…9 pick one. */
export function ProjectPicker({ projects, picked, onPick }: { projects: ActivityProject[]; picked: string; onPick: (name: string) => void }) {
  if (projects.length === 0) return null;
  return (
    <span className="activity-picker" data-stage="panel:projects" data-stage-label="Project picker (⌘1…⌘9)">
      <PanelSegmented
        label="Project"
        segments={projects.map((project) => ({ id: project.name, label: project.name }))}
        value={picked}
        onPick={onPick}
      />
    </span>
  );
}

/** The timer's TimeTracker project, when the picked project links several: "None" or one of them; ⌥0…9 pick. */
export function TimerRow({ project, picked, onPick }: { project: ActivityProject; picked: string | null; onPick: (id: string | null) => void }) {
  return (
    <div className="activity-timer" data-stage="panel:timer" data-stage-label="Timer project (⌥0…⌥9)">
      <TimerIcon size={20} aria-hidden="true" />
      <PanelSegmented
        label="Timer"
        segments={[
          { id: NO_TIMER, label: "None", title: "No timer" },
          ...project.ttProjects.map((tt) => ({ id: tt.id, label: tt.name, title: tt.client ?? undefined })),
        ]}
        value={picked ?? NO_TIMER}
        onPick={(id) => onPick(id === NO_TIMER ? null : id)}
      />
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
    <PanelList selection="tint" className="activity-suggestions" aria-label="Suggestions">
      {items.length === 0 && <li className="panel-empty">Press ↵ to start it.</li>}
      {items.flatMap((suggestion, index) => {
        const key = `${suggestion.group}\u0000${suggestion.display}`;
        const row = (
          <PanelOption
            key={key}
            ref={index === highlighted ? activeRef : undefined}
            selected={index === highlighted}
            icon={History}
            className="activity-suggestion"
            onClick={() => onPick(suggestion)}
          >
            <span className="grow-text">{suggestion.display}</span>
          </PanelOption>
        );
        const startsGroup = index === 0 || items[index - 1].group !== suggestion.group;
        if (!startsGroup) return [row];
        const heading = GROUPS.find((group) => group.group === suggestion.group)?.heading;
        return [<SectionHeader key={`${key}\u0000heading`}>{heading}</SectionHeader>, row];
      })}
    </PanelList>
  );
}
