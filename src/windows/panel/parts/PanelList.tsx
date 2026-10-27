import type { ComponentProps, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";

const classes = (...names: (string | false | null | undefined)[]) => names.filter(Boolean).join(" ") || undefined;

type PanelListProps = ComponentProps<"ul"> & {
  /** The selected row's colour: grey, or the context's tint. */
  selection?: "grey" | "tint";
};

/** A listbox under the field or prompt, with the rule above it. */
export function PanelList({ selection = "grey", className, ...list }: PanelListProps) {
  return <ul role="listbox" {...list} className={classes("palette-list", selection === "tint" && "tint", className)} />;
}

type PanelOptionProps = ComponentProps<"li"> & {
  selected: boolean;
  /** The shortcut column ("⌘1", or "1" with `narrowNumber`); an empty string keeps the column without a key. */
  number?: string;
  narrowNumber?: boolean;
  icon?: LucideIcon;
  /** The icon in secondary text (status) instead of tertiary. */
  iconSecondary?: boolean;
  /** Clicks leave the keyboard where it is (the field); on by default. */
  keepFocus?: boolean;
};

/** A palette row with an optional shortcut column and icon. */
export function PanelOption({ selected, number, narrowNumber, icon: Icon, iconSecondary, keepFocus = true, className, onMouseDown, children, ...row }: PanelOptionProps) {
  return (
    <li
      role="option"
      aria-selected={selected}
      {...row}
      className={classes(selected && "active", className)}
      onMouseDown={(e) => {
        if (keepFocus) e.preventDefault();
        onMouseDown?.(e);
      }}
    >
      {number !== undefined && <span className={narrowNumber ? "panel-number narrow" : "panel-number"}>{number}</span>}
      {Icon && <Icon size={16} className={iconSecondary ? "o2" : "o3"} aria-hidden="true" />}
      {children}
    </li>
  );
}

/** A group's heading inside a `PanelList`. */
export function SectionHeader({ children }: { children: ReactNode }) {
  return (
    <li role="presentation" className="palette-section">
      {children}
    </li>
  );
}

/** "Current", in a context's colour: the given one's, else the current context's. */
export function CurrentMarker({ context }: { context?: string | null }) {
  return (
    <span className="current-marker" data-context={context ?? undefined}>
      Current
    </span>
  );
}
