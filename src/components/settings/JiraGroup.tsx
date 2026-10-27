import type { Jira } from "../../api";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { Group, TextField } from "./controls";

const DEFAULT_PREFIX = "QRS";

/** Where issue 6805 would open; null without a URL. */
function exampleUrl({ baseUrl, defaultPrefix }: Jira): string | null {
  if (!baseUrl) return null;
  return `${baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`}${defaultPrefix || DEFAULT_PREFIX}-6805`;
}

/** Connections → Jira. Rust checks both fields on save; a rejected value shows as the save's error. */
export function JiraGroup({ s, set }: SettingsPageProps) {
  const jira = s.jira;
  const example = exampleUrl(jira);
  return (
    <Group title="Jira" anchor="jira" description="Open Jira Issue opens an issue by number, key or link.">
      <TextField
        label="Issue URL"
        description={example ? `Issue 6805 opens ${example}` : "The part of an issue link before the key."}
        placeholder="https://example.atlassian.net/browse/"
        wide
        value={jira.baseUrl}
        onCommit={(baseUrl) => set({ jira: { ...jira, baseUrl: baseUrl.trim() } })}
      />
      <TextField
        label="Default project"
        description="Used when you type only a number."
        placeholder={DEFAULT_PREFIX}
        value={jira.defaultPrefix}
        onCommit={(defaultPrefix) => set({ jira: { ...jira, defaultPrefix: defaultPrefix.trim() } })}
      />
    </Group>
  );
}
