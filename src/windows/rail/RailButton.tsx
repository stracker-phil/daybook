import { BookOpen, FolderOpen, Link2, Music, NotebookPen, Search, Sun, Ticket, Timer, Video, type LucideIcon } from "lucide-react";
import { api, type RailTool, type Status, type ToolId } from "../../api";

const ICONS: Partial<Record<ToolId, LucideIcon>> = {
  activity: Timer,
  call: Video,
  journal: BookOpen,
  hub: Sun,
  search: Search,
  jira: Ticket,
  quickNotes: NotebookPen,
  links: Link2,
  projects: FolderOpen,
  music: Music,
};

function Face({ tool, status }: { tool: RailTool; status: Status | null }) {
  if (tool.id === "context") return <span className="rail-text">{tool.badge ?? tool.label.slice(0, 3)}</span>;
  if (tool.id === "status") return <span className="rail-status-dot" data-status={status ?? "none"} />;
  const Icon = ICONS[tool.id] ?? Sun;
  return <Icon className="rail-icon" aria-hidden="true" />;
}

/** One tool: its button and, with captions on, a short line under it. */
export function RailButton({ tool, status, hovered, captions }: { tool: RailTool; status: Status | null; hovered: boolean; captions: boolean }) {
  return (
    <div className="rail-cell">
      <button
        type="button"
        className={tool.id === "context" ? "rail-btn rail-label" : "rail-btn"}
        aria-label={tool.ariaLabel}
        data-tool={tool.id}
        data-hover={hovered || undefined}
        data-filled={tool.filled || undefined}
        data-active={tool.accent || undefined}
        onClick={() => void api.runCommand(tool.command)}
      >
        <Face tool={tool} status={status} />
      </button>
      {captions && tool.caption && <span className="rail-caption">{tool.caption}</span>}
    </div>
  );
}
