import type { ComponentProps, ReactNode } from "react";
import type { LucideIcon } from "lucide-react";
import { DRAG_REGION } from "../dragRegion";

/** The small question above a view ("What is your status?"); dragging it moves the overlay. */
export function PanelPrompt({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <div className={className ? `panel-heading ${className}` : "panel-heading"} {...DRAG_REGION}>
      {children}
    </div>
  );
}

type PanelInputProps = ComponentProps<"input"> & {
  icon: LucideIcon;
  /** Between the icon and the field, e.g. a prompt, scope chips or a project picker. */
  leading?: ReactNode;
  /** The row's anchor for Stage; `panel:input` by default. */
  stage?: string;
  /** Stage's label for the row; the placeholder by default. */
  stageLabel?: string;
};

/** The field row; the field takes the keyboard on open unless `autoFocus` is turned off. */
export function PanelInput({ icon: Icon, leading, stage = "panel:input", stageLabel, autoFocus = true, ...input }: PanelInputProps) {
  return (
    <div className="palette-input" data-stage={stage} data-stage-label={stageLabel ?? input.placeholder} data-stage-role="field">
      <Icon size={20} aria-hidden="true" />
      {leading}
      <input autoFocus={autoFocus} {...input} />
    </div>
  );
}
