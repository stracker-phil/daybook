import { useState } from "react";
import { FocusEditor } from "./FocusEditor";
import { ESCAPE_HINT, PanelFooter, PanelPrompt } from "./parts";
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
      <PanelPrompt className="focus-heading">Done for today · end-of-day list</PanelPrompt>
      {info && <FocusEditor value={value} projects={info.projects} onChange={setDraft} onConfirm={() => onContinue(value.trim() ? value : null)} onSkip={() => onContinue(null)} />}
      <PanelFooter
        hints={[{ keys: ["⌘", "↵"], label: "Continue" }, { keys: ["⇧", "⌘", "↵"], label: "Without list" }, ESCAPE_HINT]}
        error={problem}
      />
    </div>
  );
}
