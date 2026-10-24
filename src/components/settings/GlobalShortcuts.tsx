import type { MenuShortcut, Settings, ShortcutOverview } from "../../api";
import { findShortcutConflict, shortcutFromKeyEvent, shortcutKeys, SYSTEM_SHORTCUTS } from "../../lib/shortcuts";
import type { SettingsPageProps } from "../../registry/settingsPages";
import { KeyCap } from "../Field";
import { Group, Row } from "./controls";
import { ShortcutRecorder } from "./ShortcutRecorder";

const glyphs = (shortcut: string) => shortcutKeys(shortcut)?.join("") ?? shortcut;

/** The shortcut stored under `action`; `""` is none. */
const stored = (hotkeys: Settings["hotkeys"], action: string) => (hotkeys as Record<string, string>)[action] ?? "";

/** What else `action`'s shortcut would clash with: macOS, the app menu and the other global shortcuts. */
function conflictFor(action: string, overview: ShortcutOverview, hotkeys: Settings["hotkeys"]) {
  const taken = [
    ...SYSTEM_SHORTCUTS,
    ...overview.menu.map((key) => ({ shortcut: key.shortcut, label: key.title })),
    ...overview.global.filter((other) => other.action !== action && stored(hotkeys, other.action)).map((other) => ({ shortcut: stored(hotkeys, other.action), label: other.title })),
  ];
  return findShortcutConflict(stored(hotkeys, action) || undefined, taken);
}

/** System-wide shortcuts, stored in `Settings.hotkeys` in Settings notation. */
export function GlobalShortcuts({ overview, s, set }: SettingsPageProps & { overview: ShortcutOverview }) {
  const save = (action: string, shortcut: string) => set({ hotkeys: { ...s.hotkeys, [action]: shortcut } });
  return (
    <Group
      title="Global shortcuts"
      description={
        <>
          Work in every app. Click a shortcut and press the new keys; ⌫ clears it, Esc cancels.
          {!overview.registered && <> This build registers none while the prototype runs; they take effect in the debug build.</>}
        </>
      }
    >
      {overview.global.map(({ action, title, help }) => {
        const value = stored(s.hotkeys, action);
        const conflict = conflictFor(action, overview, s.hotkeys);
        return (
          <Row
            key={action}
            label={title}
            description={
              <>
                <span className="shortcut-global">Global</span> {conflict ? <span className="shortcut-clash">Also used by {conflict.label}.</span> : help}
              </>
            }
          >
            <ShortcutRecorder
              name={title}
              value={value || null}
              parse={shortcutFromKeyEvent}
              label={glyphs}
              onChange={(shortcut) => save(action, shortcut ?? "")}
            />
          </Row>
        );
      })}
    </Group>
  );
}

/** The app menu's keys are fixed. */
export function MenuShortcuts({ menu }: { menu: MenuShortcut[] }) {
  return (
    <Group title="In Daybook" description="The app menu's keys while a Daybook window is in front.">
      {menu.map((key) => (
        <Row key={`${key.command}:${key.shortcut}`} label={key.title}>
          <KeyCap keys={glyphs(key.shortcut)} />
        </Row>
      ))}
    </Group>
  );
}
