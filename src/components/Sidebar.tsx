import { ChevronLeft, ChevronRight, Settings } from "lucide-react";
import { api, type HubTool, type HubView, type ToolRow } from "../api";
import { fail } from "../state/requests";
import { useStore } from "../state/store";
import { ShortcutCap } from "./Field";
import { TOOL_INFO } from "./settings/toolInfo";

function run(command: string) {
  void api.runCommand(command).catch((e) => fail(e, "Could not run the command"));
}

/** The weekday and date of the day shown; a click opens its note in Obsidian. */
function DayHeader({ hub }: { hub: HubView }) {
  function openNote() {
    if (hub.isToday) run("daily:open");
    else void api.dailyNoteOpen(hub.day).catch((e) => fail(e, "Could not open the note"));
  }
  return (
    <div className="day-header">
      <button type="button" className="day-title" title={hub.isToday ? "Open Today's Note in Obsidian" : "Open This Day's Note in Obsidian"} onClick={openNote}>
        <h2>{hub.weekday}</h2>
        <span className="day-date">{hub.date}</span>
      </button>
      <div className="day-arrows">
        <button type="button" className="icon-btn tiny" aria-label="Previous Day with a Note" title="Previous Day with a Note" disabled={!hub.previousDay}>
          <ChevronLeft size="1.077rem" aria-hidden="true" />
        </button>
        <button type="button" className="icon-btn tiny" aria-label="Next Day with a Note" title="Next Day with a Note" disabled={!hub.nextDay}>
          <ChevronRight size="1.077rem" aria-hidden="true" />
        </button>
      </div>
    </div>
  );
}

function ToolButton({ row }: { row: ToolRow }) {
  const Icon = TOOL_INFO[row.id].icon;
  const command = row.command;
  const disabled = !command || !row.enabled;
  return (
    <button type="button" className={row.accent ? "si tool-row accent" : "si tool-row"} disabled={disabled} onClick={() => command && run(command)}>
      <Icon size="1.077rem" aria-hidden="true" />
      <span className="label">{row.title}</span>
      <ShortcutCap shortcut={row.shortcut} />
    </button>
  );
}

function SidebarRow({ tool }: { tool: HubTool }) {
  return tool.kind === "separator" ? <div role="separator" className="sidebar-separator" /> : <ToolButton row={tool} />;
}

/** Always dark in the Default scheme. */
export function Sidebar() {
  const hub = useStore((s) => s.hub);
  return (
    <aside className="sidebar" aria-label="Sidebar">
      <div className="sidebar-titlebar" data-tauri-drag-region />
      <div className="sidebar-body">
        {hub && <DayHeader hub={hub} />}
        <nav className="sidebar-nav" aria-label="Tools">
          {hub?.tools.map((tool, i) => <SidebarRow key={tool.kind === "separator" ? `separator-${i}` : tool.id} tool={tool} />)}
        </nav>
      </div>
      <footer className="sidebar-footer">
        <span className="spacer" />
        <button type="button" className="icon-btn" aria-label="Settings" title="Settings (⌘,)" onClick={() => run("app:settings")}>
          <Settings size="1.077rem" aria-hidden="true" />
        </button>
      </footer>
    </aside>
  );
}
