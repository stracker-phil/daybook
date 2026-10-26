import { useEffect, useState } from "react";
import { api, toAppError, type FocusInfo } from "../../api";

/** Read on every open: the list may have changed in Obsidian. */
export function useFocusInfo(): { info: FocusInfo | null; error: string | null } {
  const [info, setInfo] = useState<FocusInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    void api.focusInfo().then(setInfo, (e) => setError(toAppError(e).message));
  }, []);
  return { info, error };
}

/** The overlay stays while you look things up in another app. */
export function useKeepPanelOpen() {
  useEffect(() => {
    void api.panelKeepOpen(true).catch(() => {});
  }, []);
}
