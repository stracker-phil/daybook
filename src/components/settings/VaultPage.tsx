import { api } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { fail } from "../../state/requests";
import { Group, Row, TextField } from "./controls";

/** `/Users/name/…` → `~/…`. */
const tildePath = (path: string) => path.replace(/^\/Users\/[^/]+/, "~");

const dailyNotePattern = ({ folder, format }: { folder: string; format: string }) => (folder ? `${folder}/${format}` : format);

function State({ found, yes, no }: { found: boolean; yes: string; no: string }) {
  return <span className={`settings-state ${found ? "found" : "missing"}`}>{found ? yes : no}</span>;
}

export function VaultPage({ s, set }: SettingsPageProps) {
  const vault = s.vault;
  const vaultInfoKey = JSON.stringify([vault.path, vault.obsidianCliPath, s.dailyNote]);
  const info = useApiResult(vaultInfoKey, api.vaultInfo, { keepData: true }).data;

  const choose = async () => {
    try {
      const path = await api.chooseFolder();
      if (path) set({ vault: { ...vault, path } });
    } catch (e) {
      fail(e, "Could not choose the vault");
    }
  };

  const dailyNotes = info?.dailyNotes;
  const cliPath = info?.obsidianCli;
  const cliDescription = cliPath ? <span className="mono selectable">{tildePath(cliPath)}</span> : info ? "Not found." : undefined;
  return (
    <>
      <Group>
        <Row label="Vault folder" description={vault.path ? <span className="mono selectable">{tildePath(vault.path)}</span> : "Choose the folder of your Obsidian vault."}>
          <button type="button" className={vault.path ? "btn" : "btn btn-primary"} onClick={() => void choose()}>
            {vault.path ? "Change…" : "Choose…"}
          </button>
        </Row>
        {vault.path && (
          <>
            <TextField
              label="Vault name"
              description="As Obsidian shows it; used for obsidian:// links and the Obsidian CLI."
              value={vault.name}
              onCommit={(name) => {
                if (name.trim()) set({ vault: { ...vault, name: name.trim() } });
              }}
            />
            {dailyNotes && (
              <Row
                label="Daily notes"
                description={dailyNotes.fromObsidian ? "From Obsidian's Daily notes settings." : "Fallback (Settings → Daily Note), because .obsidian/daily-notes.json wasn't found."}
              >
                <span className="mono selectable">{dailyNotePattern(dailyNotes)}</span>
              </Row>
            )}
            {info && (
              <Row label="Today's note" description={info.todayNote ? <span className="mono selectable">{tildePath(info.todayNote)}</span> : undefined}>
                <State found={info.todayNoteExists} yes="Found" no="Not created yet" />
              </Row>
            )}
          </>
        )}
      </Group>
      <Group title="Obsidian CLI" description="Creates today's note when it's missing, so your template runs. Turn it on in Obsidian → Settings → General → Command line interface.">
        <Row label="Status" description={cliDescription}>
          {info && <State found={!!info.obsidianCli} yes="Found" no="Missing" />}
        </Row>
        <TextField
          label="Custom path"
          description="Leave empty to find it automatically."
          placeholder="/usr/local/bin/obsidian"
          value={vault.obsidianCliPath}
          onCommit={(obsidianCliPath) => set({ vault: { ...vault, obsidianCliPath: obsidianCliPath.trim() } })}
        />
      </Group>
    </>
  );
}
