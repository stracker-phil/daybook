import { useEffect, useRef } from "react";
import { api } from "../../api";
import { handleKeydown, setQuickNoteLoaded } from "../../commands";
import { applyAppearance, useSystemDark } from "../../lib/theme";
import { useStore } from "../../state/store";
import { MarkdownEditor, type MarkdownEditorHandle } from "./MarkdownEditor";
import { usePresentationFont, usePresenting } from "./presentation";
import { useQuickNote, type SaveStatus } from "./useQuickNote";

function useAppearance() {
  const dark = useSystemDark();
  useEffect(() => {
    void api.getSettings().then((settings) => applyAppearance(settings, dark), () => {});
    const unlisten = api.onSettingsChanged((settings) => applyAppearance(settings, dark));
    return () => void unlisten.then((fn) => fn()).catch(() => {});
  }, [dark]);
}

const STATUS_TEXT: Record<Exclude<SaveStatus["kind"], "error">, string> = {
  saved: "Saved to today’s note",
  pending: "Saving…",
  changed: "Changed in the note meanwhile: your text isn’t saved.",
};

function StatusLine({ status, onLoadNote }: { status: SaveStatus; onLoadNote: () => void }) {
  if (status.kind === "error") return <span className="grow-text palette-error">Not saved: {status.message}</span>;
  return (
    <>
      <span className="grow-text o3">{STATUS_TEXT[status.kind]}</span>
      {status.kind === "changed" && (
        <button type="button" className="link-btn" title="Copies your text, then shows the note's" onClick={onLoadNote}>
          Load the note
        </button>
      )}
    </>
  );
}

/** Loads the user's keybindings and follows their changes. */
function useKeybindings() {
  useEffect(() => {
    const { applyKeybindings } = useStore.getState();
    void api.getKeybindings().then(applyKeybindings, () => {});
    const unlisten = api.onKeybindingsChanged(applyKeybindings);
    return () => void unlisten.then((fn) => fn()).catch(() => {});
  }, []);
}

/** The page's keys; Esc ends presentation first, else saves and closes. */
function useKeys(presenting: boolean, close: () => void) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") {
        handleKeydown(e);
        return;
      }
      if (e.isComposing) return;
      e.preventDefault();
      e.stopImmediatePropagation();
      if (presenting) void api.runCommand("notes:endPresentation");
      else close();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [presenting, close]);
}

function QuickNotesHead({
  level,
  title,
  disabled,
  onTitle,
  onEnter,
}: {
  level: number;
  title: string;
  disabled: boolean;
  onTitle: (title: string) => void;
  onEnter: () => void;
}) {
  return (
    <header className="quick-notes-head" data-tauri-drag-region>
      <span className="o3 mono">{"#".repeat(level)}</span>
      <input
        aria-label="Topic title"
        placeholder="Topic title"
        value={title}
        disabled={disabled}
        onChange={(e) => onTitle(e.target.value)}
        onKeyDown={(e) => {
          if (e.key !== "Enter") return;
          e.preventDefault();
          onEnter();
        }}
      />
    </header>
  );
}

const showsWhilePresenting = (status: SaveStatus) => status.kind === "error" || status.kind === "changed";

/** The editor window's page; while presenting only the text shows, fitted to the screen. */
export function QuickNotesApp() {
  useAppearance();
  const { note, session, draft, status, edit, loadNote, close } = useQuickNote();
  const editor = useRef<MarkdownEditorHandle>(null);
  const presenting = usePresenting();
  const { box, mirror } = usePresentationFont({ text: draft.body, active: presenting && !!note, onFit: () => editor.current?.requestMeasure() });
  useKeybindings();
  useKeys(presenting, close);

  useEffect(() => setQuickNoteLoaded(note !== null), [note]);

  return (
    <div className="quick-notes" data-presenting={presenting || undefined}>
      {!presenting && (
        <QuickNotesHead
          level={note?.level ?? 3}
          title={draft.title}
          disabled={!note}
          onTitle={(title) => edit({ title })}
          onEnter={() => editor.current?.focus()}
        />
      )}
      {note ? (
        <div ref={box} className="quick-notes-body">
          <MarkdownEditor ref={editor} documentKey={String(session)} value={draft.body} onChange={(body) => edit({ body })} />
          {presenting && <div ref={mirror} className="quick-notes-mirror" aria-hidden="true" />}
        </div>
      ) : (
        <p className="quick-notes-empty o3">Pick a topic with Quick Notes (⌘F3, the menu bar or the palette).</p>
      )}
      {(!presenting || showsWhilePresenting(status)) && (
        <footer className="quick-notes-foot">{note && <StatusLine status={status} onLoadNote={() => void loadNote()} />}</footer>
      )}
    </div>
  );
}
