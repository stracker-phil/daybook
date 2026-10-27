import type { ReactNode } from "react";

export interface PanelSegment<T extends string> {
  id: T;
  label: ReactNode;
  title?: string;
}

/** The overlay's segmented control, the picked segment accent-filled; clicks leave the keyboard in the field. */
export function PanelSegmented<T extends string>({ label, segments, value, onPick }: {
  label: string;
  segments: PanelSegment<T>[];
  /** `null` picks none. */
  value: T | null;
  onPick: (id: T) => void;
}) {
  return (
    <div className="segmented" role="group" aria-label={label}>
      {segments.map((segment) => (
        <button
          key={segment.id}
          type="button"
          tabIndex={-1}
          title={segment.title}
          aria-pressed={segment.id === value}
          onMouseDown={(e) => e.preventDefault()}
          onClick={() => onPick(segment.id)}
        >
          {segment.label}
        </button>
      ))}
    </div>
  );
}
