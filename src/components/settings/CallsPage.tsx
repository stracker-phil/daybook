import { X } from "lucide-react";
import { api, type AppRef, type CallsInfo, type Joining, type MeetingRule, type Missing, type OpenWith } from "../../api";
import { fail } from "../../state/requests";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, Row, Switch, TextField } from "./controls";
import { ItemActions, moveItem, replaceItem } from "./fields";
import { SlackStatusField } from "./SlackStatusField";
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

function Permissions({ info, needsTitles, reload }: { info: CallsInfo; needsTitles: boolean; reload: () => void }) {
  return (
    <>
      {needsTitles && !info.accessibility && (
        <Row label="Accessibility" description="Other apps’ window titles need Accessibility, so apps with a window title can’t be recognized yet.">
          <AllowAccessibility reload={reload} />
        </Row>
      )}
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

function AppTrigger({ rule, check, onChange }: { rule: MeetingRule; check: string | undefined; onChange: (patch: Partial<MeetingRule>) => void }) {
  const choose = () => void pickApp().then((picked) => picked && onChange({ bundleId: picked.bundleId }));
  return (
    <>
      <Switch label="Recognize calls" checked={rule.enabled} onChange={(enabled) => onChange({ enabled })} />
      <TextField label="Name" description="Shown for the call, e.g. in the menu bar." required value={rule.app} onCommit={(app) => onChange({ app: app.trim() })} />
      {check ? (
        <Row label="Call" description={check} />
      ) : (
        <>
          <Row label="macOS app" description={rule.bundleId}>
            <button type="button" className="btn" onClick={choose}>
              Choose…
            </button>
          </Row>
          <TextField
            label="Window title"
            description="Counts only while a window’s title contains this; empty: while the app runs."
            placeholder="Contains … (optional)"
            value={rule.windowTitle ?? ""}
            onCommit={(title) => onChange({ windowTitle: title.trim() || null })}
          />
        </>
      )}
      <SlackStatusField description="Shown while the call runs, in contexts with Slack." value={rule.slack} onChange={(slack) => onChange({ slack })} />
    </>
  );
}

function AppTriggers({ s, set, info, reload }: SettingsPageProps & { info: CallsInfo; reload: () => void }) {
  const rules = s.meetings.rules;
  const save = (next: MeetingRule[]) => set({ meetings: { ...s.meetings, rules: next } });
  const checkOf = (bundleId: string) => info.builtIns.find((builtIn) => builtIn.rule.bundleId === bundleId)?.check;
  const missing = info.builtIns.filter((builtIn) => !rules.some((rule) => rule.bundleId === builtIn.rule.bundleId));
  const needsTitles = rules.some((rule) => rule.enabled && !checkOf(rule.bundleId));
  const move = (from: number, to: number) => save(moveItem(rules, from, to));
  const remove = (index: number) => save(rules.filter((_, j) => j !== index));
  const addOther = () => void pickApp().then((picked) => picked && save([...rules, newRule(picked)]));
  return (
    <>
      <Group
        title="App triggers"
        description="Zoom, Teams and Google Meet calls are recognized directly, on any Space; other apps count while they run (and, if set, show a window whose title contains the text). Checked every 15 seconds while you’re at the Mac; a call ends 45 seconds after it was last seen."
      >
        <Switch label="Watch apps" description="Off: no app trigger runs." checked={s.meetings.detection} onChange={(detection) => set({ meetings: { ...s.meetings, detection } })} />
        {info.detecting && <CallState info={info} reload={reload} />}
        <Permissions info={info} needsTitles={needsTitles} reload={reload} />
      </Group>
      {rules.map((rule, i) => (
        <Group key={i} title={rule.app}>
          <AppTrigger rule={rule} check={checkOf(rule.bundleId)} onChange={(patch) => save(replaceItem(rules, i, patch))} />
          <Row label="">
            <ItemActions index={i} count={rules.length} onMove={move} onRemove={() => remove(i)} removeLabel="Remove App" />
          </Row>
        </Group>
      ))}
      <div className="button-row">
        {missing.map((builtIn) => (
          <button key={builtIn.rule.bundleId} type="button" className="btn" onClick={() => save([...rules, builtIn.rule])}>
            Add {builtIn.rule.app}
          </button>
        ))}
        <button type="button" className="btn" onClick={addOther}>
          Add Other App…
        </button>
      </div>
    </>
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

export function CallsPage(props: SettingsPageProps) {
  const { data: info, reload } = useCallsInfo();
  return (
    <>
      {info && <AppTriggers {...props} info={info} reload={reload} />}
      <JoiningCall {...props} />
    </>
  );
}

/** Rendered on Settings → General. */
export function AccessibilityRow() {
  const { data: info, reload } = useCallsInfo();
  if (!info) return null;
  return (
    <Row label="Accessibility" description="Lets app triggers read other apps’ window titles (Settings → Calls).">
      {info.accessibility ? (
        <span className="settings-state found">Allowed</span>
      ) : (
        <AllowAccessibility reload={reload} />
      )}
    </Row>
  );
}
