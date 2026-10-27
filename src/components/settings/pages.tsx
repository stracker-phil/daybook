/** The core settings pages. Modules register their own pages from their install function. */
import { BellRing, Bot, BookOpen, CalendarDays, FolderKanban, FolderOpen, Keyboard, Layers, LayoutList, Link2, ListChecks, MessageSquare, Music, NotebookPen, Palette, PanelRight, Power, Search, Ticket, Timer, Video, Workflow } from "lucide-react";
import { TEXT_SIZES, type Scheme } from "../../api";
import { schemeLook } from "../../lib/theme";
import { registerSettingsPage, type SettingsPageProps } from "../../registry/settingsPages";
import { AccentSwatches } from "../Accent";
import { Group, Row, Segmented } from "./controls";
import { LaunchAtLoginSwitch } from "./LaunchAtLoginSwitch";
import { AutomationPage } from "./AutomationPage";
import { AccessibilityRow, CallsPage } from "./CallsPage";
import { CalendarPage } from "./CalendarPage";
import { ContextsPage } from "./ContextsPage";
import { DailyNotePage } from "./DailyNotePage";
import { JiraPage } from "./JiraPage";
import { JournalPage } from "./JournalPage";
import { LinksPage } from "./LinksPage";
import { MusicPage } from "./MusicPage";
import { ProjectsPage } from "./ProjectsPage";
import { RailPage } from "./RailPage";
import { RemindersPage } from "./RemindersPage";
import { RoutinesPage } from "./RoutinesPage";
import { SearchPage } from "./SearchPage";
import { ShortcutsPage } from "./ShortcutsPage";
import { SlackPage } from "./SlackPage";
import { TimeTrackerPage } from "./TimeTrackerPage";
import { ToolsPage } from "./ToolsPage";
import { VaultPage } from "./VaultPage";
import { WorkflowsPage } from "./WorkflowsPage";

const SCHEME_LABELS: Record<Scheme, string> = { default: "Default", light: "Light", dark: "Dark" };

/** One tile per scheme: a miniature window in the scheme's look. */
function SchemePicker({ label, value, onChange }: { label: string; value: Scheme; onChange: (scheme: Scheme) => void }) {
  return (
    <div className="scheme-picker" role="group" aria-label={label}>
      {(Object.keys(SCHEME_LABELS) as Scheme[]).map((id) => {
        const look = schemeLook(id);
        return (
          <button key={id} type="button" className="scheme-option" aria-pressed={value === id} onClick={() => onChange(id)}>
            <span className="scheme-tile" data-content={look.content} data-sidebar={look.sidebar} aria-hidden="true">
              <span className="scheme-tile-sidebar">
                <span className="scheme-tile-entry" />
              </span>
              <span className="scheme-tile-content">
                <span className="scheme-tile-block" />
              </span>
            </span>
            {SCHEME_LABELS[id]}
          </button>
        );
      })}
    </div>
  );
}

const HOURS = Array.from({ length: 25 }, (_, hour) => hour);
const hourLabel = (hour: number) => `${String(hour).padStart(2, "0")}:00`;

function HourSelect({ label, value, hours, onChange }: { label: string; value: number; hours: number[]; onChange: (hour: number) => void }) {
  return (
    <Row label={label}>
      <select className="input" aria-label={label} value={value} onChange={(e) => onChange(Number(e.target.value))}>
        {hours.map((hour) => (
          <option key={hour} value={hour}>
            {hourLabel(hour)}
          </option>
        ))}
      </select>
    </Row>
  );
}

