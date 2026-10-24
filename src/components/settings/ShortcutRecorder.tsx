import { useEffect, useState, type KeyboardEvent } from "react";
import { api } from "../../api";

interface Props {
  value: string | null;
  /** The shortcut a key press records, in the field's notation; `null` keeps listening. */
  parse: (e: KeyboardEvent<HTMLButtonElement>) => string | null;
  /** How a recorded shortcut reads, e.g. `⌘⇧K`. */
  label: (shortcut: string) => string;
  onChange: (shortcut: string | null) => void;
  /** Names the field for assistive tech, e.g. the command's title. */
  name?: string;
}

/** A click records the next key press: ⌫ clears the shortcut, Esc cancels. */
export function ShortcutRecorder({ value, parse, label, onChange, name }: Props) {
  const [recording, setRecording] = useState(false);
  // Global shortcuts and the menu's keys would take the keys being recorded.
  useEffect(() => {
    if (!recording) return;
    void api.shortcutsPause(true).catch(() => {});
    return () => void api.shortcutsPause(false).catch(() => {});
  }, [recording]);
  const action = recording ? "Press the new shortcut" : "Record shortcut";
  return (
    <button
      type="button"
      data-recording={recording || undefined}
      className={`shortcut-field${recording ? " recording" : ""}${value === null && !recording ? " unbound" : ""}`}
      aria-label={name ? `${action} for ${name}` : action}
      onClick={(e) => {
        e.currentTarget.focus();
        setRecording(true);
      }}
      onBlur={() => setRecording(false)}
      onKeyDown={(e) => {
        if (!recording) return;
        e.preventDefault();
        e.stopPropagation();
        if (e.key === "Escape") return setRecording(false);
        if (e.key === "Backspace" || e.key === "Delete") {
          onChange(null);
          return setRecording(false);
        }
        const shortcut = parse(e);
        if (!shortcut) return;
        onChange(shortcut);
        setRecording(false);
      }}
    >
      {recording ? "Press keys…" : value ? label(value) : "unbound"}
    </button>
  );
}
