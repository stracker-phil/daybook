/** Settings → Workflows: Slack heads-ups before an absence in the calendar, and what counts as one. */
import type { StatusRule, StatusRules } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, NumberField, Row, Switch, TextField } from "./controls";
import { joinList, parseList, replaceItem } from "./fields";
import { SlackStatusField } from "./SlackStatusField";

const MAX_HOURS = 24 * 60;
const MAX_BUSY_HOURS = 24 * 14;

const NEW_RULE: StatusRule = {
  hours: 24,
  enabled: true,
  slack: { keepStatus: false, emoji: ":information_source:", text: "Away {when}", presence: "keep" },
};

const PLACEHOLDERS = "{when} (e.g. “from 2pm to 7pm CEST”, “from 2026-10-05 to 2026-10-09”), {start} {end} {startDate} {endDate} {tz}.";

/** Rust keeps the rules sorted by hours, so a rule has no Move Up or Down. */
function AbsenceRule({ rule, onChange, onRemove }: { rule: StatusRule; onChange: (patch: Partial<StatusRule>) => void; onRemove: () => void }) {
  return (
    <Group title={`Absence starts within ${rule.hours} hours`}>
      <Switch label="Workflow on" checked={rule.enabled} onChange={(enabled) => onChange({ enabled })} />
      <NumberField label="Hours before" description="Shown from this long before the absence starts." min={1} max={MAX_HOURS} value={rule.hours} onCommit={(hours) => onChange({ hours: Math.round(hours) })} />
      <SlackStatusField description={`Text placeholders: ${PLACEHOLDERS} Presence “No change” follows your status.`} value={rule.slack} onChange={(slack) => onChange({ slack })} />
      <Row label="">
        <div className="item-actions">
          <button type="button" className="btn btn-danger" onClick={onRemove}>
            Remove Rule
          </button>
        </div>
      </Row>
    </Group>
  );
}

export function AbsenceWorkflows({ s, set }: SettingsPageProps) {
  const config = s.statusRules;
  const change = (patch: Partial<StatusRules>) => set({ statusRules: { ...config, ...patch } });
  const rules = config.rules;
  const setRules = (next: StatusRule[]) => change({ rules: next });
  return (
    <>
      <Group
        title="Before an absence"
        description="Slack shows the rule's status until the absence starts; then your status comes back. If several apply, the one with the fewest hours wins, so a short heads-up replaces a longer one as the absence gets closer. A call comes first. Checked every minute."
      >
        <TextField label="Absence titles" description="Comma-separated, e.g. Out of office (from the HR calendar)." value={joinList(config.absenceTitles)} onCommit={(text) => change({ absenceTitles: parseList(text) })} />
        <NumberField label="Busy blocks from" description="Longer busy events count as absences too." suffix="hours" min={1} max={MAX_BUSY_HOURS} value={config.minBusyHours} onCommit={(minBusyHours) => change({ minBusyHours })} />
        <TextField label="Time zone label" description="After times in {tz}; empty uses the Mac's, e.g. CEST." placeholder="Automatic" value={config.tzLabel} onCommit={(tzLabel) => change({ tzLabel: tzLabel.trim() })} />
      </Group>
      {rules.map((rule, i) => (
        <AbsenceRule
          key={i}
          rule={rule}
          onChange={(patch) => setRules(replaceItem(rules, i, patch))}
          onRemove={() => setRules(rules.filter((_, j) => j !== i))}
        />
      ))}
      <div className="button-row">
        <button type="button" className="btn" onClick={() => setRules([...rules, NEW_RULE])}>
          Add Rule
        </button>
      </div>
    </>
  );
}
