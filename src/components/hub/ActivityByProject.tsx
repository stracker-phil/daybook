import type { DayActivityProject } from "../../api";
import { Copyable, Duration, projectColor } from "./activityParts";

/** The day's recordings per TimeTracker project, in first-appearance order, each project with its total. */
export function ActivityByProject({ projects }: { projects: DayActivityProject[] }) {
  return (
    <div className="activity-projects">
      {projects.map((group) => (
        <section key={group.projectId} className="activity-project" aria-label={group.project}>
          <div className="activity-project-head">
            <span className="accent-dot" aria-hidden="true" style={{ background: projectColor(group.color, group.shade) }} />
            <span className="activity-project-name">{group.project}</span>
            {group.client && <span className="o3">{group.client}</span>}
            <span className="spacer" />
            <Duration text={group.total} strong />
          </div>
          {group.recordings.map((recording) => (
            <div key={recording.recordingId} className="activity-recording">
              <div className="activity-recording-text">
                <Copyable text={recording.note || group.project} what="the note" title="Copy the note">
                  {recording.note || "No description"}
                </Copyable>
                <span className="activity-recording-detail">{[recording.tag, recording.spans].filter(Boolean).join(" · ")}</span>
              </div>
              <Duration text={recording.duration} />
            </div>
          ))}
        </section>
      ))}
    </div>
  );
}
