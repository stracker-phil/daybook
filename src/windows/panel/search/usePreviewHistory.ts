import { useCallback, useState } from "react";

interface History {
  /** The result the links were followed from. */
  from: string | null;
  /** Notes reached through links, oldest first. */
  trail: string[];
  /** Index into `trail` of the shown note; -1 shows `from`. */
  index: number;
}

const fresh = (from: string | null): History => ({ from, trail: [], index: -1 });

/**
 * The preview follows links while the highlight stays on `result`, with back and forward through them.
 * Another result starts over.
 */
export function usePreviewHistory(result: string | null) {
  const [history, setHistory] = useState<History>(() => fresh(result));
  const current = history.from === result ? history : fresh(result);
  if (current !== history) setHistory(current);

  // Stable, so the preview's link renderers stay the same; by a click the history already belongs to `result`.
  const navigate = useCallback((to: string) => setHistory((h) => ({ ...h, trail: [...h.trail.slice(0, h.index + 1), to], index: h.index + 1 })), []);
  const canBack = current.index >= 0;
  const canForward = current.index < current.trail.length - 1;

  return {
    shown: current.index >= 0 ? current.trail[current.index] : result,
    navigate,
    canBack,
    canForward,
    back: () => {
      if (canBack) setHistory({ ...current, index: current.index - 1 });
    },
    forward: () => {
      if (canForward) setHistory({ ...current, index: current.index + 1 });
    },
  };
}
