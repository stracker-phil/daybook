import { useRef } from "react";
import type { OpenEvent, Timing } from "../../api";
import { Row, Switch } from "./controls";
import { TimeRangeField } from "./fields";

const NEW_DUE = { from: "09:00", to: "" };

/**
 * When a routine or journal group is due (the Daybook window reminds while it isn't done) and when it opens
 * by itself. `contextEvent` names the context switch that counts for this item.
 */
export function TimingFields({ timing, contextEvent, onChange }: { timing: Timing; contextEvent: string; onChange: (timing: Timing) => void }) {
  const due = timing.due;
  // Switching the due time off and on again brings back the times you had.
  const lastDue = useRef(due ?? NEW_DUE);
  if (due) lastDue.current = due;
  const events: { value: OpenEvent; label: string }[] = [
    { value: "start", label: "On the first Working of the day" },
    { value: "context", label: contextEvent },
    { value: "meeting-start", label: "When a call starts" },
    { value: "meeting-end", label: "When a call ends" },
    { value: "off", label: "On “Done for today”" },
  ];
  const toggle = (event: OpenEvent, on: boolean) => onChange({ ...timing, openOn: on ? [...timing.openOn, event] : timing.openOn.filter((e) => e !== event) });

  return (
    <>
      <Switch
        label="Due"
        description="While due and not done, the Daybook window reminds you (Settings → Reminders sets the order)."
        checked={!!due}
        onChange={(on) => onChange(on ? { ...timing, due: lastDue.current } : { ...timing, due: null, openWhenDue: false })}
      />
      {due && (
        <Row label="Due between" nested>
          <TimeRangeField label="Due" from={due.from} to={due.to} openEnd onChange={(range) => onChange({ ...timing, due: range })} />
        </Row>
      )}
      <Row label="Opens by itself" description="Only while something is still open, and never while you're away." stacked>
        <div className="check-column">
          <label className="check">
            <input type="checkbox" checked={timing.openWhenDue} disabled={!due} onChange={(e) => onChange({ ...timing, openWhenDue: e.target.checked })} />
            When it's due (once a day)
          </label>
          {events.map((event) => (
            <label key={event.value} className="check">
              <input type="checkbox" checked={timing.openOn.includes(event.value)} onChange={(e) => toggle(event.value, e.target.checked)} />
              {event.label}
            </label>
          ))}
        </div>
      </Row>
    </>
  );
}
