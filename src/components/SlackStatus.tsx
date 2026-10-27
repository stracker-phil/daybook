/** How Slack shows a status: the emoji, the text and the presence dot. */
import type { Presence, ShownStatus } from "../api";
import { emojiGlyph, emojiName } from "../lib/slackEmoji";

export function EmojiGlyph({ code }: { code: string }) {
  const name = emojiName(code);
  if (!name) return null;
  const glyph = emojiGlyph(code);
  return glyph ? (
    <span className="slack-emoji" aria-hidden="true">
      {glyph}
    </span>
  ) : (
    <span className="slack-emoji custom" aria-hidden="true">
      :{name}:
    </span>
  );
}

export const PRESENCE_LABELS: Record<Presence, string> = { auto: "Active", away: "Away", keep: "No change" };

/** Filled: active; hollow: away. */
export function PresenceDot({ presence }: { presence: Presence }) {
  return <span className="presence-dot" data-presence={presence} aria-hidden="true" />;
}

/** The emoji and the text, or "No status". */
export function SlackLine({ emoji, text }: { emoji: string; text: string }) {
  return (
    <>
      <EmojiGlyph code={emoji} />
      <span className={text ? "slack-status-text" : "slack-status-text empty"}>{text || "No status"}</span>
    </>
  );
}

/** Read-only: what Slack shows, as Daybook last set it (the sidebar's bottom line). */
export function SlackBadge({ shown }: { shown: ShownStatus }) {
  const shownPresence = shown.presence === "keep" ? null : shown.presence;
  const presence = shownPresence ? PRESENCE_LABELS[shownPresence] : null;
  const description = `Slack status: ${shown.text || "No status"}${presence ? ` · ${presence}` : ""}`;
  return (
    <div className="slack-status badge" role="status" data-stage="hub:slack" aria-label={description} title={description}>
      <SlackLine emoji={shown.emoji} text={shown.text} />
      {shownPresence && <PresenceDot presence={shownPresence} />}
    </div>
  );
}
