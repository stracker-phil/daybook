import { useState } from "react";
import { FocusEditor } from "./FocusEditor";
import { useFocusInfo, useKeepPanelOpen } from "./useFocusInfo";

/** Before going Off: the morning's list to tick, reword and extend; it goes out with the final answer. */
export function EndOfDayListStep({ error, onContinue }: { error: string | null; onContinue: (list: string | null) => void }) {
  const { info, error: unreadable } = useFocusInfo();
  const [draft, setDraft] = useState<string | null>(null);
  useKeepPanelOpen();
  const problem = error ?? unreadable;
  const value = draft ?? info?.body ?? "";
  return (
    <div className="panel-card focus-view">
      <div className="panel-heading o2">Done for today · end-of-day list</div>
      {info && <FocusEditor value={value} projects={info.projects} onChange={setDraft} onConfirm={() => onContinue(value.trim() ? value : null)} onSkip={() => onContinue(null)} />}
      <div className="palette-foot o3">
        {problem ? (
          <span className="palette-error">{problem}</span>
        ) : (
          <>
            <span>⌘↵ continue</span>
            <span>⇧⌘↵ without list</span>
            <span>esc cancel</span>
          </>
        )}
      </div>
    </div>
  );
}
