/** The core settings pages. Modules register their own pages from their install function. */
import { Keyboard, Palette, Power } from "lucide-react";
import { TEXT_SIZES, type Scheme } from "../../api";
import { schemeLook } from "../../lib/theme";
import { registerSettingsPage, type SettingsPageProps } from "../../registry/settingsPages";
import { AccentSwatches } from "../Accent";
import { Group, Row, Segmented } from "./controls";
import { LaunchAtLoginSwitch } from "./LaunchAtLoginSwitch";
import { ShortcutsPage } from "./ShortcutsPage";

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

export function registerCoreSettingsPages() {
  registerSettingsPage({ id: "general", group: "App", label: "General", icon: Power, order: 0, intro: "How Daybook starts.", Body: GeneralPage });
  registerSettingsPage({ id: "appearance", group: "App", label: "Appearance", icon: Palette, order: 10, intro: "How Daybook looks. Changes apply right away.", Body: AppearancePage });
  registerSettingsPage({ id: "shortcuts", group: "App", label: "Shortcuts", icon: Keyboard, order: 20, intro: "Click a shortcut and press the new keys. Backspace unbinds, Escape cancels.", Body: ShortcutsPage });
}
