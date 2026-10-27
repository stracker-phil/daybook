import { CircleAlert, CircleCheck, CircleX, Info, type LucideIcon } from "lucide-react";
import { api, type DiagnosticGroup, type DiagnosticRow } from "../../api";
import { fail } from "../../state/requests";
import { Group, Row } from "./controls";

const TONES: Record<DiagnosticRow["tone"], { icon: LucideIcon; label: string }> = {
  ok: { icon: CircleCheck, label: "OK" },
  warn: { icon: CircleAlert, label: "Warning" },
  error: { icon: CircleX, label: "Error" },
  info: { icon: Info, label: "Info" },
};

function DiagnosticValue({ row, onRan }: { row: DiagnosticRow; onRan: () => void }) {
  const { icon: Icon, label } = TONES[row.tone];
  const { command, commandLabel } = row;
  const run = async (id: string) => {
    try {
      await api.runCommand(id);
    } catch (e) {
      fail(e, `Couldn't run ${commandLabel}`);
    }
    onRan();
  };
  return (
    <>
      <span className="diagnostic-value" data-tone={row.tone}>
        <Icon size={14} role="img" aria-label={label} />
        <span className="selectable">{row.value}</span>
      </span>
      {command && commandLabel && (
        <button type="button" className="btn" onClick={() => void run(command)}>
          {commandLabel}
        </button>
      )}
    </>
  );
}

/** A card per group: label left, the value with its tone right, and an optional command button. */
export function DiagnosticsGroups({ groups, onRan }: { groups: DiagnosticGroup[]; onRan: () => void }) {
  return groups.map((group) => (
    <Group key={group.title} title={group.title}>
      {group.rows.map((row) => (
        <Row key={row.label} label={row.label}>
          <DiagnosticValue row={row} onRan={onRan} />
        </Row>
      ))}
    </Group>
  ));
}
