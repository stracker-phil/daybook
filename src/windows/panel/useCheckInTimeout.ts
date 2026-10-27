import { useEffect, useState } from "react";
import { api } from "../../api";

const CHECK_IN_TIMEOUT_MS = 60_000;

/** Closes a check-in after a minute with focus and no change of the pick; blur pauses it. */
export function useCheckInTimeout(enabled: boolean, project: string, ttProjectId: string | null, note: string) {
  const [focused, setFocused] = useState(() => document.hasFocus());

  useEffect(() => {
    const onFocus = () => setFocused(true);
    const onBlur = () => setFocused(false);
    window.addEventListener("focus", onFocus);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("focus", onFocus);
      window.removeEventListener("blur", onBlur);
    };
  }, []);

  useEffect(() => {
    if (!enabled || !focused) return;
    const timer = setTimeout(() => void api.panelClose().catch(() => {}), CHECK_IN_TIMEOUT_MS);
    return () => clearTimeout(timer);
  }, [enabled, focused, project, ttProjectId, note]);
}
