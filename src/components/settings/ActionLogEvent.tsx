import { Check, CircleMinus, CircleUserRound, CircleX, Clock, Gem, LoaderCircle, MessageSquare, Sparkles, Timer, type LucideIcon } from "lucide-react";
import type { ActionLogEvent as LogEvent, ActionLogLine } from "../../api";

const TARGETS: Record<ActionLogLine["target"], { icon: LucideIcon; label: string }> = {
  obsidian: { icon: Gem, label: "Obsidian" },
  slackStatus: { icon: CircleUserRound, label: "Slack status" },
  slackMessage: { icon: MessageSquare, label: "Slack message" },
  timetracker: { icon: Timer, label: "TimeTracker" },
  daybook: { icon: Sparkles, label: "Daybook" },
};

const STATES: Record<ActionLogLine["state"], { icon: LucideIcon; label: string }> = {
  sending: { icon: LoaderCircle, label: "Sending" },
  done: { icon: Check, label: "Done" },
  waiting: { icon: Clock, label: "Waiting" },
  skipped: { icon: CircleMinus, label: "Not sent" },
  failed: { icon: CircleX, label: "Failed" },
};

/** `eventTime` hides a line's time when it is the event's own. */
function Line({ line, eventTime }: { line: ActionLogLine; eventTime: string }) {
  const target = TARGETS[line.target];
  const state = STATES[line.state];
  const sub = [line.time !== eventTime && line.time, line.detail].filter(Boolean).join(" · ");
  return (
    <li className="action-line" data-target={line.target} data-state={line.state}>
      <state.icon className="action-line-state" size={14} role="img" aria-label={state.label} />
      <span className="action-line-target">
        <target.icon size={13} aria-hidden="true" />
        {target.label}
      </span>
      <span className="action-line-body">
        <span className="action-line-text">
          <span className="selectable">{line.text}</span>
          {line.count > 1 && <span className="action-line-count">×{line.count}</span>}
        </span>
        {sub && <span className="action-line-detail selectable">{sub}</span>}
      </span>
    </li>
  );
}

/** One trigger: its time and title, then what it sent (×N for merged repeats). */
export function ActionLogEvent({ event }: { event: LogEvent }) {
  return (
    <article className="action-event" aria-label={event.trigger}>
      <header className="action-event-head">
        <time className="action-event-time">{event.time}</time>
        <span className="action-event-trigger">{event.trigger}</span>
      </header>
      <ul className="action-lines">
        {event.lines.map((line, index) => (
          <Line key={`${line.target}-${index}`} line={line} eventTime={event.time} />
        ))}
      </ul>
    </article>
  );
}
