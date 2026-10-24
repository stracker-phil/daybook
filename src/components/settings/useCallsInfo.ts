import { useEffect } from "react";
import { api } from "../../api";
import { useApiResult } from "../../lib/useApiResult";

/** Call detection's state, read again when a call changes and when the window comes back from System Settings. */
export function useCallsInfo() {
  const result = useApiResult("calls", api.callsInfo, { keepData: true });
  const { reload } = result;
  useEffect(() => {
    const unlisten = api.onCallChanged(reload);
    window.addEventListener("focus", reload);
    return () => {
      window.removeEventListener("focus", reload);
      void unlisten.then((fn) => fn()).catch(() => {});
    };
  }, [reload]);
  return result;
}
