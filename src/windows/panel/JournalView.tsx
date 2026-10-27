import { useEffect, useState } from "react";
import { api, toAppError, type JournalGroupView, type JournalInfo } from "../../api";
import { JournalForm } from "./JournalForm";
import { CLOSE_HINT, PanelFooter, PanelPrompt, PanelSegmented, type PanelHint } from "./parts";

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
      <PanelFooter error={message} hints={[CLOSE_HINT]} />
    </div>
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

  const hints: PanelHint[] = groups.length > 1 ? [{ keys: ["⌘", `1–${groups.length}`], label: "Group" }, CLOSE_HINT] : [CLOSE_HINT];
  return (
    <div className="panel-card journal-view">
      <PanelPrompt className="journal-heading">
        <span>Journal</span>
        {groups.length > 1 && active && (
          <PanelSegmented label="Group" segments={groups.map((g) => ({ id: g.title, label: g.title }))} value={active.title} onPick={setSelected} />
        )}
      </PanelPrompt>
      <div className="journal-body">
        {active ? <JournalForm key={active.title} group={active} onError={setError} /> : <p className="o3">No journal questions yet. Add them in Settings → Journal.</p>}
      </div>
      <PanelFooter hints={hints} error={error} status={info.noteExists ? null : "Saved once today's note exists"} />
    </div>
  );
}
