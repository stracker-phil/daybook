import { NotebookText } from "lucide-react";
import { api, type HubView } from "../../api";
import { fail } from "../../state/requests";

export function TodaysNotes({ hub }: { hub: HubView }) {
  const title = hub.isToday ? "Today’s notes" : "Notes";
  const open = (heading?: string) => void api.dailyNoteOpen(hub.day, heading).catch((e) => fail(e, "Couldn't open the note"));
  return (
    <section className="board-section" aria-label={title}>
      <header className="board-section-head">
        <h2 className="board-section-title">{title}</h2>
        <button type="button" className="link-btn" onClick={() => open()}>
          Open in Obsidian
        </button>
      </header>
      {hub.notes.length === 0 && <p className="board-empty o3">{hub.isToday ? "No notes yet today." : "No notes."}</p>}
      <div className="note-rows">
        {hub.notes.map((topic) => (
          <button key={`${topic.level}:${topic.title}`} type="button" className="note-row" onClick={() => open(topic.title)}>
            <NotebookText className="o3" size="1.077rem" aria-hidden="true" />
            <span className="grow-text">{topic.title}</span>
          </button>
        ))}
      </div>
    </section>
  );
}
