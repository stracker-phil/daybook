import { api, type ActionLogEvent as ActionLogEntry } from "../../api";
import { fail } from "../../state/requests";
import { ActionLogEvent } from "./ActionLogEvent";

interface Day {
  day: string;
  label: string;
  events: ActionLogEntry[];
}

/** Rust sends the events newest first; consecutive events of a day share its header. */
function byDay(events: ActionLogEntry[]): Day[] {
  const days: Day[] = [];
  for (const event of events) {
    const last = days.at(-1);
    if (last?.day === event.day) last.events.push(event);
    else days.push({ day: event.day, label: event.dayLabel, events: [event] });
  }
  return days;
}

export function ActionLogSection({ events, onCleared }: { events: ActionLogEntry[]; onCleared: () => void }) {
  const clear = async () => {
    try {
      await api.actionLogClear();
      onCleared();
    } catch (e) {
      fail(e, "Couldn't clear the action log");
    }
  };
  return (
    <section className="settings-group" aria-label="Action log">
      <div className="settings-group-head">
        <h3>Action log</h3>
        <button type="button" className="btn" disabled={events.length === 0} onClick={() => void clear()}>
          Clear Log
        </button>
      </div>
      <p className="settings-group-desc">What Daybook sent to Obsidian, Slack and TimeTracker, grouped by what triggered it.</p>
      {events.length === 0 && <p className="settings-card action-log-empty">Nothing sent yet.</p>}
      {byDay(events).map((day) => (
        <div key={day.day} className="action-day">
          <h4>{day.label}</h4>
          <div className="settings-card">
            {day.events.map((event) => (
              <ActionLogEvent key={event.id} event={event} />
            ))}
          </div>
        </div>
      ))}
    </section>
  );
}
