import { Link2 } from "lucide-react";
import { api, type LinkItem } from "../../api";
import { useApiResult } from "../../lib/useApiResult";
import { ShortcutListView, type CommandRow } from "./ShortcutListView";

const rowsOf = (links: LinkItem[], group: string): CommandRow[] => links.map((link) => ({ key: `${group}:${link.target}`, title: link.title, group, command: link.command }));

/** The links that show now, then every other link. */
export function LinksView() {
  const view = useApiResult("links", api.linksView);
  const rows = view.data ? [...rowsOf(view.data.now, "Now"), ...rowsOf(view.data.more, "More links")] : null;
  return (
    <ShortcutListView
      icon={Link2}
      inputLabel="Link"
      placeholder="Open a link…"
      label="Links"
      rows={rows}
      emptyText="No links yet. Add them in Settings → Links."
      noMatchText="No matching links."
      hints="↑↓ link · ↵ open · ⌘1–9 open · esc close"
      loadError={view.error?.message ?? null}
    />
  );
}
