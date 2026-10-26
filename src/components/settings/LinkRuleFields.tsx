import { useRef } from "react";
import type { EventWindow, MinuteRange, QuickLink } from "../../api";
import { NumberField, Row, Switch } from "./controls";
import { CommitInput, TimeRangeField } from "./fields";

const NEW_TIME: MinuteRange = { from: 9 * 60, to: 17 * 60 };

const WINDOWS: { value: EventWindow; label: string }[] = [
  { value: "until_end", label: "Until the meeting ends" },
  { value: "during", label: "During the meeting" },
  { value: "before", label: "Before and during it" },
];

/** Minutes after midnight as `HH:MM`. */
export const clockTime = (minutes: number) => `${String(Math.floor(minutes / 60)).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`;

function minutesOf(time: string): number | null {
  const match = /^(\d{1,2}):(\d{2})$/.exec(time);
  return match ? Number(match[1]) * 60 + Number(match[2]) : null;
}

/** A time window, set as a whole: both ends or none. A range that ends before it starts isn't saved. */
export function LinkTimeFields({ time, onChange }: { time: MinuteRange | null; onChange: (time: MinuteRange | null) => void }) {
  // Switching the time off and on again brings back the times you had.
  const last = useRef(time ?? NEW_TIME);
  if (time) last.current = time;
  const change = (range: { from: string; to: string }) => {
    const from = minutesOf(range.from);
    const to = minutesOf(range.to);
    if (from !== null && to !== null && from < to) onChange({ from, to });
  };
  return (
    <>
      <Switch label="Time" description="Only between these times of day." checked={!!time} onChange={(on) => onChange(on ? last.current : null)} />
      {time && (
        <Row label="Between" nested>
          <TimeRangeField label="Shows" from={clockTime(time.from)} to={clockTime(time.to)} onChange={change} />
        </Row>
      )}
    </>
  );
}

/** A meeting in today's calendar whose title contains the text, and when around it the link shows. */
export function LinkMeetingFields({ link, onChange }: { link: QuickLink; onChange: (patch: Partial<QuickLink>) => void }) {
  return (
    <>
      <Row label="Meeting">
        <CommitInput
          label="Meeting"
          className="link-input"
          placeholder="Only on days with a calendar event containing … (optional)"
          value={link.event ?? ""}
          onCommit={(event) => onChange({ event: event || null })}
        />
      </Row>
      {link.event && (
        <Row label="Shows" nested>
          <select className="input" aria-label="Shows" value={link.eventWindow} onChange={(e) => onChange({ eventWindow: e.target.value as EventWindow })}>
            {WINDOWS.map((window) => (
              <option key={window.value} value={window.value}>
                {window.label}
              </option>
            ))}
          </select>
        </Row>
      )}
      {link.event && link.eventWindow === "before" && (
        <NumberField label="Minutes before" value={link.leadMinutes} min={1} max={240} suffix="min" nested onCommit={(leadMinutes) => onChange({ leadMinutes })} />
      )}
    </>
  );
}
