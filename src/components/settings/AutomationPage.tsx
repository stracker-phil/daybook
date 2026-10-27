import { useEffect, useState } from "react";
import { api, type AutomationInfo } from "../../api";
import { copyText } from "../../lib/clipboard";
import { useApiResult } from "../../lib/useApiResult";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { fail } from "../../state/requests";
import { Group, NumberField, Row, Switch } from "./controls";
import { PresentationsGroup } from "./PresentationsGroup";

const TEST_COMMAND = "daybook help";

const CLI_EXAMPLES = [TEST_COMMAND, "daybook status break lunch", "daybook journal open", "daybook music play Focus", "daybook routine evening", "daybook slack custom :tomato: Focus time"];

/** In the examples Rust sends; Copy puts the real token in. */
const TOKEN_PLACEHOLDER = "<token>";

/** The server starts and stops in the background, so its state is read again every few seconds. */
const POLL_MS = 3000;

/** Read again when a switch or the port changes, and every few seconds. */
function useAutomationInfo({ enabled, port, presentations }: { enabled: boolean; port: number; presentations: boolean }) {
  const result = useApiResult(`${enabled}:${port}:${presentations}`, api.automationInfo, { keepData: true });
  const { reload } = result;
  useEffect(() => {
    const timer = setInterval(reload, POLL_MS);
    return () => clearInterval(timer);
  }, [reload]);
  return result;
}

function Code({ lines }: { lines: string[] }) {
  return (
    <pre className="code-block selectable">
      {lines.map((line) => (
        <div key={line}>{line}</div>
      ))}
    </pre>
  );
}

function ServerState({ info }: { info: AutomationInfo["api"] }) {
  if (info.running) return <span className="settings-state found">Listening</span>;
  if (info.error) return <span className="settings-state failed">{info.error}</span>;
  return <span className="settings-state">Off</span>;
}

function CommandLine({ cli }: { cli: AutomationInfo["cli"] }) {
  return (
    <Group title="Command line" description="Everything the menu, the palette and the shortcuts do, from Keyboard Maestro, Shortcuts or a terminal. Answers and errors show in the terminal.">
      <Row
        label="daybook"
        description={
          <>
            <span className="mono selectable">{cli.path}</span>
            {!cli.linked && <div>Installing a release build (bun run release) links it to the installed app.</div>}
          </>
        }
      >
        <span className={`settings-state ${cli.linked ? "found" : "missing"}`}>{cli.linked ? "Linked" : "Not linked"}</span>
      </Row>
      <Row label="Test command" description={<span className="mono selectable">{TEST_COMMAND}</span>}>
        <button type="button" className="btn" onClick={() => void copyText(TEST_COMMAND, "the command")}>
          Copy
        </button>
      </Row>
      <Row label="Examples" stacked>
        <Code lines={CLI_EXAMPLES} />
      </Row>
    </Group>
  );
}

function Token({ token, onChanged }: { token: string; onChanged: () => void }) {
  const [busy, setBusy] = useState(false);
  const regenerate = () => {
    setBusy(true);
    void api
      .automationNewToken()
      .then(onChanged)
      .catch((e: unknown) => fail(e, "Couldn't make a new token"))
      .finally(() => setBusy(false));
  };
  return (
    <Row label="Token" description="Send it as “Authorization: Bearer …”. A new token stops old scripts. Stored in the Keychain.">
      <input className="input mono" type="password" readOnly aria-label="API token" value={token} />
      <button type="button" className="btn" disabled={!token} onClick={() => void copyText(token, "the token")}>
        Copy
      </button>
      <button type="button" className="btn" disabled={busy} onClick={regenerate}>
        New Token
      </button>
    </Row>
  );
}

export function AutomationPage({ s, set }: SettingsPageProps) {
  const { data: info, reload } = useAutomationInfo(s.api);
  if (!info) return null;

  return (
    <>
      <CommandLine cli={info.cli} />
      <Group title="HTTP API" description="For scripts that need an answer, e.g. “Execute Shell Script” with curl. Only this Mac can reach it, and every request needs the token.">
        <Switch
          label="Enable the HTTP API"
          description={`Actions: state, status, slack, command, show, at ${info.baseUrl}/<action>.`}
          checked={s.api.enabled}
          onChange={(enabled) => set({ api: { ...s.api, enabled } })}
        />
        <Row label="Status">
          <ServerState info={info.api} />
        </Row>
        <NumberField
          label="Port"
          description="1024–65535. Change it if another app uses this one."
          min={1024}
          max={65535}
          value={s.api.port}
          onCommit={(port) => set({ api: { ...s.api, port } })}
        />
        <Token token={info.token} onChanged={reload} />
        <Row label="Examples" stacked>
          <Code lines={info.examples} />
          <div className="button-row">
            <button type="button" className="btn" disabled={!info.token} onClick={() => void copyText(info.examples.map((line) => line.replaceAll(TOKEN_PLACEHOLDER, info.token)).join("\n"), "the examples")}>
              Copy Examples
            </button>
          </div>
        </Row>
      </Group>
      <PresentationsGroup s={s} set={set} info={info} />
    </>
  );
}
