import { Fragment } from "react";
import type { EndOfDay, Status } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, Switch, TextField } from "./controls";

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
  const workflows = s.slack.manualWorkflows;
  const messages = s.statusMessages;
  return (
    <Group
      title="When you pick a status"
      description="The channel message is a suggestion prefilled in the status overlay: what you confirm there is posted to the availability channel, nothing if you clear it. Picking the status you already have does nothing. A status switched off only sets the status."
    >
      {STATUSES.map(({ status, label }) => (
        <Fragment key={status}>
          <Switch label={label} checked={workflows[status]} onChange={(on) => set({ slack: { ...s.slack, manualWorkflows: { ...workflows, [status]: on } } })} />
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

export function WorkflowsPage(props: SettingsPageProps) {
  return (
    <>
      <StatusWorkflows {...props} />
      <EndOfDayCheck {...props} />
    </>
  );
}
