import { BookOpen, CircleDot, FolderOpen, LayoutDashboard, Layers, Link2, Minus, Music, NotebookPen, Search, Ticket, Timer, Video, type LucideIcon } from "lucide-react";
import type { ToolId } from "../../api";

export interface ToolInfo {
  label: string;
  description: string;
  icon: LucideIcon;
}

/** Each tool's name, what it does and its icon, in Settings → General → Tools and the Daybook window's sidebar. */
export const TOOL_INFO: Record<ToolId, ToolInfo> = {
  context: { label: "Context", description: "The current context; a click switches it.", icon: Layers },
  status: { label: "Status", description: "Working, Break or Off; a click starts or ends a break.", icon: CircleDot },
  activity: { label: "Activity", description: "What you work on, for a project.", icon: Timer },
  call: { label: "Call", description: "The call you are in, while there is one.", icon: Video },
  separator1: { label: "Separator", description: "A line between tools.", icon: Minus },
  journal: { label: "Journal", description: "Answer today's journal questions.", icon: BookOpen },
  hub: { label: "Daybook", description: "Opens the Daybook window.", icon: LayoutDashboard },
  search: { label: "Search vault", description: "Find notes and preview them.", icon: Search },
  jira: { label: "Open Jira issue", description: "Open an issue by number, key or link.", icon: Ticket },
  quickNotes: { label: "Quick notes", description: "Write into a section of today's notes in a small window.", icon: NotebookPen },
  separator2: { label: "Separator", description: "A line between tools.", icon: Minus },
  links: { label: "Links", description: "Open a link: the ones that match now first, then all.", icon: Link2 },
  projects: { label: "Projects", description: "Show a project in Project Launcher.", icon: FolderOpen },
  music: { label: "Music", description: "Pick a playlist, pause or change the volume.", icon: Music },
};
