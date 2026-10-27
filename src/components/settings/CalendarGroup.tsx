import { api, type CalendarAccess, type CalendarInfo } from "../../api";
import { fail } from "../../state/requests";
import { Group, Row } from "./controls";
import { useCalendarInfo } from "./useCalendarInfo";

const HINTS: Partial<Record<CalendarAccess, string>> = {
  "full-access": "Daybook can read your calendar events.",
  "not-determined": "macOS asks once.",
};
const DENIED_HINT = "Turn on access for Daybook in System Settings → Privacy & Security → Calendars.";

function AccessControl({ info, onRequest }: { info: CalendarInfo; onRequest: () => void }) {
  if (info.access === "full-access") return <span className="settings-state found">Allowed</span>;
  if (info.access === "not-determined") {
    return (
      <button type="button" className="btn btn-primary" onClick={onRequest}>
        Allow Access…
      </button>
    );
  }
  return (
    <button type="button" className="btn" onClick={onRequest}>
      Open Privacy Settings
    </button>
  );
}

/** Connections → Calendar. */
export function CalendarGroup() {
  const { data: info, reload } = useCalendarInfo();
  const request = async () => {
    try {
      await api.calendarRequestAccess();
    } catch (e) {
      fail(e, "Couldn't request calendar access");
    }
    reload();
  };
  return (
    <Group title="Calendar" anchor="calendar" description="Read for the day's events and meeting reminders, never changed. Each context picks its calendars in Settings → Contexts.">
      {info && (
        <Row label="Access" description={HINTS[info.access] ?? DENIED_HINT}>
          <AccessControl info={info} onRequest={() => void request()} />
        </Row>
      )}
    </Group>
  );
}
