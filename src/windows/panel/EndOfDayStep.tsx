import { useEffect, useRef, type KeyboardEvent } from "react";
import { ExternalLink } from "lucide-react";
import { api, type EndOfDayAnswer, type EndOfDayCheck } from "../../api";
import { PanelFooter, PanelKey, PanelPrompt } from "./parts";

const ANSWER_KEYS: Record<string, EndOfDayAnswer> = { y: "done", n: "incomplete" };

/** Before going Off; the answer picks the channel message. The overlay stays open while the owner checks the link. */
export function EndOfDayStep({ check, message, error, onAnswer }: { check: EndOfDayCheck; message: string; error: string | null; onAnswer: (answer: EndOfDayAnswer) => void }) {
  const yesButton = useRef<HTMLButtonElement>(null);
  useEffect(() => {
    void api.panelKeepOpen(true).catch(() => {});
    yesButton.current?.focus();
  }, []);

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.metaKey || e.ctrlKey || e.altKey) return;
    const answer = ANSWER_KEYS[e.key.toLowerCase()];
    if (!answer) return;
    e.preventDefault();
    onAnswer(answer);
  };

  return (
    <div className="panel-card end-of-day" onKeyDown={onKeyDown}>
      <PanelPrompt>Done for today{message ? ` · ${message}` : ""}</PanelPrompt>
      <div className="panel-body-text">
        {check.reminder && (
          <p className="numbered">
            <span className="o3">1.</span>
            {check.reminder}
          </p>
        )}
        <div className="numbered">
          {check.reminder && <span className="o3">2.</span>}
          <span className="grow-text">{check.question}</span>
          {check.url && (
            <button type="button" className="btn btn-quiet" onClick={() => void api.openUrl(check.url).catch(() => {})}>
              <ExternalLink size={16} aria-hidden="true" />
              Open
            </button>
          )}
        </div>
        <div className="panel-buttons">
          <button ref={yesButton} type="button" className="btn btn-primary" onClick={() => onAnswer("done")}>
            Yes <PanelKey>Y</PanelKey>
          </button>
          <button type="button" className="btn" onClick={() => onAnswer("incomplete")}>
            Not yet <PanelKey>N</PanelKey>
          </button>
        </div>
      </div>
      <PanelFooter hints={[{ keys: ["esc"], label: "Cancel, send nothing" }]} error={error} />
    </div>
  );
}
