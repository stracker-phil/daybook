import { useCallback, useEffect } from "react";
import { api } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { fail } from "../../state/requests";

/** TimeTracker's day for the Activity tab; re-reads while shown so a running span and the total stay current. */
export function useActivityDay(day: string) {
  const fetcher = useCallback(() => api.timetrackerDay(day), [day]);
  const result = useApiResult(`activity:${day}`, fetcher, { keepData: true, onError: (e) => fail(e, "Couldn't read TimeTracker's day") });
  const { reload } = result;

  useEffect(() => {
    const everyMinute = window.setInterval(reload, 60_000);
    window.addEventListener("focus", reload);
    const unlisten = api.onActivityChanged(reload);
    return () => {
      window.clearInterval(everyMinute);
      window.removeEventListener("focus", reload);
      void unlisten.then((fn) => fn()).catch(() => {});
    };
  }, [reload]);

  return result;
}
