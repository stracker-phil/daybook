import type { TimeTrackerInfo } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, PathText, Row, TextField } from "./controls";
import { openTimeTracker, useTimetrackerInfo } from "./useTimeTracker";

function StateLabel({ info }: { info: TimeTrackerInfo }) {
  if (info.state === "available") return <span className="settings-state found">{info.version ? `Running · ${info.version}` : "Running"}</span>;
  return <span className="settings-state missing">{info.state === "notRunning" ? "TimeTracker isn't running" : "Not installed"}</span>;
}

/** Connections → TimeTracker. */
export function TimeTrackerGroup({ s, set }: SettingsPageProps) {
  const timetracker = s.timetracker;
  const { data: info } = useTimetrackerInfo(timetracker.path);
  return (
    <Group title="TimeTracker" anchor="timetracker" description="Daybook shows what TimeTracker is timing; it never records time itself.">
      <Row label="State" description={info ? <PathText path={info.path} /> : undefined}>
        {info && (
          <span className="button-row">
            <StateLabel info={info} />
            {info.state === "notRunning" && (
              <button type="button" className="btn" onClick={openTimeTracker}>
                Open TimeTracker
              </button>
            )}
          </span>
        )}
      </Row>
      <TextField
        label="Path"
        description={info ? `The timetracker command. Empty: ${info.defaultPath}.` : "The timetracker command."}
        placeholder={info?.defaultPath}
        wide
        value={timetracker.path}
        onCommit={(path) => set({ timetracker: { ...timetracker, path: path.trim() } })}
      />
    </Group>
  );
}