function AppearancePage({ s, set }: SettingsPageProps) {
  const timeline = s.timeline;
  return (
    <>
      <Group title="Look">
        <Row label="Light mode scheme" description="Used while macOS is in light mode." stacked>
          <SchemePicker label="Light mode scheme" value={s.lightScheme} onChange={(lightScheme) => set({ lightScheme })} />
        </Row>
        <Row label="Dark mode scheme" description="Used while macOS is in dark mode." stacked>
          <SchemePicker label="Dark mode scheme" value={s.darkScheme} onChange={(darkScheme) => set({ darkScheme })} />
        </Row>
        <Row label="Accent" description="Buttons, links, selection and focus. System follows macOS.">
          <AccentSwatches withSystem value={s.accent} onChange={(accent) => set({ accent })} />
        </Row>
      </Group>
      <Group title="Text">
        <Segmented
          label="Text size"
          description="All text in the window. ⌘= and ⌘- step through the sizes."
          options={TEXT_SIZES.map((t) => t.size)}
          value={s.textSize}
          onChange={(textSize) => set({ textSize })}
        />
      </Group>
      <Group title="Day timeline" description="The ruler above the day's events in the Daybook window. Each context sets its own work hours in Settings → Contexts.">
        <HourSelect label="Starts at" value={timeline.firstHour} hours={HOURS.filter((hour) => hour < timeline.lastHour)} onChange={(firstHour) => set({ timeline: { ...timeline, firstHour } })} />
        <HourSelect label="Ends at" value={timeline.lastHour} hours={HOURS.filter((hour) => hour > timeline.firstHour)} onChange={(lastHour) => set({ timeline: { ...timeline, lastHour } })} />
      </Group>
    </>
  );
}

function GeneralPage() {
  return (
    <>
      <Group>
        <LaunchAtLoginSwitch />
      </Group>
      <Group title="Permissions">
        <AccessibilityRow />
      </Group>
    </>
  );
}

/** Ids and order match Rust's `window::settings::PAGES`, which `settings:<page>` and the palette read. */
export function registerCoreSettingsPages() {
  registerSettingsPage({ id: "vault", group: "Setup", label: "Vault", icon: FolderOpen, order: 0, intro: "Your Obsidian vault: Daybook reads it and writes into today's note.", Body: VaultPage });
  registerSettingsPage({ id: "calendar", group: "Setup", label: "Calendar", icon: CalendarDays, order: 5, intro: "Daybook reads your calendars to show the day's events and remind you of meetings. It never changes them.", Body: CalendarPage });
  registerSettingsPage({ id: "slack", group: "Setup", label: "Slack", icon: MessageSquare, order: 7, intro: "Daybook sets your Slack status and presence and posts your status messages. It talks to Slack and nothing else.", Body: SlackPage });
  registerSettingsPage({
    id: "jira",
    group: "Setup",
    label: "Jira",
    icon: Ticket,
    order: 8,
    intro: "Open Jira Issue (from the menu or its shortcut) opens an issue by number, key or link. In contexts with the Jira feature, it is also remembered for the suggestions.",
    Body: JiraPage,
  });
  registerSettingsPage({
    id: "timetracker",
    group: "Setup",
    label: "TimeTracker",
    icon: Timer,
    order: 9,
    intro: "Daybook shows what TimeTracker is timing. TimeTracker keeps all the time; Daybook never records any.",
    Body: TimeTrackerPage,
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
  registerSettingsPage({ id: "workflows", group: "Workflows", label: "Workflows", icon: Workflow, order: 0, intro: "What Daybook does when something happens: each workflow is a trigger and what follows from it.", Body: WorkflowsPage });
  registerSettingsPage({ id: "reminders", group: "Workflows", label: "Reminders", icon: BellRing, order: 10, intro: "The top of the Daybook window shows one reminder: the first rule in this list that applies now. Every routine and journal group has a rule here.", Body: RemindersPage });
  registerSettingsPage({ id: "calls", group: "Workflows", label: "Calls", icon: Video, order: 20, intro: "How Daybook recognizes that you're in a call, and what Join does.", Body: CallsPage });
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
  registerSettingsPage({ id: "general", group: "System", label: "General", icon: Power, order: 0, intro: "How Daybook starts.", Body: GeneralPage });
  registerSettingsPage({ id: "tools", group: "System", label: "Tools", icon: LayoutList, order: 10, Body: ToolsPage });
  registerSettingsPage({ id: "rail", group: "System", label: "Rail", icon: PanelRight, order: 15, Body: RailPage });
  registerSettingsPage({ id: "automation", group: "System", label: "Automation", icon: Bot, order: 17, intro: "Run Daybook's actions and commands from scripts: the daybook command, daybook:// links and a local HTTP API.", Body: AutomationPage });
  registerSettingsPage({ id: "shortcuts", group: "System", label: "Shortcuts", icon: Keyboard, order: 20, Body: ShortcutsPage });
  registerSettingsPage({ id: "appearance", group: "System", label: "Appearance", icon: Palette, order: 30, intro: "How Daybook looks. Changes apply right away.", Body: AppearancePage });
}
