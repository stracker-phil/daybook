import { useEffect, useLayoutEffect, useRef, type KeyboardEvent } from "react";
import type { FocusProject } from "../../api";
import { continueList, cycleCheckbox, indentLine, setLineProject, type TextEdit } from "./focusText";

const MIN_ROWS = 4;
const MAX_ROWS = 14;
const PROJECT_KEYS = 9;

/** The edit a key makes to the caret's line; `null` leaves the key to the field. */
function keyEdit(e: KeyboardEvent<HTMLTextAreaElement>, value: string, projects: FocusProject[]): TextEdit | null {
  const at = e.currentTarget.selectionStart;
  if (e.key === "Enter" && !e.metaKey && !e.shiftKey && at === e.currentTarget.selectionEnd) return continueList(value, at);
  if (e.key === "Tab" && !e.metaKey) return indentLine(value, at, e.shiftKey);
  if (!e.metaKey || e.shiftKey) return null;
  if (e.key.toLowerCase() === "l") return cycleCheckbox(value, at);
  const digit = /^Digit([1-9])$/.exec(e.code);
  const project = digit && projects[Number(digit[1]) - 1];
  return project ? setLineProject(value, at, project.emoji) : null;
}

/** The focus list as plain text, saved as typed; Tab never leaves the field. */
export function FocusEditor({ value, projects, onChange, onConfirm, onSkip, onSend }: {
  value: string;
  projects: FocusProject[];
  onChange: (value: string) => void;
  onConfirm: () => void;
  /** ⇧⌘↵, the view's second way to confirm (the end-of-day step: without the list). */
  onSkip?: () => void;
  onSend?: () => void;
}) {
  const field = useRef<HTMLTextAreaElement>(null);
  const caret = useRef<number | null>(null);
  const shown = projects.slice(0, PROJECT_KEYS);

  useEffect(() => {
    const element = field.current;
    element?.focus();
    element?.setSelectionRange(element.value.length, element.value.length);
  }, []);

  // A shortcut's edit puts the caret where the edit left it.
  useLayoutEffect(() => {
    if (caret.current === null) return;
    field.current?.setSelectionRange(caret.current, caret.current);
    caret.current = null;
  });

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.nativeEvent.isComposing || e.ctrlKey || e.altKey) return;
    if (e.key === "Enter" && e.metaKey) {
      e.preventDefault();
      if (e.shiftKey && onSkip) onSkip();
      else onConfirm();
      return;
    }
    if (e.metaKey && !e.shiftKey && e.key.toLowerCase() === "s") {
      e.preventDefault();
      onSend?.();
      return;
    }
    const edit = keyEdit(e, value, shown);
    if (!edit) return;
    e.preventDefault();
    caret.current = edit.caret;
    onChange(edit.text);
  };

  return (
    <div className="focus-editor">
      <textarea
        ref={field}
        aria-label="Focus list"
        spellCheck={false}
        rows={Math.min(MAX_ROWS, Math.max(MIN_ROWS, value.split("\n").length + 1))}
        value={value}
        placeholder="- `[ ]` What you plan to do today"
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={onKeyDown}
      />
      {shown.length > 0 && (
        <div className="project-keys o3" aria-label="Projects">
          {shown.map((project, i) => (
            <span key={project.name}>
              ⌘{i + 1} {project.name}
            </span>
          ))}
        </div>
      )}
    </div>
  );
}
