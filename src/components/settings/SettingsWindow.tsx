import type { Settings } from "../../api";
import { resolveSettingsPage, settingsGroups, useSettingsPagesVersion } from "../../registry/settingsPages";
import { useStore } from "../../state/store";

/** System Settings layout: the page list grouped on the left, the page on the right; every change applies at once. */
export function SettingsWindow({ page: shown, onPage }: { page: string | undefined; onPage: (id: string) => void }) {
  useSettingsPagesVersion();
  const settings = useStore((s) => s.settings);
  const save = useStore((s) => s.saveSettings);
  const page = resolveSettingsPage(shown);
  if (!settings || !page) return null;

  const set = (patch: Partial<Settings>) => {
    const current = useStore.getState().settings;
    if (current) void save({ ...current, ...patch }, { quiet: true });
  };
  const { Body } = page;

  return (
    <div className="settings-window">
      <nav className="settings-nav" aria-label="Settings pages">
        <div className="settings-titlebar" data-tauri-drag-region />
        {settingsGroups().map((g) => (
          <div key={g.group} className="settings-nav-group">
            <div className="settings-nav-heading">{g.group}</div>
            {g.pages.map(({ id, label, icon: Icon }) => (
              <button key={id} type="button" aria-current={page.id === id ? "page" : undefined} onClick={() => onPage(id)}>
                <Icon size="1.077rem" /> {label}
              </button>
            ))}
          </div>
        ))}
      </nav>
      <div className="settings-pane">
        <header data-tauri-drag-region>
          <h2 data-tauri-drag-region>
            {page.label}
          </h2>
        </header>
        <div className="settings-body" key={page.id}>
          {page.intro && <p className="settings-intro">{page.intro}</p>}
          <Body s={settings} set={set} />
        </div>
      </div>
    </div>
  );
}
