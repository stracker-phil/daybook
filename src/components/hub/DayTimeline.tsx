import type { CSSProperties } from "react";
import type { DayTimeline as Timeline, Span } from "../../api";

const percent = (fraction: number) => `${fraction * 100}%`;
const between = (span: Span): CSSProperties => ({ left: percent(span.from), width: percent(span.to - span.from) });

/** The day as a ruler; hovering a block highlights its event row and back. */
export function DayTimeline({ timeline, highlighted, onHighlight }: { timeline: Timeline; highlighted: string | null; onHighlight: (key: string | null) => void }) {
  const fullDayHighlighted = timeline.fullDay.some((key) => key === highlighted);
  return (
    <div className="timeline" role="img" aria-label="The day’s timeline">
      {timeline.hours.map((hour, i) => (
        <div key={i} className="timeline-hour" style={{ left: percent(hour.at) }}>
          <span className="timeline-label" style={{ visibility: hour.labeled ? "visible" : "hidden" }}>
            {hour.label}
          </span>
          <span className="timeline-tick" />
        </div>
      ))}
      <div className="timeline-bar">
        {timeline.workHours && <div className="timeline-work" style={between(timeline.workHours)} />}
        {timeline.fullDay.length > 0 && <div className="timeline-full-day" data-highlighted={fullDayHighlighted} />}
        {timeline.blocks.map((block) => (
          <div
            key={block.key}
            className="timeline-block"
            style={between(block.span)}
            title={block.label}
            data-follows={block.follows}
            data-past={block.past}
            data-highlighted={block.key === highlighted}
            onMouseEnter={() => onHighlight(block.key)}
            onMouseLeave={() => onHighlight(null)}
          >
            <div className="timeline-block-fill" />
          </div>
        ))}
      </div>
      {timeline.now !== null && <div className="timeline-now" aria-hidden="true" style={{ left: percent(timeline.now) }} />}
    </div>
  );
}
