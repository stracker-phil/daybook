import { setOverrides } from "../../commands/registry";
import { api, type Keybindings, type Settings } from "../../api";
import { fail } from "../requests";
import type { Slice } from "../store";

export interface SettingsSlice {
  settings: Settings | null;
  keybindings: Keybindings;
  loadSettings: () => Promise<void>;
  /** Takes settings Rust saved elsewhere (`settings_changed`). */
  applySettings: (settings: Settings) => void;
  /** `quiet` skips the toast, for settings that apply as they change. */
  saveSettings: (settings: Settings, opts?: { quiet?: boolean }) => Promise<void>;
  saveKeybindings: (bindings: Keybindings) => Promise<void>;
  /** Turns launch at login on or off; resolves whether it is on now, or `null` when it failed (toasted). */
  setLaunchAtLogin: (enabled: boolean) => Promise<boolean | null>;
}

export const settingsSlice: Slice<SettingsSlice> = {
  create: (set, get) => ({
    settings: null,
    keybindings: {},

    async loadSettings() {
      try {
        const [settings, keybindings] = await Promise.all([api.getSettings(), api.getKeybindings().catch(() => ({}))]);
        set({ settings, keybindings });
      } catch (e) {
        fail(e, "Could not load settings");
      }
    },

    applySettings(settings) {
      set({ settings });
    },

    async saveSettings(settings, opts) {
      try {
        set({ settings: await api.updateSettings(settings) });
        if (!opts?.quiet) get().toast("success", "Settings saved");
      } catch (e) {
        fail(e, "Could not save settings");
      }
    },

    async setLaunchAtLogin(enabled) {
      try {
        const launchAtLogin = await api.autostartSet(enabled);
        // Rust stored the choice; the copy here must match, or the next settings save would undo it.
        const settings = get().settings;
        if (settings) set({ settings: { ...settings, launchAtLogin } });
        return launchAtLogin;
      } catch (e) {
        fail(e, "Could not change launch at login");
        return null;
      }
    },

    async saveKeybindings(bindings) {
      try {
        const saved = await api.setKeybindings(bindings);
        set({ keybindings: saved });
        setOverrides(saved);
      } catch (e) {
        fail(e, "Could not save shortcuts");
      }
    },
  }),
};
