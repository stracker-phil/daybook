import type { MusicEntry } from "../api";

/** How an entry of Settings → Music is named: its name, else its source. */
export const entryLabel = (entry: MusicEntry) => entry.name || entry.source;
