import type { UiState } from "../../api";
import { useStore } from "../../state/store";
import { SegmentedControl } from "../SegmentedControl";
import { Toolbar } from "../Toolbar";
import { ActivityTab } from "./ActivityTab";
import { TimeTrackerButton } from "./TimeTrackerButton";
import { TodoList, useOpenTodos } from "./TodoList";

const TABS = ["todos", "activity"] as const satisfies readonly UiState["inspectorTab"][];

/**
 * The Daybook window's right column: the open todos in the daily notes (the same on every day), or the shown day's
 * activity in TimeTracker, where Rust offers it. Ticking a todo writes it to its note.
 */
export function Inspector() {
  const hub = useStore((s) => s.hub);
  const chosen = useStore((s) => s.uiState.inspectorTab);
  const choose = useStore((s) => s.setInspectorTab);
  // Without the tab the inspector shows the todos and keeps the choice for when it returns.
  const offered = hub?.activityTab ?? false;
  const tab = offered ? chosen : "todos";
  const { open: groups, count, tick } = useOpenTodos(hub?.todos);
  const title = tab === "activity" ? "Activity" : groups ? `${count} open` : "Todos";
  return (
    <aside className="inspector" aria-label="Inspector" data-stage="hub:inspector" data-stage-label="Inspector: open todos or the day's activity" data-stage-role="region">
      <Toolbar title={title}>
        {tab === "activity" && hub && <TimeTrackerButton link={hub.timetracker} stage="hub:activity-timetracker" />}
        {offered && <SegmentedControl label="Inspector tab" options={TABS} value={tab} onChange={choose} />}
      </Toolbar>
      <div className="inspector-body">{tab === "activity" && hub ? <ActivityTab hub={hub} /> : <TodoList groups={groups} count={count} onTick={(todo) => void tick(todo)} />}</div>
    </aside>
  );
}
