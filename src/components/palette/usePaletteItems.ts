import type { Keybindings } from "../../api";
import { activeScopes, allCommands, bindingFor, bindingLabel } from "../../commands";
import { settingsPages, useSettingsPagesVersion } from "../../registry/settingsPages";
import { useStore } from "../../state/store";
import { parsePrefix, score, type Prefix } from "./parse";

export interface Item {
  key: string;
  /** Settings or Commands. */
  section: string;
  title: string;
  /** A command's key cap. */
  keys?: string;
  /** How well it matches the typed text (`score`); sections and their items are ordered by it. */
  rank: number;
  go: () => void;
}

const MAX_ITEMS = 80;

/** Closes the palette before acting, so a command that opens it again (with a seed) wins. */
const closeThen = (run: () => void) => () => {
  useStore.getState().setPaletteOpen(false);
  run();
};

function settingsItems(prefix: Prefix, needle: string): Item[] {
  if (prefix !== null || !needle) return [];
  return settingsPages().map((page) => {
    const title = `Settings: ${page.label}`;
    return {
      key: `settings:${page.id}`,
      section: "Settings",
      title,
      rank: score([title, page.label], needle),
      go: closeThen(() => useStore.getState().openDialog({ type: "settings", page: page.id })),
    };
  });
}

/** The active scopes' enabled commands. */
function commandItems(custom: Keybindings, needle: string): Item[] {
  const scopes = activeScopes();
  return allCommands()
    .filter((c) => c.id !== "palette.toggle" && scopes.includes(c.scope) && (c.when?.() ?? true))
    .map((c) => ({
      key: `cmd:${c.id}`,
      section: "Commands",
      title: c.title,
      keys: bindingLabel(bindingFor(c.id, custom)),
      rank: score([c.title], needle),
      go: closeThen(() => void c.run()),
    }));
}

/** Each section's matches best first, the sections by their best match; ties keep the providers' order. */
function arrange(sections: Item[][]): Item[] {
  return sections
    .map((items) => items.filter((i) => i.rank > 0).sort((a, b) => b.rank - a.rank))
    .filter((items) => items.length > 0)
    .sort((a, b) => b[0].rank - a[0].rank)
    .flat()
    .slice(0, MAX_ITEMS);
}

/** The palette's results for `query`, grouped in sections and ranked by how well they match. */
export function usePaletteItems(query: string) {
  const custom = useStore((s) => s.keybindings);
  useSettingsPagesVersion();
  const { prefix, needle } = parsePrefix(query);
  const items = arrange([settingsItems(prefix, needle), commandItems(custom, needle)]);
  return { items, prefix, needle };
}
