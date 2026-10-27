import type { MouseEvent } from "react";
import { Sparkles } from "lucide-react";
import type { DayActivityRow } from "../../api";
import { copyText } from "../../lib/clipboard";
import { Copyable, Duration, projectColor } from "./activityParts";

/** A project-coloured ray marker; gaps stay uncoloured. */
function Marker({ row }: { row: DayActivityRow }) {
  const color = row.kind === "gap" ? undefined : projectColor(row.color, row.shade);
  return (
    <span className="ray-marker" aria-hidden="true" style={color ? { color } : undefined}>
      <span className="ray-dot" />
      <span className="ray-line" />
    </span>
  );
}

function RowText({ row }: { row: DayActivityRow }) {
  if (row.copyText === null) return <span className="ray-quiet">{row.title}</span>;
  return (
    <>
      <Copyable text={row.copyText} what="the note" title="Copy the note" className="ray-text">
        {row.project ? (
          <>
            <span className="ray-project">{row.project}</span>
            {row.note && ` · ${row.note}`}
          </>
        ) : (
          row.title
        )}
      </Copyable>
      {row.suggestedBy && (
        <span className="ray-by">
          <Sparkles size="0.846rem" aria-hidden="true" />
          {row.suggestedBy}
        </span>
      )}
    </>
  );
}

/** A click anywhere on a recorded span copies its note; suggestions copy only through their note button. */
function copyRow({ kind, copyText: text }: DayActivityRow) {
  if (text === null || (kind !== "work" && kind !== "running")) return undefined;
  return (event: MouseEvent) => {
    if (!(event.target as HTMLElement).closest("button")) void copyText(text, "the note");
  };
}

/** The day as a ray of time; `end` closes the ray. */
export function ActivityTimeline({ rows, end }: { rows: DayActivityRow[]; end: string | null }) {
  return (
    <ol className="ray" aria-label="Timeline">
      {rows.map((row) => (
        <li key={`${row.kind}:${row.start}:${row.recordingId ?? row.title}`} className={`ray-row ${row.kind}`} onClick={copyRow(row)}>
          <span className="ray-time">{row.start}</span>
          <div className="ray-span">
            <Marker row={row} />
            <div className="ray-body">
              <RowText row={row} />
              <Duration text={row.duration} />
            </div>
          </div>
        </li>
      ))}
      {end && (
        <li className="ray-row">
          <span className="ray-time">{end}</span>
        </li>
      )}
    </ol>
  );
}
