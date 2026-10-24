import { useEffect, useRef } from "react";
import { api, type ReminderAction, type ReminderLine } from "../../api";
import { fail } from "../../state/requests";

function run(action: ReminderAction) {
  const done = action.kind === "join" ? api.calendarJoin(action.url) : api.runCommand(action.command);
  done.catch((e) => fail(e, `Couldn't ${action.label.toLowerCase()}`));
}

/** The reminder as one line; with an action the line is a button that takes the focus on open, so ↵ runs it. */
function Reminder({ reminder }: { reminder: ReminderLine }) {
  const button = useRef<HTMLButtonElement>(null);
  const action = reminder.action;
  // The hub reloads every minute; only a different action takes the focus again.
  const actionKey = action && JSON.stringify(action);
  useEffect(() => {
    if (!actionKey) return;
    const focus = () => button.current?.focus({ preventScroll: true });
    focus();
    const unlisten = api.onWindowOpened(focus);
    return () => void unlisten.then((fn) => fn()).catch(() => {});
  }, [actionKey]);

  const content = (
    <>
      <span className="reminder-label o3">{reminder.label}</span>
      <span className="grow-text">{reminder.text}</span>
      {action && <span className="reminder-action">{action.label}</span>}
    </>
  );
  if (!action) return <div className="reminder">{content}</div>;
  return (
    <button ref={button} type="button" className="reminder" onClick={() => run(action)}>
      {content}
    </button>
  );
}

export function TodayCard({ reminder }: { reminder: ReminderLine | null }) {
  if (!reminder) return null;
  return (
    <div className="today-card">
      <Reminder reminder={reminder} />
    </div>
  );
}
