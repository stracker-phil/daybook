import type { TimeTrackerInfo } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, Row, TextField } from "./controls";
import { openTimeTracker, useTimetrackerInfo } from "./useTimeTracker";

/** `/Users/name/…` → `~/…`. */
const tildePath = (path: string) => path.replace(/^\/Users\/[^/]+/, "~");

function StateLabel({ info }: { info: TimeTrackerInfo }) {
  if (info.state === "available") return <span className="settings-state found">{info.version ? `Running · ${info.version}` : "Running"}</span>;
  return <span className="settings-state missing">{info.state === "notRunning" ? "TimeTracker isn't running" : "Not installed"}</span>;
}

export function TimeTrackerPage({ s, set }: SettingsPageProps) {
  const timetracker = s.timetracker;
  const { data: info } = useTimetrackerInfo(timetracker.path);
  return (
    <Group title="Command line tool">
      <Row label="State" description={info ? <span className="mono selectable">{tildePath(info.path)}</span> : undefined}>
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
