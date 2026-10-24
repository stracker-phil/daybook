import { vi } from "vitest";
import type { api as realApi } from "../api";
// Must not import the store or "../api": they would load the real api module while it is being mocked.
import { EMPTY_UI_STATE, type Keybindings, type Settings, type UiState } from "../api/types";
import { settings } from "./settings";

const noUnlisten = () => Promise.resolve(() => {});

/** An `api` where every call resolves to a valid default; override per test with mockResolvedValue. */
export function createApiMock(): typeof realApi {
  return {
    getSettings: vi.fn().mockResolvedValue(settings()),
    updateSettings: vi.fn(async (s: Settings) => s),
    getKeybindings: vi.fn().mockResolvedValue({}),
    setKeybindings: vi.fn(async (bindings: Keybindings) => bindings),
    getUiState: vi.fn().mockResolvedValue(EMPTY_UI_STATE),
    setUiState: vi.fn(async (ui: UiState) => ui),
    autostartSet: vi.fn(async (enabled: boolean) => enabled),
    autostartStatus: vi.fn().mockResolvedValue(false),
    openLogFolder: vi.fn().mockResolvedValue(undefined),
    openReadme: vi.fn().mockResolvedValue(undefined),
    menuSync: vi.fn().mockResolvedValue(undefined),
    openUrl: vi.fn().mockResolvedValue(undefined),
    logError: vi.fn().mockResolvedValue(undefined),
    takeLaunchEvents: vi.fn().mockResolvedValue([]),
    remoteReply: vi.fn().mockResolvedValue(undefined),
    hideWindow: vi.fn().mockResolvedValue(undefined),
    hideToMenuBar: vi.fn().mockResolvedValue(undefined),
    quitApp: vi.fn().mockResolvedValue(undefined),
    chooseFolder: vi.fn().mockResolvedValue(null),
    revealPath: vi.fn().mockResolvedValue(undefined),
    dailyNoteState: vi.fn().mockResolvedValue({ path: null, exists: false, creating: false, waiting: 0 }),
    dailyNoteOpen: vi.fn().mockResolvedValue(undefined),
    dailyNoteCreate: vi.fn().mockResolvedValue(false),
    onLaunch: vi.fn(noUnlisten),
    onWindowOpened: vi.fn(noUnlisten),
    onCommand: vi.fn(noUnlisten),
    onRemote: vi.fn(noUnlisten),
    onSettingsChanged: vi.fn(noUnlisten),
    onNoteChanged: vi.fn(noUnlisten),
  };
}
