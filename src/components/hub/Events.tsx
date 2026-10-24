import { useState, type ReactNode } from "react";
import { Eye, EyeOff } from "lucide-react";
import { api, type EventRow, type HubView } from "../../api";
import { fail } from "../../state/requests";
import { useStore } from "../../state/store";
import { DayTimeline } from "./DayTimeline";

function Row({ row, highlighted, onHighlight }: { row: EventRow; highlighted: boolean; onHighlight: (key: string | null) => void }) {
  const meeting = row.meeting;
  const props = {
    className: "event-row",
    "data-highlighted": highlighted,
    "data-past": row.past,
    onMouseEnter: () => onHighlight(row.key),
    onMouseLeave: () => onHighlight(null),
  };
  const content = (
    <>
      <span className="event-time">{row.time}</span>
      <span className="grow-text">{row.title}</span>
      {meeting && <span className="event-app">{meeting.app}</span>}
      <span className="event-calendar o3">{row.calendar}</span>
    </>
  );
  if (!meeting) return <div {...props}>{content}</div>;
  const join = () => void api.calendarJoin(meeting.url).catch((e) => fail(e, "Couldn't join the meeting"));
  return (
    <button type="button" title={`Join the ${meeting.app} meeting`} onClick={join} {...props}>
      {content}
    </button>
  );
}

function Empty({ children }: { children: ReactNode }) {
  return <p className="board-empty o3">{children}</p>;
}

/** The day's events: the timeline above, the list below; past events can be hidden from the list today. */
export function Events({ hub }: { hub: HubView }) {
  const [highlighted, setHighlighted] = useState<string | null>(null);
  const hidePast = useStore((s) => s.uiState.hidePastEvents);
  const togglePast = useStore((s) => s.toggleHidePastEvents);
  const { timeline, rows } = hub.events;
  const hiding = hidePast && hub.isToday;
  const listed = hiding ? rows.filter((row) => !row.past) : rows;
  const hidden = rows.length - listed.length;
  const title = hub.isToday ? "Today’s events" : "Events";
  const toggleLabel = hidePast ? "Show Past Events" : "Hide Past Events";

  return (
    <section className="board-section" aria-label={title}>
      <header className="board-section-head">
        <h2 className="board-section-title">{title}</h2>
        {rows.length > 0 && hub.isToday && (
          <button type="button" className="icon-btn" aria-label={toggleLabel} aria-pressed={hidePast} title={toggleLabel} onClick={togglePast}>
            {hidePast ? <EyeOff size="1.077rem" aria-hidden="true" /> : <Eye size="1.077rem" aria-hidden="true" />}
          </button>
        )}
      </header>
      <DayTimeline timeline={timeline} highlighted={highlighted} onHighlight={setHighlighted} />
      {rows.length === 0 && <Empty>{hub.isToday ? "No events today." : "No events."}</Empty>}
      {rows.length > 0 && listed.length === 0 && <Empty>No more events today.</Empty>}
      {hiding && hidden > 0 && listed.length > 0 && (
        <p className="board-note o3">
          {hidden} past {hidden === 1 ? "event" : "events"} hidden
        </p>
      )}
      <div className="event-rows">
        {listed.map((row) => (
          <Row key={row.key} row={row} highlighted={row.key === highlighted} onHighlight={setHighlighted} />
        ))}
      </div>
    </section>
  );
}
