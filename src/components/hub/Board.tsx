import { PanelRight } from "lucide-react";
import { useBindingLabel } from "../../commands";
import { useStore } from "../../state/store";
import { Toolbar } from "../Toolbar";
import { Events } from "./Events";
import { TodayCard } from "./TodayCard";

/** The day in the middle of the Daybook window, under the greeting: the reminder, then the day's events. */
export function Board() {
  const hub = useStore((s) => s.hub);
  const title = hub?.title ?? "";
  const hidden = useStore((s) => s.uiState.inspectorHidden);
  const toggle = useStore((s) => s.toggleInspector);
  const keys = useBindingLabel("inspector.toggle");
  const label = hidden ? "Show Inspector" : "Hide Inspector";
  return (
    <div className="view-page">
      <Toolbar title={title}>
        <button type="button" className="icon-btn" aria-label={label} aria-pressed={!hidden} title={keys ? `${label} (${keys})` : label} onClick={toggle}>
          <PanelRight size="1.077rem" aria-hidden="true" />
        </button>
      </Toolbar>
      <div className="board">
        {hub && (
          <div className="board-content">
            <TodayCard reminder={hub.reminder} />
            <Events hub={hub} />
          </div>
        )}
      </div>
    </div>
  );
}
