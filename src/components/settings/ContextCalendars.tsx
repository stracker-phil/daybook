import type { CalendarInfo } from "../../api";
import { Row } from "./controls";

/** The calendars whose events count while a context is current; chosen ones missing on this Mac stay listed. */
export function ContextCalendars({ info, chosen, onChange }: { info: CalendarInfo; chosen: string[]; onChange: (calendars: string[]) => void }) {
  const toggle = (title: string, on: boolean) => onChange(on ? [...chosen, title] : chosen.filter((entry) => entry !== title));
  const missing = info.access === "full-access" ? chosen.filter((title) => !info.calendars.includes(title)) : [];
  const rows = [...info.calendars.map((title) => ({ title, missing: false })), ...missing.map((title) => ({ title, missing: true }))];
  const description =
    info.calendars.length === 0
      ? "Allow calendar access in Settings → Calendar to choose calendars."
      : "Their events show in the Daybook window's timeline and remind you of meetings.";
  return (
    <Row label="Calendars" description={description} stacked>
      {rows.length > 0 && (
        <div className="check-row">
          {rows.map(({ title, missing }) => (
            <label key={title} className="check">
              <input type="checkbox" checked={missing || chosen.includes(title)} onChange={(e) => toggle(title, e.target.checked)} />
              {title}
              {missing && <span className="o3">not on this Mac</span>}
            </label>
          ))}
        </div>
      )}
    </Row>
  );
}
