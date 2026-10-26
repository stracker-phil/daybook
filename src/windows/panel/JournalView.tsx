import { useEffect, useState } from "react";
import { api, toAppError, type JournalGroupView, type JournalInfo } from "../../api";
import { SegmentedControl } from "../../components/SegmentedControl";
import { JournalForm } from "./JournalForm";

/** Read on open and again whenever the note or Settings change, so answers typed in Obsidian show. */
function useJournalInfo(): { info: JournalInfo | null; error: string | null } {
  const [info, setInfo] = useState<JournalInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const load = () => void api.journalInfo().then(setInfo, (e) => setError(toAppError(e).message));
    load();
    const subscriptions = [api.onNoteChanged(load), api.onSettingsChanged(load)];
    return () => subscriptions.forEach((subscription) => void subscription.then((unlisten) => unlisten()).catch(() => {}));
  }, []);
  return { info, error };
}

/** The group asked for, else the first with open questions, else the first. */
function pick(groups: JournalGroupView[], asked: string | null): JournalGroupView | undefined {
  return groups.find((g) => g.title === asked) ?? groups.find((g) => g.answered < g.total) ?? groups[0];
}

function Unreadable({ message }: { message: string }) {
  return (
    <div className="panel-card journal-view">
      <div className="palette-foot palette-error">{message}</div>
    </div>
  );
}

function Footer({ error, groups, noteExists }: { error: string | null; groups: number; noteExists: boolean }) {
  if (error) return <span className="palette-error">{error}</span>;
  return (
    <>
      {groups > 1 && <span>⌘1–{groups} group</span>}
      <span>esc close</span>
      {!noteExists && <span className="journal-waiting">Saved once today's note exists</span>}
    </>
  );
}

/** The journal, one group at a time; ⌘1…⌘n switch groups and answers are saved as you type. */
export function JournalView({ group: asked }: { group: string | null }) {
  const { info, error: unreadable } = useJournalInfo();
  const [selected, setSelected] = useState(asked);
  const [error, setError] = useState<string | null>(null);
  const groups = info?.groups;
  const active = pick(groups ?? [], selected);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const digit = /^Digit([1-9])$/.exec(e.code);
      const group = e.metaKey && digit ? groups?.[Number(digit[1]) - 1] : undefined;
      if (!group) return;
      e.preventDefault();
      setSelected(group.title);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [groups]);

  if (!info || !groups) return unreadable ? <Unreadable message={unreadable} /> : null;

  return (
    <div className="panel-card journal-view">
      <div className="panel-heading journal-heading">
        <span className="o2">Journal</span>
        {groups.length > 1 && active && (
          <SegmentedControl label="Group" options={groups.map((g) => g.title)} value={active.title} labels={Object.fromEntries(groups.map((g) => [g.title, g.title]))} onChange={setSelected} />
        )}
      </div>
      <div className="journal-body">
        {active ? <JournalForm key={active.title} group={active} onError={setError} /> : <p className="o3">No journal questions yet. Add them in Settings → Journal.</p>}
      </div>
      <div className="palette-foot o3">
        <Footer error={error} groups={groups.length} noteExists={info.noteExists} />
      </div>
    </div>
  );
}
