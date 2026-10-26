import { useEffect, useRef } from "react";
import { api } from "../../api";
import { applyAppearance, useSystemDark } from "../../lib/theme";
import { MarkdownEditor, type MarkdownEditorHandle } from "./MarkdownEditor";
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

/** The editor window's page. Esc saves and closes. */
export function QuickNotesApp() {
  useAppearance();
  const { note, session, draft, status, edit, loadNote, close } = useQuickNote();
  const editor = useRef<MarkdownEditorHandle>(null);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.isComposing) return;
      e.preventDefault();
      close();
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [close]);

  return (
    <div className="quick-notes">
      <header className="quick-notes-head" data-tauri-drag-region>
        <span className="o3 mono">{"#".repeat(note?.level ?? 3)}</span>
        <input
          aria-label="Topic title"
          placeholder="Topic title"
          value={draft.title}
          disabled={!note}
          onChange={(e) => edit({ title: e.target.value })}
          onKeyDown={(e) => {
            if (e.key !== "Enter") return;
            e.preventDefault();
            editor.current?.focus();
          }}
        />
      </header>
      {note ? (
        <MarkdownEditor ref={editor} documentKey={String(session)} value={draft.body} onChange={(body) => edit({ body })} />
      ) : (
        <p className="quick-notes-empty o3">Pick a topic with Quick Notes (⌘F3, the menu bar or the palette).</p>
      )}
      <footer className="quick-notes-foot">{note && <StatusLine status={status} onLoadNote={() => void loadNote()} />}</footer>
    </div>
  );
}
