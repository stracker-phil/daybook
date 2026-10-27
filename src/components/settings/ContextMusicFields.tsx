import type { Context, ContextMusic, MusicEntry, OnCall } from "../../api";
import { entryLabel } from "../../lib/music";
import { Row, Switch } from "./controls";
import { CommitInput } from "./fields";

const ON_CALL: { value: OnCall; label: string }[] = [
  { value: "nothing", label: "Keep playing" },
  { value: "pause", label: "Pause" },
  { value: "lower", label: "Lower the volume" },
];

/** What Daybook does with the music by itself while this context is active; Break and the workday need the Status feature. */
export function ContextMusicFields({ context, entries, onChange }: { context: Context; entries: MusicEntry[]; onChange: (patch: Partial<Context>) => void }) {
  const music = context.music;
  const patch = (changes: Partial<ContextMusic>) => onChange({ music: { ...music, ...changes } });
  const withStatus = context.features.includes("status");
  if (entries.length === 0) {
    return (
      <Row label="Music" description="Add a playlist at Music to set what happens to it during calls and breaks.">
        <span className="o3">Not set up</span>
      </Row>
    );
  }
  return (
    <>
      <Row label="Music during a call" description="Calls are noticed within 15 seconds. The volume is Music’s own, not the Mac’s, and returns to what it was after the call.">
        <div className="settings-input">
          {music.onCall === "lower" && (
            <>
              <span className="muted">to</span>
              <CommitInput
                label="Volume during a call in percent"
                className="number music-call-volume"
                value={String(music.callVolume)}
                onCommit={(text) => {
                  const volume = Number(text);
                  if (text && volume >= 0 && volume <= 100) patch({ callVolume: Math.round(volume) });
                }}
              />
              <span className="muted">%</span>
            </>
          )}
          <select className="input" aria-label="Music during a call" value={music.onCall} onChange={(e) => patch({ onCall: e.target.value as OnCall })}>
            {ON_CALL.map((option) => (
              <option key={option.value} value={option.value}>
                {option.label}
              </option>
            ))}
          </select>
        </div>
      </Row>
      {withStatus && <Switch label="Pause music on Break and Done for today" checked={music.pauseWhenAway} onChange={(pauseWhenAway) => patch({ pauseWhenAway })} />}
      <Switch label="Resume music when back" description="Only music Daybook paused, once the call or the break is over." checked={music.resume} onChange={(resume) => patch({ resume })} />
      {withStatus && (
        <Row label="Start music with the workday" description="Played with the day’s first Working, unless music already plays.">
          <select className="input" aria-label="Start music with the workday" value={music.startWith} onChange={(e) => patch({ startWith: e.target.value })}>
            <option value="">Nothing</option>
            {entries
              .filter((entry) => entry.id)
              .map((entry) => (
                <option key={entry.id} value={entry.id}>
                  {entryLabel(entry)}
                </option>
              ))}
          </select>
        </Row>
      )}
    </>
  );
}
