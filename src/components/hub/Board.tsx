import { PanelRight } from "lucide-react";
import { useBindingLabel } from "../../commands";
import { useStore } from "../../state/store";
import { Toolbar } from "../Toolbar";

/** The day in the middle of the Daybook window, under the greeting. */
export function Board() {
  const title = useStore((s) => s.hub?.title ?? "");
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
      <div className="board" />
    </div>
  );
}
