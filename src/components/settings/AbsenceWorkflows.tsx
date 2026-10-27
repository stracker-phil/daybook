/** Settings → Workflows: Slack heads-ups before an absence in the calendar, and what counts as one. */
import type { StatusRule, StatusRules } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Trash2 } from "lucide-react";
import { BufferedInput, ControlRow, EntryFooter, FormRow, Group, LeadSwitch, NumberField, TextField } from "./controls";
import { joinList, parseList, replaceItem } from "./fields";
import { SlackStatusEditor } from "./SlackStatusField";

const MAX_HOURS = 24 * 60;
const MAX_BUSY_HOURS = 24 * 14;

const NEW_RULE: StatusRule = {
  hours: 24,
  enabled: true,
  slack: { keepStatus: false, emoji: ":information_source:", text: "Away {when}", presence: "keep" },
};

const PLACEHOLDERS = "{when} (e.g. “from 2pm to 7pm CEST”, “from 2026-10-05 to 2026-10-09”), {start} {end} {startDate} {endDate} {tz}.";

/** Rust keeps rules sorted by hours, so there is no Move Up or Down. */
function AbsenceRule({ rule, onChange, onRemove }: { rule: StatusRule; onChange: (patch: Partial<StatusRule>) => void; onRemove: () => void }) {
  const title = (
    <>
      <strong>Absence starts within</strong>
      <BufferedInput
        className="input number hours-input"
        type="number"
        aria-label="Hours before"
        min={1}
        max={MAX_HOURS}
        value={String(rule.hours)}
        onCommit={(hours) => onChange({ hours: Math.round(Number(hours)) })}
      />
      <strong>hours</strong>
    </>
  );
  const remove = (
    <button type="button" className="icon-btn item-remove" aria-label="Remove Rule" title="Remove Rule" onClick={onRemove}>
      <Trash2 size="1.077rem" aria-hidden="true" />
    </button>
  );
  return (
    <LeadSwitch label={`Absence within ${rule.hours} hours`} title={title} checked={rule.enabled} onChange={(enabled) => onChange({ enabled })} accessory={remove}>
      <FormRow label="Slack status">
        <SlackStatusEditor value={rule.slack} onChange={(slack) => onChange({ slack })} />
      </FormRow>
    </LeadSwitch>
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
        description={`Slack shows the rule's status until the absence starts; then your status comes back. If several apply, the one with the fewest hours wins, so a short heads-up replaces a longer one as the absence gets closer. A call comes first. Checked every minute. Text placeholders: ${PLACEHOLDERS} Presence “No change” follows your status.`}
      >
        <ControlRow stacked>
          {rules.map((rule, i) => (
            <AbsenceRule key={i} rule={rule} onChange={(patch) => setRules(replaceItem(rules, i, patch))} onRemove={() => setRules(rules.filter((_, j) => j !== i))} />
          ))}
        </ControlRow>
        <EntryFooter>
          <button type="button" className="btn" onClick={() => setRules([...rules, NEW_RULE])}>
            Add Rule
          </button>
        </EntryFooter>
      </Group>
      <Group title="Absences" description="Events in the current context's calendars (Settings → Contexts).">
        <TextField label="Titles" description="Comma-separated, e.g. Out of office (from the HR calendar)." value={joinList(config.absenceTitles)} onCommit={(text) => change({ absenceTitles: parseList(text) })} />
        <NumberField label="Busy blocks from" description="Longer busy events count as absences too." suffix="hours" min={1} max={MAX_BUSY_HOURS} value={config.minBusyHours} onCommit={(minBusyHours) => change({ minBusyHours })} />
        <TextField label="Time zone label" description="After times in {tz}; empty uses the Mac's, e.g. CEST." placeholder="Automatic" value={config.tzLabel} onCommit={(tzLabel) => change({ tzLabel: tzLabel.trim() })} />
      </Group>
    </>
  );
}
