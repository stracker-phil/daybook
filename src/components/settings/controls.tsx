/** Shared controls for settings pages: grouped cards and rows that apply their change right away. */
import { createContext, useContext, useEffect, useId, useRef, useState, type InputHTMLAttributes, type ReactNode } from "react";
import { SegmentedControl, type SegmentedControlProps } from "../SegmentedControl";

/** The section `settings:<id>` asked for; its group scrolls into view. */
export const SectionTarget = createContext<string | undefined>(undefined);

/** `anchor`: the section id (`registerSettingsPage` `sections`) that scrolls to this group. */
export function Group({ title, description, anchor, children }: { title?: string; description?: ReactNode; anchor?: string; children: ReactNode }) {
  const target = useContext(SectionTarget);
  const ref = useRef<HTMLElement>(null);
  useEffect(() => {
    if (anchor && anchor === target) ref.current?.scrollIntoView?.({ block: "start" });
  }, [anchor, target]);
  return (
    <section ref={ref} className="settings-group" data-section={anchor}>
      {title && <h3>{title}</h3>}
      {description && <p className="settings-group-desc">{description}</p>}
      <div className="settings-card">{children}</div>
    </section>
  );
}

interface RowProps {
  label: string;
  icon?: ReactNode;
  description?: ReactNode;
  htmlFor?: string;
  /** The control goes below the label instead of beside it. */
  stacked?: boolean;
  /** Indented under the row above it, the switch it depends on. */
  nested?: boolean;
  /** Dimmed: it has no effect with the current settings. */
  disabled?: boolean;
  children?: ReactNode;
}

/** One card row: label and description on the left, the control on the right. */
export function Row({ label, icon, description, htmlFor, stacked, nested, disabled, children }: RowProps) {
  const className = ["settings-row", stacked && "stacked", nested && "nested", disabled && "disabled"].filter(Boolean).join(" ");
  return (
    <div className={className}>
      <div className="settings-row-text">
        {htmlFor ? (
          <label htmlFor={htmlFor}>{label}</label>
        ) : (
          <div className={icon ? "settings-row-label with-icon" : "settings-row-label"}>
            {icon ? (
              <>
                {icon}
                <span>{label}</span>
              </>
            ) : (
              label
            )}
          </div>
        )}
        {description && <div className="settings-row-desc">{description}</div>}
      </div>
      <div className="settings-row-control">{children}</div>
    </div>
  );
}

/** Controls without a label: one line of a repeating item, or a value right under the switch it depends on. */
export function ControlRow({ nested, stacked, children }: { nested?: boolean; stacked?: boolean; children: ReactNode }) {
  const className = ["settings-row", "control-row", nested && "nested", stacked && "stacked"].filter(Boolean).join(" ");
  return <div className={className}>{children}</div>;
}

/** The last row of an item's card: what adds to it on the left, the item's own move and remove on the right. */
export function EntryFooter({ children }: { children: ReactNode }) {
  return <div className="settings-row entry-footer">{children}</div>;
}

/** The switch that gates the block below it, with an optional accessory at the end. */
export function LeadSwitch({ label, title, checked, onChange, accessory, children }: {
  label: string;
  /** Shown instead of `label`, e.g. with a field in it; `label` then names the switch. */
  title?: ReactNode;
  checked: boolean;
  onChange: (on: boolean) => void;
  accessory?: ReactNode;
  children?: ReactNode;
}) {
  const id = useId();
  return (
    <div className="lead-block">
      <div className="lead-switch">
        <input id={id} className="switch" type="checkbox" role="switch" aria-label={title ? label : undefined} checked={checked} onChange={(e) => onChange(e.target.checked)} />
        {title ? <div className="lead-title">{title}</div> : <label htmlFor={id}>{label}</label>}
        {accessory}
      </div>
      {children && <div className={checked ? "form-grid" : "form-grid off"}>{children}</div>}
    </div>
  );
}

/** A label and a control: one line of a LeadSwitch block. */
export function FormRow({ label, htmlFor, hint, children }: { label: string; htmlFor?: string; hint?: ReactNode; children?: ReactNode }) {
  return (
    <>
      {htmlFor ? (
        <label className="form-grid-label" htmlFor={htmlFor}>
          {label}
        </label>
      ) : (
        <div className="form-grid-label">{label}</div>
      )}
      <div className="form-grid-control">
        {children}
        {hint && <span className="form-grid-hint">{hint}</span>}
      </div>
    </>
  );
}

export function FormText({ label, value, placeholder, hint, onCommit }: { label: string; value: string; placeholder?: string; hint?: ReactNode; onCommit: (v: string) => void }) {
  const id = useId();
  return (
    <FormRow label={label} htmlFor={id} hint={hint}>
      <BufferedInput id={id} className="input" placeholder={placeholder} value={value} onCommit={onCommit} />
    </FormRow>
  );
}

