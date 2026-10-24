import { useEffect } from "react";
import { api, type ContextState } from "../api";

/** The current context's colour on the root, as `data-context` for the `--context` tokens (styles/context.css). */
export function applyContextColor(state: ContextState) {
  const color = state.contexts.find((context) => context.id === state.current)?.color;
  if (color) document.documentElement.dataset.context = color;
  else delete document.documentElement.dataset.context;
}

/** Keeps the root's context colour current: on mount, on a switch and after a settings save. */
export function useContextColor() {
  useEffect(() => {
    const refresh = () => void api.contextState().then(applyContextColor, () => {});
    refresh();
    const unlisten = [api.onContextChanged(refresh), api.onSettingsChanged(refresh)];
    return () => unlisten.forEach((p) => void p.then((fn) => fn()).catch(() => {}));
  }, []);
}
