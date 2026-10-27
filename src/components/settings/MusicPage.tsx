import { useState } from "react";
import { api, toAppError, type ArtistMatch, type ArtistSearch, type MusicEntry, type MusicKind } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Checkbox, ControlRow, EntryFooter, Group } from "./controls";
import { CommitInput, ItemActions, moveItem, replaceItem } from "./fields";

const NEW_ENTRY: MusicEntry = { id: "", name: "New entry", kind: "playlist", source: "", shuffle: true, artistId: "", storefront: "" };

const KINDS: { value: MusicKind; label: string }[] = [
  { value: "playlist", label: "Playlist" },
  { value: "artist", label: "Artist radio" },
  { value: "link", label: "Link" },
];

/** Type the artist; the best match is taken, and another artist of that name can be picked. */
function ArtistField({ entry, onChange }: { entry: MusicEntry; onChange: (patch: Partial<MusicEntry>) => void }) {
  const [found, setFound] = useState<ArtistSearch | null>(null);
  const [note, setNote] = useState<string | null>(null);
  const matches = found?.artists ?? [];
  const take = (match: ArtistMatch, storefront: string) => onChange({ source: match.name, artistId: match.id, storefront });
  const search = (term: string) => {
    if (!term) return;
    setNote("Searching…");
    api.musicFindArtists(term).then(
      (result) => {
        setFound(result);
        setNote(result.artists.length > 0 ? null : "No artist found");
        if (result.artists[0]) take(result.artists[0], result.storefront);
        else onChange({ source: term, artistId: "" });
      },
      (e) => setNote(toAppError(e).message),
    );
  };
  const showPicker = matches.length > 1 && matches.some((match) => match.id === entry.artistId);
  return (
    <div className="music-source">
      <CommitInput label="Artist" className="grow" placeholder="Artist name, then ↵" value={entry.source} onCommit={search} />
      {showPicker && (
        <select
          className="input"
          aria-label="Which artist"
          value={entry.artistId}
          onChange={(e) => {
            const match = matches.find((m) => m.id === e.target.value);
            if (match && found) take(match, found.storefront);
          }}
        >
          {matches.map((match) => (
            <option key={match.id} value={match.id}>
              {match.genre ? `${match.name} · ${match.genre}` : match.name}
            </option>
          ))}
        </select>
      )}
      {note && <span className="music-note o3">{note}</span>}
    </div>
  );
}

/** A playlist is picked from the library once it's loaded, and typed by name before that. */
function PlaylistField({ entry, playlists, onChange }: { entry: MusicEntry; playlists: string[] | null; onChange: (patch: Partial<MusicEntry>) => void }) {
  if (!playlists) {
    return <CommitInput label="Playlist" className="music-source-input" placeholder="Playlist name, exactly as in Music" value={entry.source} onCommit={(source) => onChange({ source })} />;
  }
  const names = entry.source && !playlists.includes(entry.source) ? [entry.source, ...playlists] : playlists;
  return (
    <select className="input music-source-input" aria-label="Playlist" value={entry.source} onChange={(e) => onChange({ source: e.target.value })}>
      {!entry.source && (
        <option value="" disabled>
          Choose a playlist
        </option>
      )}
      {names.map((name) => (
        <option key={name} value={name}>
          {name}
        </option>
      ))}
    </select>
  );
}

/** Where the entry plays from: a library playlist, an artist's station or an Apple Music link (Share → Copy Link). */
function SourceField({ entry, playlists, onChange }: { entry: MusicEntry; playlists: string[] | null; onChange: (patch: Partial<MusicEntry>) => void }) {
  if (entry.kind === "playlist") return <PlaylistField entry={entry} playlists={playlists} onChange={onChange} />;
  if (entry.kind === "artist") return <ArtistField entry={entry} onChange={onChange} />;
  return <CommitInput label="Apple Music link" className="music-source-input" placeholder="https://music.apple.com/…" value={entry.source} onCommit={(source) => onChange({ source })} />;
}

interface EntryRowProps {
  entry: MusicEntry;
  index: number;
  count: number;
  playlists: string[] | null;
  onChange: (patch: Partial<MusicEntry>) => void;
  onMove: (from: number, to: number) => void;
  onRemove: () => void;
}

function EntryRow({ entry, index, count, playlists, onChange, onMove, onRemove }: EntryRowProps) {
  return (
    <ControlRow>
      <div className="item-line music-line">
        <CommitInput label="Name" className="music-name" placeholder="Name" required value={entry.name} onCommit={(name) => onChange({ name })} />
        <select
          className="input music-kind"
          aria-label="Kind"
          value={entry.kind}
          onChange={(e) => onChange({ kind: e.target.value as MusicKind, source: "", artistId: "", storefront: "" })}
        >
          {KINDS.map((kind) => (
            <option key={kind.value} value={kind.value}>
              {kind.label}
            </option>
          ))}
        </select>
        <SourceField entry={entry} playlists={playlists} onChange={onChange} />
        {entry.kind === "playlist" && (
          <Checkbox label="Shuffle" checked={entry.shuffle} onChange={(shuffle) => onChange({ shuffle })} />
        )}
        <ItemActions index={index} count={count} onMove={onMove} onRemove={onRemove} removeLabel="Remove Entry" />
      </div>
    </ControlRow>
  );
}

/** One line per playlist, artist radio or Apple Music link; the library's playlists load on request, since asking starts Music. */
export function MusicPage({ s, set }: SettingsPageProps) {
  const [playlists, setPlaylists] = useState<string[] | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const entries = s.music;
  const save = (music: MusicEntry[]) => set({ music });

  const loadPlaylists = () => {
    setLoading(true);
    api
      .musicPlaylists()
      .then(
        (names) => {
          setPlaylists(names);
          setError(null);
        },
        (e) => setError(toAppError(e).message),
      )
      .finally(() => setLoading(false));
  };

  return (
    <>
      <Group
        title="Playlists and stations"
        description="Offered in the rail, the menu-bar menu and the Music overlay. A playlist is one from your library; artist radio is the station Apple Music builds around an artist (“… & Similar Artists”); a link is anything else from Apple Music (in Music: Share → Copy Link). What happens during calls and breaks is set per context, at Contexts."
      >
        <div className="item-list">
          {entries.map((entry, i) => (
            <EntryRow
              key={entry.id || i}
              entry={entry}
              index={i}
              count={entries.length}
              playlists={playlists}
              onChange={(patch) => save(replaceItem(entries, i, patch))}
              onMove={(from, to) => save(moveItem(entries, from, to))}
              onRemove={() => save(entries.filter((_, j) => j !== i))}
            />
          ))}
        </div>
        <EntryFooter>
          <button type="button" className="btn" onClick={() => save([...entries, NEW_ENTRY])}>
            Add Entry
          </button>
          <button type="button" className="btn btn-quiet" disabled={loading} onClick={loadPlaylists}>
            {loading ? "Asking Music…" : playlists ? "Reload My Playlists" : "Load My Playlists"}
          </button>
          {error && (
            <span className="form-field-error music-error" role="alert">
              {error}
            </span>
          )}
        </EntryFooter>
      </Group>
    </>
  );
}
