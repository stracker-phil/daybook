import { useState } from "react";
import type { Presence, SlackStatus } from "../../api";
import { asShortcode } from "../../lib/slackEmoji";
import { PRESENCE_LABELS, PresenceDot, SlackLine } from "../SlackStatus";
import { BufferedInput, Checkbox } from "./controls";

const PRESENCES: Presence[] = ["auto", "away", "keep"];

/** The status as Slack draws it: emoji, text, then the presence after a divider. */
function SlackStatusBar({ value }: { value: SlackStatus }) {
  return (
    <>
      {value.keepStatus ? (
        <span className="slack-status-text empty">Status unchanged</span>
      ) : (
        <SlackLine emoji={value.emoji} text={value.text} />
      )}
      {value.presence !== "keep" && (
        <span className="slack-presence">
          <PresenceDot presence={value.presence} />
          {PRESENCE_LABELS[value.presence]}
        </span>
      )}
    </>
  );
}

/** The Slack-like bar; clicking it opens emoji, text and presence on one line and Keep the status below. */
export function SlackStatusEditor({ value, onChange }: { value: SlackStatus; onChange: (next: SlackStatus) => void }) {
  const [open, setOpen] = useState(false);
  const change = (patch: Partial<SlackStatus>) => onChange({ ...value, ...patch });
  const kept = value.keepStatus;
  return (
    <div className="slack-status-editor">
      <button type="button" className="slack-status badge" aria-expanded={open} aria-label="Edit Slack status" onClick={() => setOpen(!open)}>
        <SlackStatusBar value={value} />
      </button>
      {open && (
        <>
          <div className="slack-status-line">
            <BufferedInput className="input slack-status-emoji" aria-label="Emoji" placeholder=":emoji:" disabled={kept} value={value.emoji} onCommit={(emoji) => change({ emoji: asShortcode(emoji) })} />
            <BufferedInput className="input slack-status-text-input" aria-label="Status text" placeholder="What's your status?" disabled={kept} value={value.text} onCommit={(text) => change({ text })} />
            <select className="input" aria-label="Presence" value={value.presence} onChange={(e) => change({ presence: e.target.value as Presence })}>
              {PRESENCES.map((presence) => (
                <option key={presence} value={presence}>
                  {PRESENCE_LABELS[presence]}
                </option>
              ))}
            </select>
          </div>
          <Checkbox label="Keep the status" checked={kept} onChange={(keepStatus) => change({ keepStatus })} />
        </>
      )}
    </div>
  );
}
