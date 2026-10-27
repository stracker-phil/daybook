/** What a page tells Stage it shows: its view, the Settings section, which toggleable parts are open. */
import { useEffect, useState } from "react";
import type { StagePageMeta } from "./types";

const parts = new Map<symbol, StagePageMeta>();

/** Several components may each tell a part; the last `view` and `section` win, `ui` objects merge. */
export function useStageMeta(meta: StagePageMeta): void {
  const [key] = useState(() => Symbol("stage-meta"));
  const json = JSON.stringify(meta);
  useEffect(() => {
    parts.set(key, JSON.parse(json) as StagePageMeta);
    return () => void parts.delete(key);
  }, [key, json]);
}

export function currentMeta(): StagePageMeta {
  const meta: StagePageMeta = {};
  for (const part of parts.values()) {
    if (part.view !== undefined) meta.view = part.view;
    if (part.section !== undefined) meta.section = part.section;
    if (part.ui) meta.ui = { ...meta.ui, ...part.ui };
  }
  return meta;
}
