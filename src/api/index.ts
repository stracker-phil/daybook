import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  AppError,
  AppRef,
  CalendarAccess,
  CalendarInfo,
  CallsInfo,
  ContextChanged,
  ContextState,
  DailyNoteState,
  EndOfDayAnswer,
  ErrorKind,
  FocusInfo,
  HubView,
  JournalGroupAnswers,
  JournalInfo,
  Keybindings,
  LaunchEvent,
  Missing,
  NoteChanged,
  PaletteResults,
  PanelSession,
  RemoteEvent,
  Settings,
  ShortcutOverview,
  SlackIdentity,
  SlackInfo,
  Status,
  StatusChanged,
  UiState,
  VaultInfo,
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
  /** Runs a command of the Rust command bus by id (`app:settings`, `hub:open`, …); rejects when it is unknown, disabled or fails. */
  runCommand: (id: string) => invoke<void>("run_command", { id }),
  /** The contexts as the switcher shows them, and the current one. */
  contextState: () => invoke<ContextState>("context_state"),
  /** Sets today's status and closes the status overlay; `endOfDay` is the end-of-day check's answer and `endOfDayList` the end-of-day list when going Off. */
  statusSet: (status: Status, message: string, endOfDay?: EndOfDayAnswer, endOfDayList?: string) =>
    invoke<void>("status_set", { status, message, endOfDay: endOfDay ?? null, endOfDayList: endOfDayList ?? null }),
  /** Today's focus list, the current projects and where the list goes in Slack. */
  focusInfo: () => invoke<FocusInfo>("focus_info"),
  /** Writes the list to today's note as typed and closes the focus overlay. */
  focusSave: (body: string) => invoke<void>("focus_save", { body }),
  /** Puts the list as typed into the day's message in Slack, without writing the note. */
  focusSend: (body: string) => invoke<void>("focus_send", { body }),
  /** Today's journal groups with their answers. */
  journalInfo: () => invoke<JournalInfo>("journal_info"),
  /** Rewrites one group's callout in today's note (it waits for the note when it doesn't exist yet). */
  journalSave: (group: string, answers: JournalGroupAnswers) => invoke<void>("journal_save", { group, answers }),
  /** The palette's results for what was typed, grouped by kind. */
  paletteQuery: (query: string) => invoke<PaletteResults>("palette_query", { query }),
  /** Runs a palette pick; the palette closes or another overlay replaces it. */
  paletteRun: (command: string) => invoke<void>("palette_run", { command }),
  /** Settings → Shortcuts: the global shortcuts of the commands that exist and the app menu's keys. */
  shortcutOverview: () => invoke<ShortcutOverview>("shortcut_overview"),
  /** Everything the Daybook window shows for `day` (`YYYY-MM-DD`, default today). */
  hubView: (day?: string) => invoke<HubView>("hub_view", { day: day ?? null }),
  /** Calendar access and this Mac's event calendars, for Settings. */
  calendarInfo: () => invoke<CalendarInfo>("calendar_info"),
  /** Asks macOS the first time; after a refusal it opens the privacy settings. Resolves the access now. */
  calendarRequestAccess: () => invoke<CalendarAccess>("calendar_request_access"),
  /** Joins an event's call: starts the start-first apps, then opens the link in its app. Join links only. */
  calendarJoin: (url: string) => invoke<void>("calendar_join", { url }),
  /** Settings → Calls: detection, the running call, permissions, the built-in app triggers. */
  callsInfo: () => invoke<CallsInfo>("calls_info"),
  /** Accessibility: macOS's prompt the first time, then its privacy settings; Automation: its privacy settings. */
  callsRequestAccess: (permission: Missing) => invoke<boolean>("calls_request_access", { permission }),
  /** An app chosen in the file panel, with its bundle id; null when cancelled. */
  callsPickApp: () => invoke<AppRef | null>("calls_pick_app"),
  /** Whether a Slack token is stored, and whose it is once checked. The token never comes back. */
  slackInfo: () => invoke<SlackInfo>("slack_info"),
  /** Checks the token with Slack, then stores it in the Keychain. */
  slackConnect: (token: string) => invoke<SlackIdentity>("slack_connect", { token }),
  /** Checks the stored token with Slack. */
  slackTest: () => invoke<SlackIdentity>("slack_test"),
  /** Removes the token from the Keychain. */
  slackDisconnect: () => invoke<void>("slack_disconnect"),
  /** Where the daily notes are and whether today's note and the Obsidian CLI were found. */
  vaultInfo: () => invoke<VaultInfo>("vault_info"),
  /** While Settings records a shortcut, global shortcuts and the menu's key equivalents step aside. */
  shortcutsPause: (paused: boolean) => invoke<void>("shortcuts_pause", { paused }),
  /** http(s) links only; Rust refuses anything else. */
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  logError: (message: string) => invoke<void>("log_frontend_error", { message }),
  takeLaunchEvents: () => invoke<LaunchEvent[]>("take_launch_events"),
  /** Answers a `remote` event; a non-null `error` fails the request. */
  remoteReply: (id: string, result: unknown, error: string | null) => invoke<void>("remote_reply", { id, result, error }),
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
  /** The open overlay stays while focus is elsewhere, e.g. while a link it opened is checked. */
  panelKeepOpen: (keep: boolean) => invoke<void>("panel_keep_open", { keep }),

  onLaunch: (handler: () => void): Promise<UnlistenFn> => listen("launch", handler),
  /** The hidden main window was shown again (the Dock icon came back). */
  onWindowOpened: (handler: () => void): Promise<UnlistenFn> => listen("window_opened", handler),
  /** To the Settings window: show this page. */
  onOpenSettings: (handler: (page: string) => void): Promise<UnlistenFn> => listen<string>("open_settings", (e) => handler(e.payload)),
  /** To the Daybook window: show or hide the inspector column. */
  onToggleInspector: (handler: () => void): Promise<UnlistenFn> => listen("toggle_inspector", handler),
  /** Another context became current. */
  onContextChanged: (handler: (change: ContextChanged) => void): Promise<UnlistenFn> => listen<ContextChanged>("context_changed", (e) => handler(e.payload)),
  /** Today's status changed. */
  onStatusChanged: (handler: (change: StatusChanged) => void): Promise<UnlistenFn> => listen<StatusChanged>("status_changed", (e) => handler(e.payload)),
  /** An event, a calendar or the calendar access changed. */
  onCalendarChanged: (handler: () => void): Promise<UnlistenFn> => listen("calendar_changed", handler),
  /** A call started, changed or ended. */
  onCallChanged: (handler: () => void): Promise<UnlistenFn> => listen("call_changed", handler),
  /** What Slack shows, or the Slack connection, changed. */
  onSlackChanged: (handler: () => void): Promise<UnlistenFn> => listen("slack_changed", handler),
  /** Settings were saved, from any window. */
  onSettingsChanged: (handler: (settings: Settings) => void): Promise<UnlistenFn> => listen<Settings>("settings_changed", (e) => handler(e.payload)),
  /** Shortcut overrides were saved, from any window. */
  onKeybindingsChanged: (handler: (bindings: Keybindings) => void): Promise<UnlistenFn> => listen<Keybindings>("keybindings_changed", (e) => handler(e.payload)),
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
  network: true,
  cancelled: true,
};

export function toAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "kind" in e && "message" in e && Object.hasOwn(ERROR_KINDS, String(e.kind))) {
    return e as AppError;
  }
  const message = e && typeof e === "object" && "message" in e ? String(e.message) : String(e);
  return { kind: "io", message };
}
