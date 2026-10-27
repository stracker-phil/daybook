import { api, type AutomationInfo } from "../../api";
import { copyText } from "../../lib/clipboard";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { fail } from "../../state/requests";
import { Group, Row, Switch } from "./controls";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

/** How long ago `time` (epoch ms) was, in words; a date after a day. */
export function ago(time: number, now = Date.now()): string {
  const elapsed = now - time;
  if (elapsed < MINUTE) return "just now";
  if (elapsed < 2 * MINUTE) return "a minute ago";
  if (elapsed < HOUR) return `${Math.floor(elapsed / MINUTE)} minutes ago`;
  if (elapsed < 2 * HOUR) return "an hour ago";
  if (elapsed < 24 * HOUR) return `${Math.floor(elapsed / HOUR)} hours ago`;
  return new Date(time).toLocaleString([], { dateStyle: "medium", timeStyle: "short" });
}

function PresentationsState({ info, on }: { info: AutomationInfo; on: boolean }) {
  if (info.stageFile) return <span className="settings-state found">Discoverable</span>;
  if (!on) return <span className="settings-state">Off</span>;
  if (info.api.error) return <span className="settings-state failed">{info.api.error}</span>;
  return <span className="settings-state">Off (needs the HTTP API)</span>;
}

/** Settings → Automation: whether a presenter app such as Stage may find and drive Daybook. */
export function PresentationsGroup({ s, set, info }: SettingsPageProps & { info: AutomationInfo }) {
  const reveal = () => void api.stageRevealFile().catch((e: unknown) => fail(e, "Couldn’t show the file"));
  return (
    <Group
      title="Presentations"
      description="Lets a presenter app such as Stage highlight parts of Daybook and click through it for a demo. Only apps on this Mac can connect; they use the HTTP API’s port and token above."
    >
      <Switch
        label="Allow presentations"
        description="A presenter finds Daybook through a file that exists while this is on and the HTTP API runs."
        checked={s.api.presentations}
        onChange={(presentations) => set({ api: { ...s.api, presentations } })}
      />
      <Row label="Status">
        <PresentationsState info={info} on={s.api.presentations} />
      </Row>
      {info.stageFile && (
        <Row label="Discovery file" stacked>
          <pre className="code-block selectable">{info.stageFile}</pre>
          <div className="button-row">
            <button type="button" className="btn" onClick={reveal}>
              Show in Finder
            </button>
          </div>
        </Row>
      )}
      {s.api.presentations && s.api.enabled && (
        <Row label="Register with Stage" description={<span className="mono selectable">{info.registerCommand}</span>}>
          <button type="button" className="btn" onClick={() => void copyText(info.registerCommand, "the command")}>
            Copy
          </button>
        </Row>
      )}
      <Row label="Last presenter request">
        <span className="o2">{info.lastPresenterRequest === null ? "None since Daybook started" : ago(info.lastPresenterRequest)}</span>
      </Row>
    </Group>
  );
}
