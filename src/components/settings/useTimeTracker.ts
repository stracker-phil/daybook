import { useCallback, useEffect, useRef } from "react";
import { api, type TimeTrackerInfo, type TimeTrackerProjects } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { fail } from "../../state/requests";

/** Brings TimeTracker to the front, starting it if needed; a failure toasts. */
export const openTimeTracker = () => void api.timetrackerFocus().catch((e) => fail(e, "Couldn't open TimeTracker"));

/** Reads again whenever TimeTracker's timer or state changes, e.g. once it starts. */
function useReloadOnTimerChanged(reload: () => void) {
  useEffect(() => {
    const unlisten = api.onTimerChanged(() => reload());
    return () => void unlisten.then((fn) => fn()).catch(() => {});
  }, [reload]);
}

/** Settings → TimeTracker's state; `path` is the setting, so a new path reads again. */
export function useTimetrackerInfo(path: string) {
  const result = useApiResult<TimeTrackerInfo>(`timetracker:${path}`, api.timetrackerInfo, { keepData: true });
  useReloadOnTimerChanged(result.reload);
  return result;
}

/** TimeTracker's projects for the project cards: the list Rust read last, and a fresh one on `retry`. */
export function useTimetrackerProjects() {
  const fresh = useRef(false);
  const fetcher = useCallback(() => {
    const ask = fresh.current;
    fresh.current = false;
    return api.timetrackerProjects(ask);
  }, []);
  const result = useApiResult<TimeTrackerProjects>("timetracker-projects", fetcher, { keepData: true });
  const { reload } = result;
  useReloadOnTimerChanged(reload);
  const retry = useCallback(() => {
    fresh.current = true;
    reload();
  }, [reload]);
  return { ...result, retry };
}

export type TimeTrackerProjectsResult = ReturnType<typeof useTimetrackerProjects>;
