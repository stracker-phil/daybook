import type { ReactNode } from "react";
import { X } from "lucide-react";
import { api, type AppRef, type CallsInfo, type Joining, type MeetingRule, type Missing, type OpenWith } from "../../api";
import { fail } from "../../state/requests";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Checkbox, ControlRow, EntryFooter, FormRow, FormText, Group, LeadSwitch, Row, Switch } from "./controls";
import { CommitInput } from "./fields";
import { listEditor } from "./listEditor";
import { SlackStatusEditor } from "./SlackStatusField";
import { useCallsInfo } from "./useCallsInfo";

const SERVICES: { service: keyof OpenWith; label: string }[] = [
  { service: "Meet", label: "Open Google Meet in" },
  { service: "Zoom", label: "Open Zoom in" },
  { service: "Teams", label: "Open Teams in" },
];

async function pickApp(): Promise<AppRef | null> {
  try {
    return await api.callsPickApp();
  } catch (e) {
    fail(e, "Couldn't use this app");
    return null;
  }
}

async function requestAccess(permission: Missing, reload: () => void) {
  try {
    await api.callsRequestAccess(permission);
  } catch (e) {
    fail(e, "Couldn't ask for access");
  }
  reload();
}

function newRule({ app, bundleId }: AppRef): MeetingRule {
  return { enabled: true, app, bundleId, windowTitle: null, slack: { keepStatus: false, emoji: ":spiral_calendar_pad:", text: `In a ${app} call`, presence: "auto" } };
}

function AllowAccessibility({ reload }: { reload: () => void }) {
  return (
    <button type="button" className="btn btn-primary" onClick={() => void requestAccess("accessibility", reload)}>
      Allow Access…
    </button>
  );
}

function CallState({ info, reload }: { info: CallsInfo; reload: () => void }) {
  const check = () => void api.runCommand("meetings:check").catch((e) => fail(e, "Couldn't check for a call")).finally(reload);
  const call = info.active;
  return (
    <Row label="Now" description={info.error ?? undefined}>
      <span className="muted">{call ? `In a ${call.app} call since ${call.since}: ${call.title}` : "No call"}</span>
      <button type="button" className="btn" onClick={check}>
        Check Now
      </button>
    </Row>
  );
}

const READS_TITLES = "Lets app triggers read other apps’ window titles.";
const TITLES_BLOCKED = "Other apps’ window titles need it, so apps with a window title can’t be recognized yet.";

function Permissions({ info, needsTitles, reload }: { info: CallsInfo; needsTitles: boolean; reload: () => void }) {
  return (
    <>
      <Row label="Accessibility" description={needsTitles && !info.accessibility ? TITLES_BLOCKED : READS_TITLES}>
        {info.accessibility ? <span className="settings-state found">Allowed</span> : <AllowAccessibility reload={reload} />}
      </Row>
      {info.missing.includes("automation") && (
        <Row label="Automation" description="Daybook isn’t allowed to read Chrome’s tabs (Privacy → Automation), so Meet calls can’t be recognized.">
          <button type="button" className="btn" onClick={() => void requestAccess("automation", reload)}>
            Open Privacy Settings
          </button>
        </Row>
      )}
    </>
  );
}

interface TriggerProps {
  rule: MeetingRule;
  actions: ReactNode;
  onChange: (patch: Partial<MeetingRule>) => void;
}

function Trigger({ rule, actions, onChange, call, children }: TriggerProps & { call: boolean; children: ReactNode }) {
  const title = (
    <>
      <strong>{call ? "During a" : "While"}</strong>
      <CommitInput label="Name" className="app-trigger-name" placeholder="App name" required value={rule.app} onCommit={(app) => onChange({ app })} />
      <strong>{call ? "call" : "is running"}</strong>
    </>
  );
  return (
    <LeadSwitch label={`Recognize ${rule.app} calls`} title={title} checked={rule.enabled} onChange={(enabled) => onChange({ enabled })} accessory={actions}>
      {children}
      <FormRow label="Slack status">
        <SlackStatusEditor value={rule.slack} onChange={(slack) => onChange({ slack })} />
      </FormRow>
    </LeadSwitch>
  );
}

function BuiltInTrigger({ check, ...props }: TriggerProps & { check: string }) {
  return (
    <Trigger {...props} call>
      <FormRow label="Call" hint={check} />
    </Trigger>
  );
}

function CustomTrigger(props: TriggerProps) {
  const { rule, onChange } = props;
  const choose = () => void pickApp().then((picked) => picked && onChange({ bundleId: picked.bundleId }));
  return (
    <Trigger {...props} call={false}>
      <FormRow label="macOS app" hint={rule.bundleId}>
        <button type="button" className="btn" onClick={choose}>
          Choose…
        </button>
      </FormRow>
      <FormText
        label="Window title"
        placeholder="Contains … (optional)"
        hint="Only windows on the current Space count."
        value={rule.windowTitle ?? ""}
        onCommit={(title) => onChange({ windowTitle: title.trim() || null })}
      />
      <FormRow label="Microphone">
        <Checkbox label="Only while it records from the microphone" checked={rule.needsMicrophone ?? false} onChange={(needsMicrophone) => onChange({ needsMicrophone })} />
      </FormRow>
    </Trigger>
  );
}

