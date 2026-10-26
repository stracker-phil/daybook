import { Fragment } from "react";
import type { EndOfDay, Status } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { AbsenceWorkflows } from "./AbsenceWorkflows";
import { Group, Switch, TextField } from "./controls";
import { SlackStatusField } from "./SlackStatusField";

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
      description="The Slack status is what your profile shows ({note} in its text is your channel message). The channel message is a suggestion prefilled in the status overlay: what you confirm there is posted to the channel, nothing if you clear it, never twice in a row. Picking the status you already have does nothing. A status switched off only sets the status."
    >
      <TextField label="Channel" description="Status changes post their message here. Empty: no messages." placeholder="#team-status" value={slack.channel} onCommit={(channel) => set({ slack: { ...slack, channel: channel.trim() } })} />
      {STATUSES.map(({ status, label }) => (
        <Fragment key={status}>
          <Switch label={label} checked={workflows[status]} onChange={(on) => set({ slack: { ...slack, manualWorkflows: { ...workflows, [status]: on } } })} />
          <SlackStatusField nested value={slack.statuses[status]} onChange={(next) => set({ slack: { ...slack, statuses: { ...slack.statuses, [status]: next } } })} />
          <TextField nested label="Channel message" value={messages[status]} placeholder="No message" onCommit={(text) => set({ statusMessages: { ...messages, [status]: text } })} />
          {status === "working" && (
            <TextField
              nested
              label="First of the day"
              description="Suggested for the day's first Working instead."
              value={messages.firstWorking}
              placeholder={messages.working ? `Same as above: ${messages.working}` : "Same as above"}
              onCommit={(firstWorking) => set({ statusMessages: { ...messages, firstWorking } })}
            />
          )}
          {status === "off" && (
            <Switch
              nested
              label="Ask first"
              description={s.endOfDay.enabled ? "The end-of-day check (below) comes before Done for today." : "Done for today is set right away."}
              checked={s.endOfDay.enabled}
              onChange={(enabled) => set({ endOfDay: { ...s.endOfDay, enabled } })}
            />
          )}
        </Fragment>
      ))}
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
      <AbsenceWorkflows {...props} />
    </>
  );
}
