import { useState } from "react";
import type { Presence, SlackStatus } from "../../api";
import { asShortcode } from "../../lib/slackEmoji";
import { PRESENCE_LABELS, SlackStatusPreview } from "../SlackStatus";
import { Row, Segmented, Switch, TextField } from "./controls";

const PRESENCES: Presence[] = ["auto", "away", "keep"];

const LABEL = "Slack status";

/** A preview row whose Edit… opens the emoji, text and presence below it. */
export function SlackStatusField({ description, value, onChange, nested }: {
  /** e.g. which placeholders the text takes. */
  description?: string;
  value: SlackStatus;
  onChange: (next: SlackStatus) => void;
  nested?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const change = (patch: Partial<SlackStatus>) => onChange({ ...value, ...patch });
  return (
    <>
      <Row label={LABEL} description={open ? description : undefined} nested={nested}>
        <SlackStatusPreview status={value} />
        <button type="button" className="btn" aria-expanded={open} aria-label={`${open ? "Done Editing" : "Edit"} ${LABEL}`} onClick={() => setOpen(!open)}>
          {open ? "Done" : "Edit…"}
        </button>
      </Row>
      {open && (
        <>
          <Switch nested label="Keep the status" description="Slack's emoji and text stay as they are." checked={value.keepStatus} onChange={(keepStatus) => change({ keepStatus })} />
          {!value.keepStatus && (
            <>
              <TextField nested label="Emoji" placeholder=":emoji:" value={value.emoji} onCommit={(emoji) => change({ emoji: asShortcode(emoji) })} />
              <TextField nested label="Text" placeholder="What's your status?" value={value.text} onCommit={(text) => change({ text })} />
            </>
          )}
          <Segmented nested label="Presence" options={PRESENCES} labels={PRESENCE_LABELS} value={value.presence} onChange={(presence) => change({ presence })} />
        </>
      )}
    </>
  );
}
