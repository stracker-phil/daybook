import { useEffect, useState, type KeyboardEvent } from "react";
import { Music } from "lucide-react";
import { api, toAppError, type MusicEntry, type MusicListItem, type MusicState } from "../../api";
import { entryLabel } from "../../lib/music";
import { cycle, useActiveScroll } from "./listKeys";

const VOLUME_STEP = 10;
const SHORTCUT_ROWS = 9;
const DIRECTION: Record<string, number> = { ArrowDown: 1, ArrowUp: -1 };
const VOLUME_BY_KEY: Record<string, number> = { ArrowUp: VOLUME_STEP, ArrowDown: -VOLUME_STEP };

const listItem = (entry: MusicEntry): MusicListItem => ({ id: entry.id, name: entryLabel(entry) });

/** ⌘1…⌘9's row number from `e.code`, else 0. */
const digitKey = (code: string) => Number(/^Digit([1-9])$/.exec(code)?.[1] ?? 0);

function stateLine(music: MusicState): string {
  const name = music.track ?? music.entries.find((entry) => entry.id === music.entry)?.name;
  if (music.player === "playing") return name ? `Playing · ${name}` : "Playing";
  if (music.player === "paused") return name ? `Paused · ${name}` : "Paused";
  return "Nothing is playing";
}

function hints(music: MusicState | null, entries: number): string {
  return [
    entries > 0 && "↑↓ entry · ↵ play",
    entries > 0 && "⌘1–9 play",
    music?.player === "playing" ? "⌘P pause" : music?.player === "paused" ? "⌘P resume" : "⌘P play",
    typeof music?.volume === "number" && "⌘↑↓ volume",
    "esc close",
  ]
    .filter(Boolean)
    .join(" · ");
}

function Volume({ volume }: { volume: number }) {
  return (
    <span className="music-volume" aria-label={`Volume ${volume} %`}>
      <span className="music-volume-track" aria-hidden="true">
        <span className="music-volume-fill" style={{ width: `${volume}%` }} />
      </span>
      <span className="music-volume-text o3" aria-hidden="true">
        {volume} %
      </span>
    </span>
  );
}

/** ⌘ alone, so ⌃ and ⌥ combinations stay free. */
const commandOnly = (e: KeyboardEvent) => e.metaKey && !e.ctrlKey && !e.altKey;

/** Music's state, kept current by `music_changed`. Without an answer from Music, the saved entries still show, with the reason. */
function useMusicState() {
  const [music, setMusic] = useState<MusicState | null>(null);
  const [saved, setSaved] = useState<MusicListItem[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.musicState().then(setMusic, (e) => {
      setError(toAppError(e).message);
      api.getSettings().then((settings) => setSaved(settings.music.map(listItem)), () => setSaved([]));
    });
    const unlisten = api.onMusicChanged(setMusic);
    return () => void unlisten.then((fn) => fn()).catch(() => {});
  }, []);

  return { music, setMusic, entries: music?.entries ?? saved, error, setError };
}

/** Pick what plays, pause and set Music's volume, all from the keyboard. */
export function MusicView() {
  const { music, setMusic, entries, error, setError } = useMusicState();
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [starting, setStarting] = useState<string | null>(null);

  const words = query.toLowerCase().split(/\s+/).filter(Boolean);
  const shown = (entries ?? []).filter((entry) => words.every((word) => entry.name.toLowerCase().includes(word)));
  const current = Math.min(selected, Math.max(0, shown.length - 1));
  const activeRef = useActiveScroll<HTMLLIElement>(current);

  const act = (request: Promise<MusicState>) =>
    request.then(
      (next) => {
        setMusic(next);
        setError(null);
      },
      (e) => setError(toAppError(e).message),
    );

  const play = (entry: MusicListItem | undefined) => {
    if (!entry || starting) return;
    setStarting(entry.name);
    api
      .musicPlay(entry.id)
      .then(() => api.panelClose())
      .catch((e) => {
        setStarting(null);
        setError(toAppError(e).message);
      });
  };

  const changeVolume = (change: number) => {
    const volume = music?.volume;
    if (typeof volume !== "number") return;
    void act(api.musicVolume(Math.max(0, Math.min(100, volume + change))));
  };

  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) return;
    const command = commandOnly(e);
    const digit = command ? digitKey(e.code) : 0;
    const step = DIRECTION[e.key];
    if (command && e.code === "KeyP") {
      e.preventDefault();
      void act(api.musicToggle());
    } else if (command && VOLUME_BY_KEY[e.key]) {
      e.preventDefault();
      changeVolume(VOLUME_BY_KEY[e.key]);
    } else if (digit) {
      e.preventDefault();
      play(shown[digit - 1]);
    } else if (step && !e.metaKey && shown.length > 0) {
      e.preventDefault();
      setSelected(cycle(current, step, shown.length));
    } else if (e.key === "Enter") {
      e.preventDefault();
      play(shown[current]);
    }
  };

  return (
    <div className="panel-card palette-view">
      <div className="palette-input" data-stage="panel:input" data-stage-label="Music entry" data-stage-role="field">
        <Music size="1.077rem" aria-hidden="true" />
        <input
          autoFocus
          aria-label="Entry"
          placeholder="Play…"
          value={query}
          onChange={(e) => {
            setQuery(e.target.value);
            setSelected(0);
          }}
          onKeyDown={onKeyDown}
        />
      </div>
      {music && (
        <div className="music-state">
          <span className="grow-text o2">{starting ? `Starting ${starting}…` : stateLine(music)}</span>
          {typeof music.volume === "number" && <Volume volume={music.volume} />}
        </div>
      )}
      <ul className="palette-list music-list" role="listbox" aria-label="Music">
        {entries && shown.length === 0 && <li className="o3">{entries.length === 0 ? "No music yet. Add playlists in Settings → Music." : "No matching entry."}</li>}
        {shown.map((entry, index) => (
          <li
            key={entry.id}
            ref={index === current ? activeRef : undefined}
            role="option"
            aria-selected={index === current}
            className={index === current ? "active" : undefined}
            // The second click of a double-click lands on the selected row and plays it.
            onClick={() => (index === current ? play(entry) : setSelected(index))}
          >
            <span className="context-number o3">{index < SHORTCUT_ROWS ? `⌘${index + 1}` : ""}</span>
            <span className="grow-text">{entry.name}</span>
            {entry.id === music?.entry && <span className="context-pill">Current</span>}
          </li>
        ))}
      </ul>
      <div className="palette-foot o3">{error ? <span className="palette-error">{error}</span> : <span>{hints(music, shown.length)}</span>}</div>
    </div>
  );
}
