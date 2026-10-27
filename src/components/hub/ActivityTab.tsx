import type { HubView, UiState } from "../../api";
import { useStore } from "../../state/store";
import { SegmentedControl } from "../SegmentedControl";
import { ActivityByProject } from "./ActivityByProject";
import { ActivityTimeline } from "./ActivityTimeline";
import { Duration } from "./activityParts";
import { showInTimeTracker } from "./TimeTrackerButton";
import { useActivityDay } from "./useActivityDay";

const VIEWS = ["timeline", "byProject"] as const satisfies readonly UiState["activityView"][];

function NotRunning({ hub }: { hub: HubView }) {
  return (
    <div className="activity-state">
      <p className="o3">TimeTracker isn't running</p>
      <button type="button" className="btn" onClick={() => showInTimeTracker(hub.timetracker)}>
        Open TimeTracker
      </button>
    </div>
  );
}

/** The inspector's Activity tab: TimeTracker's recordings on the shown day, read-only, as a timeline or per project. */
export function ActivityTab({ hub }: { hub: HubView }) {
  const view = useStore((s) => s.uiState.activityView);
  const setView = useStore((s) => s.setActivityView);
  const { data } = useActivityDay(hub.day);
  if (data?.state === "notRunning") return <NotRunning hub={hub} />;
  const day = data?.activity;
  // Kept data of another day must not show while the new day loads.
  if (!day || day.day !== hub.day) return null;
  return (
    <div className="activity">
      <SegmentedControl label="Activity view" options={VIEWS} value={view} onChange={setView} labels={{ timeline: "Timeline", byProject: "By Project" }} />
      {day.totalMs > 0 && (
        <div className="activity-total">
          <span className="o3">Recorded</span>
          <span className="spacer" />
          <Duration text={day.total} strong />
        </div>
      )}
      {day.empty && <p className="board-empty o3">{day.empty}</p>}
      {view === "timeline" ? <ActivityTimeline rows={day.rows} end={day.endLabel} /> : <ActivityByProject projects={day.projects} />}
    </div>
  );
}
