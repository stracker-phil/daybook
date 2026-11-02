import { useCallback, useEffect, useRef, useState } from "react";
import { api, type NoteSectionRef, type QuickNote } from "../../api";
import { useAutosave } from "../../lib/useAutosave";

const SAVE_DELAY_MS = 800;

interface Draft {
  title: string;
  body: string;
}

export type SaveStatus =
  | { kind: "saved" }
  | { kind: "pending" }
  | { kind: "error"; message: string }
  /** The note holds another text for the topic than the editor started from; the draft isn't saved. */
  | { kind: "changed" };

/**
 * The editor's topic: loaded when Rust picks one, saved a moment after typing stops and when the window
 * loses focus. While nothing is unsaved, a change made in the note shows at once, in place; with unsaved
 * text the save is refused instead (`changed`), and `loadNote` takes the note's text after copying the draft.
 */
export function useQuickNote() {
  const [note, setNote] = useState<QuickNote | null>(null);
  /** Changes with every loaded topic, so the editor starts fresh with its own undo history. */
  const [session, setSession] = useState(0);
  const [draft, setDraft] = useState<Draft>({ title: "", body: "" });
  const [status, setStatus] = useState<SaveStatus>({ kind: "saved" });
  /** The heading as the note has it now; a new title renames it. */
  const topic = useRef<NoteSectionRef | null>(null);
  /** The topic's text as the note had it when loaded or last saved. */
  const base = useRef("");
  const latest = useRef(draft);
  /** Counts edits, so a reload that finishes after typing is dropped. */
  const edits = useRef(0);
  const refusedNote = useRef<QuickNote | null>(null);

  /** Takes the note's text into the open editor, keeping its cursor and undo history. */
  const adopt = useCallback((next: QuickNote) => {
    topic.current = { title: next.title, level: next.level };
    base.current = next.body;
    latest.current = { title: next.title, body: next.body };
    refusedNote.current = null;
    setNote(next);
    setDraft(latest.current);
    setStatus({ kind: "saved" });
  }, []);

  /** Starts a fresh editor on the note's text. */
  const show = useCallback(
    (next: QuickNote) => {
      adopt(next);
      setSession((value) => value + 1);
    },
    [adopt],
  );

  const autosave = useAutosave<Draft>({
    delayMs: SAVE_DELAY_MS,
    save: async (next) => {
      if (!topic.current) return;
      const result = await api.quickNoteSave(topic.current, next.title, next.body, base.current);
      if (result.kind === "changed") {
        refusedNote.current = result.note;
        setStatus({ kind: "changed" });
        return;
      }
      topic.current = result.topic;
      base.current = result.body;
      if (!autosave.isDirty()) setStatus({ kind: "saved" });
    },
    onError: (message) => setStatus({ kind: "error", message }),
  });
  const { change, flush, reset, isDirty } = autosave;

  const load = useCallback(async () => {
    const next = await api.quickNoteGet();
    reset();
    if (next) show(next);
  }, [reset, show]);

  /** Shows the note's text unless there is unsaved or refused text, or typing started meanwhile. */
  const reload = useCallback(async () => {
    if (isDirty() || refusedNote.current || !topic.current) return;
    await flush();
    const before = edits.current;
    const next = await api.quickNoteGet().catch(() => null);
    if (!next || isDirty() || refusedNote.current || edits.current !== before) return;
    // The draft may differ from what was saved by whitespace the note drops; that is no change.
    const synced = topic.current;
    if (synced && next.body === base.current && next.title === synced.title && next.level === synced.level) return;
    adopt(next);
  }, [flush, isDirty, adopt]);

  const edit = (patch: Partial<Draft>) => {
    latest.current = { ...latest.current, ...patch };
    edits.current++;
    setDraft(latest.current);
    if (!refusedNote.current) setStatus({ kind: "pending" });
    change(latest.current);
  };

  /** Drops the refused draft for the note's text; the draft goes to the clipboard first. */
  const loadNote = async () => {
    const current = refusedNote.current;
    if (!current) return;
    await navigator.clipboard?.writeText(latest.current.body).catch(() => {});
    reset();
    show(current);
  };

  const close = useCallback(() => void flush().then(() => api.quickNotesClose()), [flush]);

  useEffect(() => {
    void load().catch((e: unknown) => setStatus({ kind: "error", message: String(e) }));
    const subscriptions = [api.onQuickNoteTarget(() => void flush().then(load)), api.onNoteChanged(() => void reload())];
    const onFocus = () => void reload();
    const onBlur = () => void flush();
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    return () => {
      subscriptions.forEach((subscription) => void subscription.then((unlisten) => unlisten()).catch(() => {}));
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }, [load, reload, flush]);

  return { note, session, draft, status, edit, loadNote, close };
}
