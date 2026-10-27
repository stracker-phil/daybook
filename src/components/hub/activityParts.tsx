import type { ReactNode } from "react";
import { copyText } from "../../lib/clipboard";
import { accentColor } from "../Accent";

/** A project without a TimeTracker colour draws in graphite. */
export const projectColor = (color: string | null, shade: number | null) => accentColor(color ?? "graphite", shade ?? 4);

/** Text that copies itself on click; `what` ends the toast ("Copied the note"). */
export function Copyable({ text, what, title, className, children }: { text: string; what: string; title: string; className?: string; children: ReactNode }) {
  return (
    <button type="button" className={["copyable", className].filter(Boolean).join(" ")} title={title} onClick={() => void copyText(text, what)}>
      {children}
    </button>
  );
}

/** A duration as TimeTracker writes it ("1h 05m"); a click copies it. */
export function Duration({ text, strong = false }: { text: string; strong?: boolean }) {
  return (
    <Copyable text={text} what={text} title={`Copy ${text}`} className={strong ? "activity-duration strong" : "activity-duration"}>
      {text}
    </Copyable>
  );
}
