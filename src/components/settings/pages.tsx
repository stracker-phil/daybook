/** The core settings pages. Modules register their own pages from their install function. */
import { FolderKanban, FolderOpen, Keyboard, Layers, LayoutList, NotebookPen, Palette, Power } from "lucide-react";
import { TEXT_SIZES, type Scheme } from "../../api";
import { schemeLook } from "../../lib/theme";
import { registerSettingsPage, type SettingsPageProps } from "../../registry/settingsPages";
import { AccentSwatches } from "../Accent";
import { Group, Row, Segmented } from "./controls";
import { LaunchAtLoginSwitch } from "./LaunchAtLoginSwitch";
import { ContextsPage } from "./ContextsPage";
import { DailyNotePage } from "./DailyNotePage";
import { ProjectsPage } from "./ProjectsPage";
import { ShortcutsPage } from "./ShortcutsPage";
import { ToolsPage } from "./ToolsPage";
import { VaultPage } from "./VaultPage";

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

function AppearancePage({ s, set }: SettingsPageProps) {
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
    </>
  );
}

function GeneralPage() {
  return (
    <Group>
      <LaunchAtLoginSwitch />
    </Group>
  );
}

/** Ids and order match Rust's `window::settings::PAGES`, which `settings:<page>` and the palette read. */
export function registerCoreSettingsPages() {
  registerSettingsPage({ id: "vault", group: "Setup", label: "Vault", icon: FolderOpen, order: 0, intro: "Your Obsidian vault: Daybook reads it and writes into today's note.", Body: VaultPage });
  registerSettingsPage({ id: "contexts", group: "Setup", label: "Contexts", icon: Layers, order: 10, intro: "Each context turns features on and off, has its colour and work hours, and can become current by itself when they start.", Body: ContextsPage });
  registerSettingsPage({ id: "projects", group: "Setup", label: "Projects", icon: FolderKanban, order: 20, intro: "What you work on: offered for meetings and the Projects tool in the contexts you pick.", Body: ProjectsPage });
  registerSettingsPage({ id: "daily-note", group: "Obsidian", label: "Daily Note", icon: NotebookPen, order: 0, intro: "Where Daybook writes into today's note. Sections are created in this order when Daybook first writes to them; your template only needs the title and the notes heading.", Body: DailyNotePage });
  registerSettingsPage({ id: "general", group: "System", label: "General", icon: Power, order: 0, intro: "How Daybook starts.", Body: GeneralPage });
  registerSettingsPage({ id: "tools", group: "System", label: "Tools", icon: LayoutList, order: 10, Body: ToolsPage });
  registerSettingsPage({ id: "shortcuts", group: "System", label: "Shortcuts", icon: Keyboard, order: 20, Body: ShortcutsPage });
  registerSettingsPage({ id: "appearance", group: "System", label: "Appearance", icon: Palette, order: 30, intro: "How Daybook looks. Changes apply right away.", Body: AppearancePage });
}
