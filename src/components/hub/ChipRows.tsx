import type { ReactNode } from "react";
import { BookOpen, Check, ListChecks } from "lucide-react";
import { api, type HubView, type JournalProgress } from "../../api";
import { fail } from "../../state/requests";
import { ShortcutCap } from "../Field";

/** One dot per question or item, filled when done; a check once all are. */
function Progress({ done, total }: { done: number; total: number }) {
  if (total > 0 && done >= total) return <Check className="chip-done" size="1.077rem" aria-label="Done" />;
  return (
    <span className="chip-dots" aria-label={`${done} of ${total}`}>
      {Array.from({ length: total }, (_, i) => (
        <span key={i} className="chip-dot" data-done={i < done} />
      ))}
    </span>
  );
}

/** A routine or journal group in one line; without `onClick` it only shows its state (another day). */
function Chip({ icon, title, trailing, onClick }: { icon: ReactNode; title: string; trailing: ReactNode; onClick?: () => void }) {
  return (
    <button type="button" className="chip" disabled={!onClick} onClick={onClick}>
      <span className="chip-icon o3">{icon}</span>
      <span className="chip-title">{title}</span>
      {trailing}
    </button>
  );
}

function ChipRow({ label, shortcut, children }: { label: string; shortcut?: string | null; children: ReactNode }) {
  return (
    <section className="chip-row" aria-label={label}>
      <h2 className="chip-row-title">
        {label}
        {shortcut && <ShortcutCap shortcut={shortcut} />}
      </h2>
      <div className="chip-list">{children}</div>
    </section>
  );
}

function JournalTrailing({ group }: { group: JournalProgress }) {
  if (group.total > 0) return <Progress done={group.answered} total={group.total} />;
  return group.hasNote ? <Progress done={1} total={1} /> : null;
}

function runOrFail(command: string, failure: string) {
  return () => void api.runCommand(command).catch((e) => fail(e, failure));
}

function shortcutOf(hub: HubView, command: string): string | null {
  for (const tool of hub.tools) if (tool.kind === "tool" && tool.command === command) return tool.shortcut;
  return null;
}

/** The current context's routines on the day; today's open the routine's checklist. */
export function RoutinesRow({ hub }: { hub: HubView }) {
  if (hub.routines.length === 0) return null;
  return (
    <ChipRow label="Routines">
      {hub.routines.map((routine) => (
        <Chip
          key={routine.id}
          icon={<ListChecks size="1.077rem" aria-hidden="true" />}
          title={routine.name}
          trailing={routine.total > 0 ? <Progress done={routine.done} total={routine.total} /> : null}
          onClick={hub.isToday ? runOrFail(`routine:${routine.id}`, "Couldn't open the routine") : undefined}
        />
      ))}
    </ChipRow>
  );
}

/** The day's journal groups; today's open the journal overlay on that group. */
export function JournalRow({ hub }: { hub: HubView }) {
  if (hub.journal.length === 0) return null;
  return (
    <ChipRow label="Journal" shortcut={shortcutOf(hub, "journal:open")}>
      {hub.journal.map((group) => (
        <Chip
          key={group.title}
          icon={<BookOpen size="1.077rem" aria-hidden="true" />}
          title={group.title}
          trailing={<JournalTrailing group={group} />}
          onClick={hub.isToday ? runOrFail(`journal:open:${group.title}`, "Couldn't open the journal") : undefined}
        />
      ))}
    </ChipRow>
  );
}
