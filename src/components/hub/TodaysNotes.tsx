import { NotebookText, PenLine } from "lucide-react";
import { api, type HubView, type NoteSectionRef } from "../../api";
import { fail } from "../../state/requests";

/** Today's topics open in the quick-notes editor; another day's open in Obsidian, as the editor only writes today. */
export function TodaysNotes({ hub }: { hub: HubView }) {
  const title = hub.isToday ? "Today’s notes" : "Notes";
  const openNote = (heading?: string) => void api.dailyNoteOpen(hub.day, heading).catch((e) => fail(e, "Couldn't open the note"));
  const writeNote = () => void api.runCommand("notes:quick").catch((e) => fail(e, "Couldn't open quick notes"));
  const editTopic = (topic: NoteSectionRef) => void api.quickNotesOpen(topic, false).catch((e) => fail(e, "Couldn't open quick notes"));
  const openTopic = (topic: NoteSectionRef) => (hub.isToday ? editTopic(topic) : openNote(topic.title));
  return (
    <section className="board-section" aria-label={title}>
      <header className="board-section-head">
        <h2 className="board-section-title">{title}</h2>
        <button type="button" className="link-btn" onClick={() => openNote()}>
          Open in Obsidian
        </button>
      </header>
      {hub.notes.length === 0 && !hub.isToday && <p className="board-empty o3">No notes.</p>}
      <div className="note-rows">
        {hub.notes.length === 0 && hub.isToday && (
          <button type="button" className="note-row" onClick={writeNote}>
            <PenLine className="o3" size="1.077rem" aria-hidden="true" />
            <span className="grow-text">Write a note…</span>
          </button>
        )}
        {hub.notes.map((topic) => (
          <button key={`${topic.level}:${topic.title}`} type="button" className="note-row" onClick={() => openTopic(topic)}>
            <NotebookText className="o3" size="1.077rem" aria-hidden="true" />
            <span className="grow-text">{topic.title}</span>
          </button>
        ))}
      </div>
    </section>
  );
}
