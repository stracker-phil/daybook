import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppError,
  DailyNoteState,
  ErrorKind,
  Keybindings,
  LaunchEvent,
  MenuState,
  NoteChanged,
  PanelSession,
  RemoteEvent,
  Settings,
  UiState,
} from "./types";

export * from "./types";

/** The only place that talks to Rust. Tests mock this module. */
export const api = {
  getSettings: () => invoke<Settings>("get_settings"),
  /** Returns the settings as Rust saved them (repaired by `Settings::from_json`). */
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  getKeybindings: () => invoke<Keybindings>("get_keybindings"),
  setKeybindings: (bindings: Keybindings) => invoke<Keybindings>("set_keybindings", { bindings }),
  getUiState: () => invoke<UiState>("get_ui_state"),
  setUiState: (ui: UiState) => invoke<UiState>("set_ui_state", { ui }),
  /** Installs or removes the launch-at-login LaunchAgent and stores the choice; resolves whether it is on now. */
  autostartSet: (enabled: boolean) => invoke<boolean>("autostart_set", { enabled }),
  /** Whether the LaunchAgent is installed (the truth; `settings.launchAtLogin` mirrors it). */
  autostartStatus: () => invoke<boolean>("autostart_status"),
  openLogFolder: () => invoke<void>("open_log_folder"),
  /** Opens the bundled README (in debug builds the repository's) in the default Markdown app. */
  openReadme: () => invoke<void>("open_readme"),
  /** Reports which native menu items can run now and which are checked. */
  menuSync: (state: MenuState) => invoke<void>("menu_sync", { state }),
  /** http(s) links only; Rust refuses anything else. */
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  logError: (message: string) => invoke<void>("log_frontend_error", { message }),
  takeLaunchEvents: () => invoke<LaunchEvent[]>("take_launch_events"),
  /** Answers a `remote` event; a non-null `error` fails the request. */
  remoteReply: (id: string, result: unknown, error: string | null) => invoke<void>("remote_reply", { id, result, error }),
  /** Hides the window; the app keeps running in the menu bar. */
  hideWindow: () => invoke<void>("hide_window"),
  /** Hides the window and the Dock icon; the app keeps running in the menu bar. */
  hideToMenuBar: () => invoke<void>("hide_to_menu_bar"),
  quitApp: () => invoke<void>("quit_app"),
  /** The native folder panel; `null` when the user cancels. */
  chooseFolder: () => invoke<string | null>("choose_folder"),
  /** Opens the enclosing folder in Finder with `path` selected. */
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  /** Today's note: where it is, whether it exists or is being created, and how many changes wait for it. */
  dailyNoteState: () => invoke<DailyNoteState>("daily_note_state"),
  /** Opens a day's note (`YYYY-MM-DD`, default today) in Obsidian; today's is created when missing. */
  dailyNoteOpen: (day?: string) => invoke<void>("daily_note_open", { day: day ?? null }),
  /** Creates today's note through the Obsidian CLI; resolves whether it exists now. */
  dailyNoteCreate: () => invoke<boolean>("daily_note_create"),
  /** Opens `view` in the overlay; the same view again closes it. */
  panelOpen: (view: string, args?: unknown) => invoke<void>("panel_open", { view, args: args ?? null }),
  panelClose: () => invoke<void>("panel_close"),
  /** The overlay's current session, `null` while it is closed. */
  panelSession: () => invoke<PanelSession | null>("panel_session"),
  /** The overlay page rendered `session` at this content height. */
  panelReady: (session: number, height: number) => invoke<void>("panel_ready", { session, height }),
  /** The overlay page painted `session`'s first frame. */
  panelPainted: (session: number) => invoke<void>("panel_painted", { session }),
  panelSetHeight: (height: number) => invoke<void>("panel_set_height", { height }),

  onLaunch: (handler: () => void): Promise<UnlistenFn> => listen("launch", handler),
  /** The hidden main window was shown again (the Dock icon came back). */
  onWindowOpened: (handler: () => void): Promise<UnlistenFn> => listen("window_opened", handler),
  /** Native menu and tray items emit the id of the command they run. */
  onCommand: (handler: (id: string) => void): Promise<UnlistenFn> => listen<string>("command", (e) => handler(e.payload)),
  /** Settings were saved, from any window. */
  onSettingsChanged: (handler: (settings: Settings) => void): Promise<UnlistenFn> => listen<Settings>("settings_changed", (e) => handler(e.payload)),
  /** A daily note, or the changes waiting for today's note, changed. */
  onNoteChanged: (handler: (change: NoteChanged) => void): Promise<UnlistenFn> => listen<NoteChanged>("note_changed", (e) => handler(e.payload)),
  /** To the overlay page: the session to render, `null` once the overlay closed. */
  onPanelSession: (handler: (session: PanelSession | null) => void): Promise<UnlistenFn> =>
    listen<PanelSession | null>("panel_session", (e) => handler(e.payload)),
  /** Requests from other processes; answer each with `remoteReply`. */
  onRemote: (handler: (event: RemoteEvent) => void): Promise<UnlistenFn> => listen<RemoteEvent>("remote", (e) => handler(e.payload)),
};

/** Every `ErrorKind` Rust sends; the `Record` makes the compiler check the list is complete. */
const ERROR_KINDS: Record<ErrorKind, true> = {
  io: true,
  invalid_input: true,
  not_found: true,
  cancelled: true,
};

export function toAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "kind" in e && "message" in e && Object.hasOwn(ERROR_KINDS, String(e.kind))) {
    return e as AppError;
  }
  const message = e && typeof e === "object" && "message" in e ? String(e.message) : String(e);
  return { kind: "io", message };
}