const APP_TRIGGERS = { title: "App triggers", anchor: "calls" };

function AppTriggers({ s, set, info, reload }: SettingsPageProps & { info: CallsInfo; reload: () => void }) {
  const rules = s.meetings.rules;
  const edit = listEditor(rules, (next) => set({ meetings: { ...s.meetings, rules: next } }));
  const checkOf = (bundleId: string) => info.builtIns.find((builtIn) => builtIn.rule.bundleId === bundleId)?.check;
  const missing = info.builtIns.filter((builtIn) => !rules.some((rule) => rule.bundleId === builtIn.rule.bundleId));
  const needsTitles = rules.some((rule) => rule.enabled && !checkOf(rule.bundleId));
  const addOther = () => void pickApp().then((picked) => picked && edit.add(newRule(picked)));
  return (
    <Group
      {...APP_TRIGGERS}
      description="Zoom, Teams and Google Meet calls are recognized directly; other apps count while they run, or while a window title contains the text. Checked every 15 seconds while you’re at the Mac; a call ends 45 seconds after it was last seen."
    >
      <Switch label="Watch apps" description="Off: no app trigger runs." checked={s.meetings.detection} onChange={(detection) => set({ meetings: { ...s.meetings, detection } })} />
      {info.detecting && <CallState info={info} reload={reload} />}
      <Permissions info={info} needsTitles={needsTitles} reload={reload} />
      <ControlRow stacked>
        {rules.map((rule, i) => {
          const check = checkOf(rule.bundleId);
          const props: TriggerProps = {
            rule,
            onChange: (patch) => edit.change(i, patch),
            actions: edit.actions(i, "Remove App"),
          };
          return check ? <BuiltInTrigger key={i} check={check} {...props} /> : <CustomTrigger key={i} {...props} />;
        })}
      </ControlRow>
      <EntryFooter>
        {missing.map((builtIn) => (
          <button key={builtIn.rule.bundleId} type="button" className="btn" onClick={() => edit.add(builtIn.rule)}>
            Add {builtIn.rule.app}
          </button>
        ))}
        <button type="button" className="btn" onClick={addOther}>
          Add Other App…
        </button>
      </EntryFooter>
    </Group>
  );
}

function StartFirst({ apps, onChange }: { apps: AppRef[]; onChange: (apps: AppRef[]) => void }) {
  const add = () =>
    void pickApp().then((picked) => {
      if (picked && !apps.some((app) => app.bundleId === picked.bundleId)) onChange([...apps, picked]);
    });
  return (
    <div className="choice-list">
      {apps.length > 0 && (
        <ul aria-label="Start first">
          {apps.map((app) => (
            <li key={app.bundleId}>
              <span className="grow-text">{app.app}</span>
              <button type="button" className="icon-btn tiny" aria-label={`Remove ${app.app}`} onClick={() => onChange(apps.filter((other) => other.bundleId !== app.bundleId))}>
                <X size="0.846rem" aria-hidden="true" />
              </button>
            </li>
          ))}
        </ul>
      )}
      <button type="button" className="btn" onClick={add}>
        Add App…
      </button>
    </div>
  );
}

function JoiningCall({ s, set }: SettingsPageProps) {
  const joining = s.joining;
  const change = (patch: Partial<Joining>) => set({ joining: { ...joining, ...patch } });
  const setApp = (service: keyof OpenWith, app: AppRef | null) => change({ openWith: { ...joining.openWith, [service]: app } });
  return (
    <Group title="Joining a Call" description="What Join does in the Daybook window and on an event’s row.">
      <Row label="Start first" description="Started in the background before the call opens, e.g. a virtual camera like Camo. An app that wasn’t running gets a moment to start." stacked>
        <StartFirst apps={joining.startFirst} onChange={(startFirst) => change({ startFirst })} />
      </Row>
      {SERVICES.map(({ service, label }) => {
        const app = joining.openWith[service];
        return (
          <Row key={service} label={label} description={app ? "A Chrome web app (like Google Meet) opens the call in its own window." : "The app macOS has for the link."}>
            <span className={app ? undefined : "muted"}>{app?.app ?? "Default"}</span>
            {app && (
              <button type="button" className="btn" onClick={() => setApp(service, null)}>
                Use Default
              </button>
            )}
            <button type="button" className="btn" aria-label={`Choose the app for ${service}`} onClick={() => void pickApp().then((picked) => picked && setApp(service, picked))}>
              Choose…
            </button>
          </Row>
        );
      })}
    </Group>
  );
}

/** Workflows → App triggers and Joining a Call. */
export function CallWorkflows(props: SettingsPageProps) {
  const { data: info, reload } = useCallsInfo();
  return (
    <>
      {info ? <AppTriggers {...props} info={info} reload={reload} /> : <Group {...APP_TRIGGERS}>{null}</Group>}
      <JoiningCall {...props} />
    </>
  );
}
