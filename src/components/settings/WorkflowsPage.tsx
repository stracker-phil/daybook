import type { EndOfDay, Status } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { AbsenceWorkflows } from "./AbsenceWorkflows";
import { CallWorkflows } from "./CallWorkflows";
import { ControlRow, FormRow, FormText, Group, InlineSwitch, LeadSwitch, Row, Switch, TextField } from "./controls";
import { SlackStatusEditor } from "./SlackStatusField";

const STATUSES: { status: Status; label: string }[] = [
  { status: "working", label: "Working and present" },
  { status: "break", label: "Taking a break" },
  { status: "off", label: "Done for today" },
];

interface CheckField {
  key: Exclude<keyof EndOfDay, "enabled">;
  label: string;
  description?: string;
  placeholder?: string;
  required?: boolean;
}

const CHECK_FIELDS: CheckField[] = [
  { key: "reminder", label: "Reminder", description: "Shown above the question." },
  { key: "question", label: "Question", required: true },
  { key: "url", label: "Link", description: "Opened from the question, e.g. your timesheets.", placeholder: "https://" },
  { key: "done", label: "Message if yes", required: true },
  { key: "incomplete", label: "Message if not yet", required: true },
];

function StatusWorkflows({ s, set }: SettingsPageProps) {
  const slack = s.slack;
  const workflows = slack.manualWorkflows;
  const messages = s.statusMessages;
  return (
    <Group
      title="When you pick a status"
      description="Sets your Slack status ({note} in its text is the channel message) and suggests the channel message in the status overlay; only what you confirm is posted."
    >
      <TextField label="Channel" description="Status changes post their message here. Empty: no messages." placeholder="#team-status" value={slack.channel} onCommit={(channel) => set({ slack: { ...slack, channel: channel.trim() } })} />
      <ControlRow stacked>
        {STATUSES.map(({ status, label }) => (
          <LeadSwitch key={status} label={label} checked={workflows[status]} onChange={(on) => set({ slack: { ...slack, manualWorkflows: { ...workflows, [status]: on } } })}>
            <FormRow label="Slack status">
              <SlackStatusEditor value={slack.statuses[status]} onChange={(next) => set({ slack: { ...slack, statuses: { ...slack.statuses, [status]: next } } })} />
            </FormRow>
            <FormText label="Channel message" value={messages[status]} placeholder="No message" onCommit={(text) => set({ statusMessages: { ...messages, [status]: text } })} />
            {status === "working" && (
              <FormText
                label="First of the day"
                value={messages.firstWorking}
                placeholder={messages.working ? `Same as above: ${messages.working}` : "Same as above"}
                onCommit={(firstWorking) => set({ statusMessages: { ...messages, firstWorking } })}
              />
            )}
            {status === "off" && (
              <FormRow label="Ask first">
                <InlineSwitch
                  label="Ask first"
                  text={s.endOfDay.enabled ? "The end-of-day check (below) comes before Done for today." : "Done for today is set right away."}
                  checked={s.endOfDay.enabled}
                  onChange={(enabled) => set({ endOfDay: { ...s.endOfDay, enabled } })}
                />
              </FormRow>
            )}
          </LeadSwitch>
        ))}
      </ControlRow>
      <Switch
        label="Start and stop the timer"
        description="Working continues TimeTracker's last timer (or asks what you work on for another context's project). Break and Done for today stop it."
        checked={s.timetracker.statusTimer}
        onChange={(statusTimer) => set({ timetracker: { ...s.timetracker, statusTimer } })}
      />
      <Row
        label="Finished calls"
        description="While Working, a call of 3 minutes or more whose title or calendar event names a project (its name or a meeting word, Settings → Projects) waits in TimeTracker as a suggestion to approve."
      />
    </Group>
  );
}

function EndOfDayCheck({ s, set }: SettingsPageProps) {
  const check = s.endOfDay;
  return (
    <Group title="End-of-day check" description="Asked when you pick Done for today, before anything is sent. Esc cancels. {message} is your channel message.">
      {CHECK_FIELDS.map((field) => (
        <TextField
          key={field.key}
          label={field.label}
          description={field.description}
          placeholder={field.placeholder}
          required={field.required}
          value={check[field.key]}
          onCommit={(text) => set({ endOfDay: { ...check, [field.key]: text } })}
        />
      ))}
    </Group>
  );
}

function DailyFocus({ s, set }: SettingsPageProps) {
  const focus = s.focus;
  return (
    <Group
      title="Daily focus"
      description="Your focus list lives in today's note (Daily Note → Focus). Tasks are bullets with an inline-code checkbox, e.g. - `[x]` :nexo: Task, so they don't count as open todos; each project's emoji is set in Projects."
    >
      <Switch
        label="Daily focus"
        description="The list on the board. In Slack, ⌘S in the focus overlay adds it to the day's first Working message, and Done for today first asks for the end-of-day list and posts it as a reply to that message, also sent to the channel."
        checked={focus.enabled}
        onChange={(enabled) => set({ focus: { ...focus, enabled } })}
      />
      <TextField nested label="End-of-day title" description="The first line of the reply; empty: just the list." value={focus.endOfDayTitle} onCommit={(endOfDayTitle) => set({ focus: { ...focus, endOfDayTitle: endOfDayTitle.trim() } })} />
    </Group>
  );
}

export function WorkflowsPage(props: SettingsPageProps) {
  return (
    <>
      <StatusWorkflows {...props} />
      <EndOfDayCheck {...props} />
      <DailyFocus {...props} />
      <CallWorkflows {...props} />
      <AbsenceWorkflows {...props} />
    </>
  );
}
