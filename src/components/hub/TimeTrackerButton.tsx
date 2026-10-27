import { Timer } from "lucide-react";
import { api, type TimeTrackerLink } from "../../api";
import { fail } from "../../state/requests";

/** Runs the shown day's TimeTracker command (Rust names it and its label); TimeTracker starts if needed. */
export const showInTimeTracker = ({ command }: TimeTrackerLink) => void api.runCommand(command).catch((e) => fail(e, "Couldn't open TimeTracker"));

/** Shows the day in TimeTracker. */
export function TimeTrackerButton({ link, stage }: { link: TimeTrackerLink; stage: string }) {
  return (
    <button type="button" className="icon-btn" data-stage={stage} data-stage-label={link.label} aria-label={link.label} title={link.label} onClick={() => showInTimeTracker(link)}>
      <Timer size="1.077rem" aria-hidden="true" />
    </button>
  );
}
