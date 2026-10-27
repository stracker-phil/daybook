/** The core settings pages. Modules register their own pages from their install function. */
import { Activity, BellRing, Bot, BookOpen, FolderKanban, Keyboard, Layers, Link2, ListChecks, Music, NotebookPen, Palette, Plug, Search, SlidersHorizontal, Workflow } from "lucide-react";
import { registerSettingsPage } from "../../registry/settingsPages";
import { AppearancePage } from "./AppearancePage";
import { AutomationPage } from "./AutomationPage";
import { ConnectionsPage } from "./ConnectionsPage";
import { ContextsPage } from "./ContextsPage";
import { DailyNotePage } from "./DailyNotePage";
import { GeneralPage } from "./GeneralPage";
import { JournalPage } from "./JournalPage";
import { LinksPage } from "./LinksPage";
import { MusicPage } from "./MusicPage";
import { ProjectsPage } from "./ProjectsPage";
import { RemindersPage } from "./RemindersPage";
import { RoutinesPage } from "./RoutinesPage";
import { SearchPage } from "./SearchPage";
import { ShortcutsPage } from "./ShortcutsPage";
import { StatusPage } from "./StatusPage";
import { WorkflowsPage } from "./WorkflowsPage";

/** Ids, order and sections match Rust's `window::settings::PAGES` and `SECTIONS`, which `settings:<page>` and the palette read. */
export function registerCoreSettingsPages() {
  registerSettingsPage({
    id: "connections",
    group: "Setup",
    label: "Connections",
    icon: Plug,
    order: 0,
    intro: (
      <>
        What Daybook works with: your <strong>Obsidian vault</strong>, your <strong>calendar</strong>, <strong>Slack</strong>, Jira and <strong>TimeTracker</strong>.
      </>
    ),
    sections: [
      { id: "vault", title: "Vault" },
      { id: "calendar", title: "Calendar" },
      { id: "slack", title: "Slack" },
      { id: "jira", title: "Jira" },
      { id: "timetracker", title: "TimeTracker" },
    ],
    Body: ConnectionsPage,
  });
  registerSettingsPage({ id: "contexts", group: "Setup", label: "Contexts", icon: Layers, order: 10, intro: "Each context turns features on and off, has its colour and work hours, and can become current by itself when they start.", Body: ContextsPage });
  registerSettingsPage({ id: "projects", group: "Setup", label: "Projects", icon: FolderKanban, order: 20, intro: "What you work on: offered for meetings and the Projects tool in the contexts you pick.", Body: ProjectsPage });
  registerSettingsPage({ id: "daily-note", group: "Obsidian", label: "Daily Note", icon: NotebookPen, order: 0, intro: "Where Daybook writes into today's note. Sections are created in this order when Daybook first writes to them; your template only needs the title and the notes heading.", Body: DailyNotePage });
  registerSettingsPage({ id: "routines", group: "Obsidian", label: "Routines", icon: ListChecks, order: 5, intro: "Checklists you come back to during the day. Opening one adds its items to today's note's plan section; ticking one writes [x].", Body: RoutinesPage });
  registerSettingsPage({ id: "journal", group: "Obsidian", label: "Journal", icon: BookOpen, order: 10, intro: "The journal's question groups. Each group is a callout in today's note's Journal section; only answered questions are written.", Body: JournalPage });
  registerSettingsPage({
    id: "search",
    group: "Obsidian",
    label: "Search",
    icon: Search,
    order: 15,
    intro: (
      <>
        Search finds notes in your <strong>Obsidian vault</strong> and previews them; Enter opens the note in Obsidian.
      </>
    ),
    Body: SearchPage,
  });
  registerSettingsPage({ id: "workflows", group: "Workflows", label: "Workflows", icon: Workflow, order: 0, sections: [{ id: "calls", title: "Calls" }], intro: "What Daybook does when something happens: each workflow is a trigger and what follows from it.", Body: WorkflowsPage });
  registerSettingsPage({ id: "reminders", group: "Workflows", label: "Reminders", icon: BellRing, order: 10, intro: "The top of the Daybook window shows one reminder: the first rule in this list that applies now.", Body: RemindersPage });
  registerSettingsPage({
    id: "links",
    group: "Workflows",
    label: "Links",
    icon: Link2,
    order: 30,
    intro: (
      <>
        A link is a vault path (opens in <strong>Obsidian</strong>) or a URL. It can show always, only in some contexts, on some days and times, or around{" "}
        <strong>calendar events</strong> with a matching title.
      </>
    ),
    Body: LinksPage,
  });
  registerSettingsPage({
    id: "music",
    group: "Workflows",
    label: "Music",
    icon: Music,
    order: 40,
    intro: (
      <>
        What Daybook can play in <strong>Apple Music</strong>: pick it from the rail, the menu bar or the overlay. Daybook can also turn the music down or pause it
        for <strong>calls</strong> and <strong>breaks</strong>.
      </>
    ),
    Body: MusicPage,
  });
  registerSettingsPage({ id: "general", group: "System", label: "General", icon: SlidersHorizontal, order: 0, sections: [{ id: "tools", title: "Tools" }], Body: GeneralPage });
  registerSettingsPage({ id: "appearance", group: "System", label: "Appearance", icon: Palette, order: 10, sections: [{ id: "rail", title: "Rail" }], Body: AppearancePage });
  registerSettingsPage({ id: "shortcuts", group: "System", label: "Shortcuts", icon: Keyboard, order: 20, Body: ShortcutsPage });
  registerSettingsPage({ id: "automation", group: "System", label: "Automation", icon: Bot, order: 30, intro: "Run Daybook's actions and commands from scripts: the daybook command, daybook:// links and a local HTTP API.", Body: AutomationPage });
  registerSettingsPage({ id: "status", group: "System", label: "Status", icon: Activity, order: 40, intro: "Whether Daybook's connections work, and what it sent where and why.", Body: StatusPage });
}
