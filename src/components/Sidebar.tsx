import type { ReactNode } from "react";
import { ChevronLeft, ChevronRight, Settings } from "lucide-react";
import { api, type HubTool, type HubView, type Status, type ToolRow } from "../api";
import { fail } from "../state/requests";
import { useStore } from "../state/store";
import { ShortcutCap } from "./Field";
import { SlackBadge } from "./SlackStatus";
import { TOOL_INFO } from "./settings/toolInfo";

function run(command: string) {
  void api.runCommand(command).catch((e) => fail(e, "Could not run the command"));
}

/** The weekday and date of the day shown; a click opens its note in Obsidian. ‹ › step to the days with a note. */
function DayHeader({ hub }: { hub: HubView }) {
  const showDay = useStore((s) => s.showDay);
  function openNote() {
    if (hub.isToday) run("daily:open");
    else api.dailyNoteOpen(hub.day).catch((e) => fail(e, "Could not open the note"));
  }
  return (
    <div className="day-header">
      <button
        type="button"
        className="day-title"
        data-stage="hub:date"
        data-stage-label={`${hub.weekday}, ${hub.date} (opens the note in Obsidian)`}
        title={hub.isToday ? "Open Today's Note in Obsidian" : "Open This Day's Note in Obsidian"} onClick={openNote}>
        <h2>{hub.weekday}</h2>
        <span className="day-date">{hub.date}</span>
      </button>
      <div className="day-arrows">
        <button type="button" aria-label="Previous Day with a Note" title="Previous Day with a Note (P)" disabled={!hub.previousDay} onClick={() => showDay(hub.previousDay)}>
          <ChevronLeft size={16} aria-hidden="true" />
        </button>
        <button type="button" aria-label="Next Day with a Note" title="Next Day with a Note (N)" disabled={!hub.nextDay} onClick={() => showDay(hub.nextDay)}>
          <ChevronRight size={16} aria-hidden="true" />
        </button>
      </div>
      {!hub.isToday && (
        <button type="button" className="back-to-today" title="Back to Today (O)" onClick={() => showDay(null)}>
          Back to Today
        </button>
      )}
    </div>
  );
}

function StatusDot({ status }: { status: Status | null }) {
  return <span className="status-dot" data-status={status ?? "none"} aria-hidden="true" />;
}

/** `icon` replaces the tool's own, e.g. the status dot. */
function ToolButton({ row, icon }: { row: ToolRow; icon?: ReactNode }) {
  const Icon = TOOL_INFO[row.id].icon;
  const command = row.command;
  const disabled = !command || !row.enabled;
  return (
    <button
      type="button"
      className={row.accent ? "si tool-row accent" : "si tool-row"}
      data-stage={`tool:${row.id}`}
      data-stage-label={row.title}
      disabled={disabled}
      onClick={() => command && run(command)}>
      {icon ?? <Icon size={16} aria-hidden="true" />}
      <span className="label">{row.title}</span>
      {row.detail && <span className="meta">{row.detail}</span>}
      <ShortcutCap shortcut={row.shortcut} />
    </button>
  );
}

function SidebarRow({ tool, hub }: { tool: HubTool; hub: HubView }) {
  if (tool.kind === "separator") return <div role="separator" className="sidebar-separator" />;
  return <ToolButton row={tool} icon={tool.id === "status" ? <StatusDot status={hub.status} /> : undefined} />;
}

/** Always dark in the Default scheme. */
export function Sidebar() {
  const hub = useStore((s) => s.hub);
  return (
    <aside className="sidebar" aria-label="Sidebar">
      <div className="sidebar-titlebar" data-tauri-drag-region />
      <div className="sidebar-body">
        {hub && <DayHeader hub={hub} />}
        <nav className="sidebar-nav" aria-label="Tools" data-stage-list="tools">
          {hub?.tools.map((tool, i) => <SidebarRow key={tool.kind === "separator" ? `separator-${i}` : tool.id} tool={tool} hub={hub} />)}
        </nav>
      </div>
      <footer className="sidebar-footer">
        {hub?.slack && <SlackBadge shown={hub.slack} />}
        <button type="button" className="icon-btn" aria-label="Settings" title="Settings (⌘,)" data-stage="hub:settings" onClick={() => run("app:settings")}>
          <Settings size={16} aria-hidden="true" />
        </button>
      </footer>
    </aside>
  );
}
