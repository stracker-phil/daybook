import { useEffect, useState } from "react";
import { api, type SlackInfo } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { fail } from "../../state/requests";
import { Group, Row } from "./controls";

/** Whether a token is stored, read again when the connection changes. */
function useSlackInfo() {
  const result = useApiResult("slack", api.slackInfo, { keepData: true });
  const { reload } = result;
  useEffect(() => {
    const unlisten = api.onSlackChanged(reload);
    return () => void unlisten.then((fn) => fn()).catch(() => {});
  }, [reload]);
  return result;
}

/** Runs one Slack action at a time; a failure shows as a toast. */
function useBusyAction(): [boolean, (action: () => Promise<unknown>, failure: string) => void] {
  const [busy, setBusy] = useState(false);
  const run = (action: () => Promise<unknown>, failure: string) => {
    setBusy(true);
    void action()
      .catch((e: unknown) => fail(e, failure))
      .finally(() => setBusy(false));
  };
  return [busy, run];
}

const TOKEN_HINT = "A Slack user token (xoxp-…) with users.profile:write, users:write for presence and chat:write for the channel. Stored in the Keychain.";

function Connected({ info }: { info: SlackInfo }) {
  const [busy, run] = useBusyAction();
  const identity = info.identity;
  const description = identity ? `${identity.user} in ${identity.team}` : "The token is stored in the Keychain.";
  return (
    <>
      <Row label="Connection" description={description}>
        <span className="settings-state found">Connected</span>
      </Row>
      <Row label="">
        <div className="button-row">
          <button type="button" className="btn" disabled={busy} onClick={() => run(api.slackTest, "Slack connection failed")}>
            Test Connection
          </button>
          <button type="button" className="btn btn-danger" disabled={busy} onClick={() => run(api.slackDisconnect, "Couldn't remove the token")}>
            Remove Token
          </button>
        </div>
      </Row>
    </>
  );
}

function Connect() {
  const [token, setToken] = useState("");
  const [busy, run] = useBusyAction();
  const canConnect = token.trim() !== "" && !busy;
  const connect = () => {
    if (!canConnect) return;
    run(async () => {
      await api.slackConnect(token);
      setToken("");
    }, "Couldn't connect Slack");
  };
  return (
    <Row label="User token" description={TOKEN_HINT}>
      <input
        className="input"
        type="password"
        aria-label="User token"
        placeholder="xoxp-…"
        autoComplete="off"
        value={token}
        onChange={(e) => setToken(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && connect()}
      />
      <button type="button" className="btn btn-primary" disabled={!canConnect} onClick={connect}>
        Connect
      </button>
    </Row>
  );
}

export function SlackPage() {
  const { data: info } = useSlackInfo();
  if (!info) return null;
  return (
    <Group description="Workflows set your Slack status and presence and post to the channel (Settings → Workflows), in contexts with the Slack feature.">
      {info.connected ? <Connected info={info} /> : <Connect />}
    </Group>
  );
}
