import { PanelRight } from "lucide-react";
import { useBindingLabel } from "../../commands";
import { useStageMeta } from "../../stage/meta";
import { useStore } from "../../state/store";
import { Toolbar } from "../Toolbar";
import { JournalRow, RoutinesRow } from "./ChipRows";
import { Events } from "./Events";
import { TimeTrackerButton } from "./TimeTrackerButton";
import { TodayCard } from "./TodayCard";
import { TodaysNotes } from "./TodaysNotes";

/** The day in the middle of the Daybook window, under the greeting: the reminder, the day's events, then the routines, the journal and the day's notes. */
export function Board() {
  const hub = useStore((s) => s.hub);
  const title = hub?.title ?? "";
  const hidden = useStore((s) => s.uiState.inspectorHidden);
  const toggle = useStore((s) => s.toggleInspector);
  const keys = useBindingLabel("inspector.toggle");
  const label = hidden ? "Show Inspector" : "Hide Inspector";
  useStageMeta({ view: hub ? (hub.isToday ? "today" : `day:${hub.day}`) : undefined, ui: { inspector: hidden ? "closed" : "open" } });
  return (
    <div className="view-page">
      <Toolbar title={title}>
        {hub && <TimeTrackerButton link={hub.timetracker} stage="hub:timetracker" />}
        <button
          type="button"
          className="icon-btn"
          data-stage="hub:inspector-toggle"
          data-stage-label="Show or hide the inspector"
          data-stage-role="toggle"
          aria-label={label}
          aria-pressed={!hidden} title={keys ? `${label} (${keys})` : label} onClick={toggle}>
          <PanelRight size="1.077rem" aria-hidden="true" />
        </button>
      </Toolbar>
      <div className="board">
        {hub && (
          <div className="board-content">
            <TodayCard reminder={hub.reminder} focus={hub.focus} isToday={hub.isToday} />
            <Events hub={hub} />
            <div className="board-checklists" data-stage="hub:checklists" data-stage-label="Routines and journal" data-stage-role="region">
              <RoutinesRow hub={hub} />
              <JournalRow hub={hub} />
            </div>
            <TodaysNotes hub={hub} />
          </div>
        )}
      </div>
    </div>
  );
}
