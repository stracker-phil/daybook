import { useEffect } from "react";
import { api } from "../../api";
import { useApiResult } from "../../lib/useApiResult";

/** Calendar access and this Mac's event calendars, read again whenever a calendar or the access changes. */
export function useCalendarInfo() {
  const result = useApiResult("calendar", api.calendarInfo, { keepData: true });
  const { reload } = result;
  useEffect(() => {
    const unlisten = api.onCalendarChanged(reload);
    return () => void unlisten.then((fn) => fn()).catch(() => {});
  }, [reload]);
  return result;
}
