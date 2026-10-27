import { useEffect, useRef } from "react";
import { Check, Minus, Plus, type LucideIcon } from "lucide-react";
import { api, type FocusLine, type FocusMark, type ReminderAction, type ReminderLine } from "../../api";
import { fail, runBusCommand } from "../../state/requests";
import { ShortcutCap } from "../Field";

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

const MARK_ICONS: Partial<Record<FocusMark, LucideIcon>> = { done: Check, cancelled: Minus };

function FocusCheckbox({ mark }: { mark: FocusMark }) {
  const Icon = MARK_ICONS[mark];
  return (
    <span className="focus-box" data-mark={mark} aria-hidden="true">
      {Icon && <Icon size={12} strokeWidth={3} />}
    </span>
  );
}

function FocusItem({ line }: { line: FocusLine }) {
  if (!line.mark) return <div className="focus-item sub o2">{line.text}</div>;
  return (
    <div className="focus-item" data-mark={line.mark}>
      <FocusCheckbox mark={line.mark} />
      {line.tag && <span className="focus-tag o2">{line.tag}</span>}
      <span className="focus-text">{line.text}</span>
    </div>
  );
}

/** The day's focus list as the note holds it; a click opens the focus overlay, which edits today's only. */
function Focus({ lines, isToday, shortcut }: { lines: FocusLine[]; isToday: boolean; shortcut: string | null }) {
  const empty = lines.length === 0;
  const open = () => runBusCommand("focus:open", "Couldn't open the focus list");
  return (
    <section className="board-section focus-section" aria-label="Focus">
      <header className="board-section-head">
        <h2 className="board-section-title">
          Focus
          {isToday && <ShortcutCap shortcut={shortcut} />}
        </h2>
      </header>
      <button type="button" className="focus-list" data-stage="hub:focus" disabled={!isToday} aria-label={empty ? "Set Today's Focus" : "Edit Today's Focus"} onClick={open}>
        {empty && !isToday && <span className="o3">No focus list.</span>}
        {empty && isToday && (
          <span className="focus-add">
            <Plus size={16} aria-hidden="true" />
            Set today's focus…
          </span>
        )}
        {lines.map((line, i) => (
          <FocusItem key={i} line={line} />
        ))}
      </button>
    </section>
  );
}

export function TodayCard({
  reminder,
  focus,
  isToday,
  focusShortcut = null,
}: {
  reminder: ReminderLine | null;
  focus: FocusLine[] | null;
  isToday: boolean;
  /** The global shortcut that opens the focus list, shown beside its title. */
  focusShortcut?: string | null;
}) {
  if (!reminder && !focus) return null;
  return (
    <div className="today-card">
      {reminder && <Reminder reminder={reminder} />}
      {reminder && focus && <div role="separator" className="today-separator" />}
      {focus && <Focus lines={focus} isToday={isToday} shortcut={focusShortcut} />}
    </div>
  );
}
