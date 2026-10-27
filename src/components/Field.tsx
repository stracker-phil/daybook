import type { ReactNode } from "react";
import { shortcutKeys } from "../lib/shortcuts";

/** A key combination as a small cap (`⌘K`); renders nothing for an unbound command. */
export function KeyCap({ keys }: { keys: string }) {
  if (!keys) return null;
  return <kbd className="keycap">{keys}</kbd>;
}

/** A global shortcut in Settings notation (`ctrl+alt+cmd+f`) as a key cap; nothing without one. */
export function ShortcutCap({ shortcut }: { shortcut?: string | null }) {
  return <KeyCap keys={shortcutKeys(shortcut)?.join("") ?? ""} />;
}

interface InsetFieldProps {
  icon: ReactNode;
  /** Key cap text, e.g. `⌘⇧F`; empty for none. */
  keys?: string;
  className?: string;
  children: ReactNode;
}

/**
 * The macOS field frame (docs/ux.md, Inputs): a leading icon, the control, an optional key cap. The frame is
 * a label, so a click anywhere on it focuses the input.
 */
export function InsetField({ icon, keys = "", className = "", children }: InsetFieldProps) {
  return (
    <label className={`inset-field ${className}`.trim()}>
      <span className="inset-field-icon" aria-hidden="true">{icon}</span>
      {children}
      <KeyCap keys={keys} />
    </label>
  );
}
