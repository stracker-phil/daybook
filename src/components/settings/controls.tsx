/** Shared controls for settings pages: grouped cards and rows that apply their change right away. */
import { useEffect, useId, useRef, useState, type ReactNode } from "react";
import { SegmentedControl, type SegmentedControlProps } from "../SegmentedControl";

export function Group({ title, description, children }: { title?: string; description?: ReactNode; children: ReactNode }) {
  return (
    <section className="settings-group">
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
  children?: ReactNode;
}

/** One card row: label and description on the left, the control on the right. */
export function Row({ label, icon, description, htmlFor, stacked, nested, children }: RowProps) {
  const className = ["settings-row", stacked && "stacked", nested && "nested"].filter(Boolean).join(" ");
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
    <Row label={label} description={description} nested={nested}>
      <SegmentedControl label={label} {...buttons} />
    </Row>
  );
}

/** Text input that commits on blur or Enter, so half-typed values never reach the backend.
 *  Number spinner steps (input without typing) also commit, debounced, so they preview right away. */
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
  /** An empty value isn't saved; the field shows the saved one again. */
  required?: boolean;
  /** Room for a URL. */
  wide?: boolean;
}) {
  const id = useId();
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
    <Row label={label} description={description} htmlFor={id} nested={nested}>
      <div className="settings-input">
        <input
          id={id}
          className={["input", type === "number" && "number", wide && "wide"].filter(Boolean).join(" ")}
          type={type}
          min={min}
          max={max}
          placeholder={placeholder}
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
