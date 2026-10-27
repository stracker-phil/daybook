import { useRef, useState } from "react";
import { api, toAppError, type FocusInfo } from "../../api";
import { FocusEditor } from "./FocusEditor";
import { ESCAPE_HINT, PanelFooter, PanelPrompt, type PanelHint } from "./parts";
import { useFocusInfo, useKeepPanelOpen } from "./useFocusInfo";

function slackHint({ slack, channel }: FocusInfo): string {
  switch (slack) {
    case "edit":
      return `Updates your morning message in ${channel}`;
    case "post":
      return `Posts to ${channel} · no morning message yet`;
    case "sent":
      return "End-of-day list sent · today's note only";
    case "none":
      return "Today's note only";
  }
}

/** ⌘S puts the list as typed into the day's Slack message, without writing the note or closing. */
export function FocusView() {
  const { info, error: unreadable } = useFocusInfo();
  const [draft, setDraft] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  /** The list as last sent to Slack while the overlay is open. */
  const [sent, setSent] = useState<string | null>(null);
  const busy = useRef(false);
  useKeepPanelOpen();
  if (!info) {
    return unreadable ? (
      <div className="panel-card focus-view">
        <PanelFooter error={unreadable} hints={[ESCAPE_HINT]} />
      </div>
    ) : null;
  }
  const value = draft ?? info.body;
  const shares = info.slack === "edit" || info.slack === "post";

  const run = async (action: () => Promise<void>) => {
    if (busy.current) return;
    busy.current = true;
    try {
      await action();
    } catch (e) {
      setError(toAppError(e).message);
    } finally {
      busy.current = false;
    }
  };
  const save = () => run(() => api.focusSave(value));
  const send = () =>
    run(async () => {
      await api.focusSend(value);
      setSent(value);
    });

  const hints: PanelHint[] = [
    { keys: ["⌘", "L"], label: "Check" },
    ...(shares ? [{ keys: ["⌘", "S"], label: "Send to Slack" }] : []),
    { keys: ["⌘", "↵"], label: "Save" },
    ESCAPE_HINT,
  ];
  const sentState = sent === null ? null : sent === value ? "Sent to Slack" : "Changed since sending to Slack";

  return (
    <div className="panel-card focus-view">
      <PanelPrompt className="focus-heading">
        <span>Today's focus</span>
        <span className="o3">{slackHint(info)}</span>
      </PanelPrompt>
      <FocusEditor
        value={value}
        projects={info.projects}
        onChange={(next) => {
          setDraft(next);
          setError(null);
        }}
        onConfirm={() => void save()}
        onSend={shares ? () => void send() : undefined}
      />
      <PanelFooter hints={hints} error={error} status={sentState} />
    </div>
  );
}
