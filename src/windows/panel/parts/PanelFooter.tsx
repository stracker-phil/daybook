import type { ReactNode } from "react";
import { DRAG_REGION } from "../dragRegion";

export interface PanelHint {
  /** One cap per key: `["⌘", "L"]`, `["↑", "↓"]`, `["esc"]`. */
  keys: string[];
  label: string;
}

export const ESCAPE_HINT: PanelHint = { keys: ["esc"], label: "Cancel" };
export const CLOSE_HINT: PanelHint = { keys: ["esc"], label: "Close" };

/** One key as an outline cap. */
export function PanelKey({ children }: { children: ReactNode }) {
  return <kbd className={children === "↵" ? "panel-key enter" : "panel-key"}>{children}</kbd>;
}

/** A hint's key caps, then its label. */
export function PanelHintItem({ keys, label }: PanelHint) {
  return (
    <span className="panel-hint">
      {keys.map((key, index) => (
        <PanelKey key={index}>{key}</PanelKey>
      ))}
      <span>{label}</span>
    </span>
  );
}

/**
 * The bottom bar: the error, else the status, on the left; the key hints on the right. `children` sit between
 * them (the palette's prefix buttons). Dragging it moves the overlay.
 */
export function PanelFooter({ hints = [], status, error, children }: {
  hints?: PanelHint[];
  status?: ReactNode;
  error?: string | null;
  children?: ReactNode;
}) {
  return (
    <div className="palette-foot" {...DRAG_REGION}>
      <div className={error ? "panel-footer-status palette-error" : "panel-footer-status"}>{error || status}</div>
      {children}
      {hints.map((hint) => (
        <PanelHintItem key={hint.label} {...hint} />
      ))}
    </div>
  );
}
