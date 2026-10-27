import { useEffect } from "react";
import { api } from "../../api";
import { useApiResult } from "../../lib/useApiResult";

/** Diagnostics, read when the page shows and again when the window comes back to the front. */
export function useDiagnostics() {
  const result = useApiResult("diagnostics", api.diagnosticsGet, { keepData: true });
  const { reload } = result;
  useEffect(() => {
    window.addEventListener("focus", reload);
    return () => window.removeEventListener("focus", reload);
  }, [reload]);
  return result;
}

/** The action log, read again whenever Rust logs or updates an event. */
export function useActionLog() {
  const result = useApiResult("action-log", api.actionLogGet, { keepData: true });
  const { reload } = result;
  useEffect(() => {
    const unlisten = api.onActionLogChanged(reload);
    return () => void unlisten.then((fn) => fn()).catch(() => {});
  }, [reload]);
  return result;
}