/** A switch with its text to the right, e.g. as the control of a `FormRow`. */
export function InlineSwitch({ label, text, checked, onChange }: { label: string; text: ReactNode; checked: boolean; onChange: (on: boolean) => void }) {
  return (
    <label className="inline-switch">
      <input className="switch" type="checkbox" role="switch" aria-label={label} checked={checked} onChange={(e) => onChange(e.target.checked)} />
      <span className="o2">{text}</span>
    </label>
  );
}

/** A checkbox with its label to the right; `children` follow the label. */
export function Checkbox({ label, checked, disabled, onChange, children }: { label: string; checked: boolean; disabled?: boolean; onChange: (on: boolean) => void; children?: ReactNode }) {
  return (
    <label className="check">
      <input type="checkbox" checked={checked} disabled={disabled} onChange={(e) => onChange(e.target.checked)} />
      {label}
      {children}
    </label>
  );
}

export function Switch({ label, description, checked, onChange, nested }: { label: string; description?: ReactNode; checked: boolean; onChange: (v: boolean) => void; nested?: boolean }) {
  const id = useId();
  return (
    <Row label={label} description={description} htmlFor={id} nested={nested}>
      <input id={id} className="switch" type="checkbox" role="switch" checked={checked} onChange={(e) => onChange(e.target.checked)} />
    </Row>
  );
}

export function Segmented<T extends string>({ label, description, nested, ...buttons }: SegmentedControlProps<T> & { description?: ReactNode; nested?: boolean }) {
  return (
    <Row label={label} description={description} nested={nested} disabled={buttons.disabled}>
      <SegmentedControl label={label} {...buttons} />
    </Row>
  );
}

type BufferedInputProps = Omit<InputHTMLAttributes<HTMLInputElement>, "value" | "type" | "min" | "max" | "required"> & {
  value: string;
  onCommit: (v: string) => void;
  type?: "text" | "number";
  min?: number;
  max?: number;
  /** An empty value isn't saved; the field shows the saved one again. */
  required?: boolean;
};

/** Input that commits on blur or Enter, so half-typed values never reach the backend.
 *  Number spinner steps (input without typing) also commit, debounced, so they preview right away. */
export function BufferedInput({ value, onCommit, type = "text", min, max, required, ...input }: BufferedInputProps) {
  const [text, setText] = useState(value);
  const stepTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  useEffect(() => setText(value), [value]);
  useEffect(() => () => clearTimeout(stepTimer.current), []);

  const isValid = (draft: string) => {
    if (required && !draft.trim()) return false;
    if (type !== "number") return true;
    const n = Number(draft);
    return draft.trim() !== "" && n >= (min ?? -Infinity) && n <= (max ?? Infinity);
  };
  const commit = (draft: string) => {
    clearTimeout(stepTimer.current);
    if (draft === value) return;
    if (isValid(draft)) onCommit(draft);
    else setText(value);
  };

  return (
    <input
      {...input}
      type={type}
      min={min}
      max={max}
      value={text}
      onChange={(e) => {
        const next = e.target.value;
        setText(next);
        // Typing reports an inputType; spinner clicks and arrow-key steps do not.
        const typed = !!(e.nativeEvent as InputEvent).inputType;
        if (type !== "number" || typed || !isValid(next)) return;
        clearTimeout(stepTimer.current);
        stepTimer.current = setTimeout(() => commit(next), 300);
      }}
      onBlur={() => commit(text)}
      onKeyDown={(e) => e.key === "Enter" && commit(text)}
    />
  );
}

export function TextField({ label, description, value, onCommit, type = "text", min, max, suffix, placeholder, nested, required, wide }: {
  label: string;
  description?: ReactNode;
  value: string;
  onCommit: (v: string) => void;
  type?: "text" | "number";
  min?: number;
  max?: number;
  suffix?: string;
  placeholder?: string;
  nested?: boolean;
  required?: boolean;
  /** Room for a URL. */
  wide?: boolean;
}) {
  const id = useId();
  return (
    <Row label={label} description={description} htmlFor={id} nested={nested}>
      <div className="settings-input">
        <BufferedInput
          id={id}
          className={["input", type === "number" && "number", wide && "wide"].filter(Boolean).join(" ")}
          type={type}
          min={min}
          max={max}
          required={required}
          placeholder={placeholder}
          value={value}
          onCommit={onCommit}
        />
        {suffix && <span className="muted">{suffix}</span>}
      </div>
    </Row>
  );
}

export function NumberField({ value, onCommit, ...rest }: {
  label: string;
  description?: ReactNode;
  value: number;
  onCommit: (v: number) => void;
  min: number;
  max: number;
  suffix?: string;
  nested?: boolean;
}) {
  return (
    <TextField
      {...rest}
      type="number"
      value={String(value)}
      onCommit={(t) => {
        const n = Number(t);
        if (t.trim() !== "" && Number.isFinite(n)) onCommit(n);
      }}
    />
  );
}
