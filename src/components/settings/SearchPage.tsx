import { api, type Search, type SearchScope } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { ControlRow, EntryFooter, Group, Row } from "./controls";
import { CommitInput, joinList, parseList } from "./fields";
import { listEditor } from "./listEditor";

/** `scope<n>` from the count up, skipping names in use. */
function freeScopeName(scopes: SearchScope[]): string {
  let n = scopes.length + 1;
  while (scopes.some((scope) => scope.name === `scope${n}`)) n += 1;
  return `scope${n}`;
}

/** Scopes name folder sets for `@name` in search; Rust resolves them and reports what doesn't add up. */
export function SearchPage({ s, set }: SettingsPageProps) {
  const { scopes, exclude } = s.search;
  // Settings arrive here once saved, so the problems are those of the shown scopes.
  const problems = useApiResult(JSON.stringify(scopes), api.searchScopes).data?.problems ?? [];
  const setSearch = (patch: Partial<Search>) => set({ search: { ...s.search, ...patch } });
  const edit = listEditor(scopes, (next) => setSearch({ scopes: next }));

  return (
    <>
      <Group title="Scopes" description="Type @name in search to narrow it. Folders end with / (e.g. 2-Work/); other entries include another scope; / is the whole vault.">
        {scopes.map((scope, i) => (
          <ControlRow key={i} className="search-scope-row">
            <CommitInput label="Scope name" className="search-scope-name" required value={scope.name} onCommit={(name) => edit.change(i, { name })} />
            <CommitInput
              label="Folders and scopes"
              className="grow"
              placeholder="2-Work/, journal"
              value={joinList(scope.entries)}
              onCommit={(text) => edit.change(i, { entries: parseList(text) })}
            />
            {edit.actions(i, "Remove Scope")}
          </ControlRow>
        ))}
        <EntryFooter>
          <button type="button" className="btn" onClick={() => edit.add({ name: freeScopeName(scopes), entries: [] })}>
            Add Scope
          </button>
        </EntryFooter>
      </Group>
      {problems.length > 0 && <p className="settings-alert" role="alert">{problems.join(" ")}</p>}
      <Group title="Never search">
        <Row label="Folders" description="Comma-separated, e.g. _/, .trash/">
          <CommitInput label="Excluded folders" className="search-exclude" value={joinList(exclude)} onCommit={(text) => setSearch({ exclude: parseList(text) })} />
        </Row>
      </Group>
    </>
  );
}
