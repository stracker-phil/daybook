import { useRef, useState } from "react";
import { api, toAppError, type FocusInfo } from "../../api";
import { FocusEditor } from "./FocusEditor";
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

/** The footer: an error, whether Slack has the list as shown, or the keys. */
function Footer({ error, sent, value, shares }: { error: string | null; sent: string | null; value: string; shares: boolean }) {
  if (error) return <span className="palette-error">{error}</span>;
  if (sent !== null) return <span>{sent === value ? "Sent to Slack" : "Changed since sending to Slack"}</span>;
  const keys = ["⌘L check", ...(shares ? ["⌘S send to Slack"] : []), "⌘↵ save", "esc close"];
  return (
    <>
      {keys.map((key) => (
        <span key={key}>{key}</span>
      ))}
    </>
  );
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
  if (!info) return unreadable ? <div className="panel-card focus-view"><div className="palette-foot palette-error">{unreadable}</div></div> : null;
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

  return (
    <div className="panel-card focus-view">
      <div className="panel-heading focus-heading">
        <span className="o2">Today's focus</span>
        <span className="o3">{slackHint(info)}</span>
      </div>
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
      <div className="palette-foot o3">
        <Footer error={error} sent={sent} value={value} shares={shares} />
      </div>
    </div>
  );
}
