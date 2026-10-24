import { useState } from "react";
import { RotateCcw, Search, X } from "lucide-react";
import { api } from "../../api";
import { allCommands, bindingFor, bindingFromEvent, bindingLabel, conflicts, scopeLabel, type Command } from "../../commands";
import type { Keybindings } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { useStore } from "../../state/store";
import { InsetField } from "../Field";
import { Group, Row } from "./controls";
import { GlobalShortcuts, MenuShortcuts } from "./GlobalShortcuts";
import { ShortcutRecorder } from "./ShortcutRecorder";

function RemoveShortcut({ title, onRemove }: { title: string; onRemove: () => void }) {
  return (
    <button type="button" className="icon-btn tiny" aria-label={`Remove shortcut for ${title}`} title="Remove shortcut" onClick={onRemove}>
      <X size="0.846rem" />
    </button>
  );
}

const matchesCommand = (c: Command, needle: string) => !needle || c.title.toLowerCase().includes(needle) || c.id.toLowerCase().includes(needle);

function withoutBinding(bindings: Keybindings, id: string): Keybindings {
  const rest = { ...bindings };
  delete rest[id];
  return rest;
}

function WindowShortcutRow({ command: c, bindings, save }: { command: Command; bindings: Keybindings; save: (bindings: Keybindings) => void }) {
  const overridden = Object.hasOwn(bindings, c.id);
  const binding = bindingFor(c.id, bindings);
  const set = (next: string | null) => save({ ...bindings, [c.id]: next });
  return (
    <Row label={c.title} description={scopeLabel(c.scope)}>
      <div className="shortcut-control">
        {overridden && (
          <button type="button" className="icon-btn tiny" aria-label={`Reset ${c.title}`} title="Reset to default" onClick={() => save(withoutBinding(bindings, c.id))}>
            <RotateCcw size="0.846rem" />
          </button>
        )}
        {binding && <RemoveShortcut title={c.title} onRemove={() => set(null)} />}
        <ShortcutRecorder value={binding} parse={bindingFromEvent} label={bindingLabel} onChange={set} name={c.title} />
      </div>
    </Row>
  );
}

/** The windows' own keys (`keybindings.json`), in page notation (`Mod+Shift+K`). */
function WindowShortcuts() {
  const bindings = useStore((s) => s.keybindings);
  const saveKeybindings = useStore((s) => s.saveKeybindings);
  const save = (next: Keybindings) => void saveKeybindings(next);
  const [search, setSearch] = useState("");
  const needle = search.trim().toLowerCase();
  const list = allCommands().filter((c) => matchesCommand(c, needle));
  const clashes = conflicts(bindings);
  return (
    <>
      <InsetField icon={<Search size="1.077rem" />} className="settings-search">
        <input aria-label="Search shortcuts" placeholder="Search commands" value={search} onChange={(e) => setSearch(e.target.value)} />
      </InsetField>
      {clashes.length > 0 && (
        <div className="shortcut-conflicts" role="alert">
          {clashes.map((c) => (
            <div key={`${c.a.id}:${c.b.id}`}>
              {bindingLabel(c.binding)} is used by both “{c.a.title}” and “{c.b.title}”.
            </div>
          ))}
        </div>
      )}
      <Group title="In the windows">
        {list.length === 0 && <div className="settings-row muted">No matching commands.</div>}
        {list.map((c) => (
          <WindowShortcutRow key={c.id} command={c} bindings={bindings} save={save} />
        ))}
      </Group>
    </>
  );
}

/** Global shortcuts first, then the app menu's fixed keys, then the windows' own keys. */
export function ShortcutsPage({ s, set }: SettingsPageProps) {
  const overview = useApiResult("shortcuts", api.shortcutOverview).data;
  return (
    <>
      {overview && <GlobalShortcuts overview={overview} s={s} set={set} />}
      {overview && overview.menu.length > 0 && <MenuShortcuts menu={overview.menu} />}
      <WindowShortcuts />
    </>
  );
}
