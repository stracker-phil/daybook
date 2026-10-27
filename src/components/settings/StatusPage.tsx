import { ActionLogSection } from "./ActionLogSection";
import { DiagnosticsGroups } from "./DiagnosticsGroups";
import { useActionLog, useDiagnostics } from "./useStatusInfo";

/** Settings → Status: the diagnostics Rust reports, then the action log. */
export function StatusPage() {
  const diagnostics = useDiagnostics();
  const log = useActionLog();
  return (
    <>
      {diagnostics.error && !diagnostics.data && <p className="settings-alert" role="alert">{diagnostics.error.message}</p>}
      {diagnostics.data && <DiagnosticsGroups groups={diagnostics.data.groups} onRan={diagnostics.reload} />}
      {log.error && !log.data && <p className="settings-alert" role="alert">{log.error.message}</p>}
      {log.data && <ActionLogSection events={log.data.events} onCleared={log.reload} />}
    </>
  );
}
