import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  ActivityMode,
  ActivityPick,
  ActivitySuggestion,
  AppError,
  AppRef,
  AutomationInfo,
  ArtistSearch,
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
  ProjectGroup,
  LauncherProjects,
  QuickAccess,
  LaunchEvent,
  Missing,
  MusicState,
  NoteChanged,
  PaletteResults,
  NoteSectionRef,
  NotePreview,
  PanelSession,
  QuickNote,
  QuickNotePresentation,
  QuickNoteSaved,
  QuickNoteTopics,
  RailPoint,
  RailRect,
  RailShown,
  RailTipContent,
  RailModel,
  RemoteEvent,
  RoutineView,
  SearchResponse,
  SearchScopes,
  Settings,
  ShortcutOverview,
  StageActReply,
  StageActRequest,
  StageAnchorsReply,
  StageCollectRequest,
  SlackIdentity,
  SlackInfo,
  Status,
  ToolId,
  StatusChanged,
  TimeTrackerDay,
  TimeTrackerInfo,
  TimeTrackerProjects,
  TimerChanged,
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
  /** A routine with today's items, ticked or not. */
  routineViewToday: (id: string) => invoke<RoutineView>("routine_view_today", { id }),
  /** Ticks (`done`) or unticks item `index` of a routine in today's note. */
  routineStep: (id: string, index: number, done: boolean) => invoke<void>("routine_step", { id, index, done }),
  /** Opens an item's link: a URL, or a vault path in Obsidian. */
  routineOpenLink: (link: string) => invoke<void>("routine_open_link", { link }),
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
  /** Settings → Automation: the `daybook` command's link, the HTTP API's state and its token (from the Keychain). */
  automationInfo: () => invoke<AutomationInfo>("automation_info"),
  /** Shows the Stage discovery file in Finder. */
  stageRevealFile: () => invoke<void>("stage_reveal_file"),
  /** The page's anchors and meta for a `stage:collect` request. */
  stageAnchors: (reply: StageAnchorsReply) => invoke<void>("stage_anchors", { reply }),
  /** The page's answer to a `stage:act` request. */
  stageDone: (reply: StageActReply) => invoke<void>("stage_done", { reply }),
  /** Stores a new API token in the Keychain; scripts with the old one stop working. */
  automationNewToken: () => invoke<string>("automation_new_token"),
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
  dailyNoteOpen: (day?: string, heading?: string) => invoke<void>("daily_note_open", { day: day ?? null, heading: heading ?? null }),
  /** Opens a todo's note (relative to the vault) in Obsidian. */
  todoOpen: (file: string) => invoke<void>("todo_open", { file }),
  /** Creates today's note through the Obsidian CLI; resolves whether it exists now. */
  dailyNoteCreate: () => invoke<boolean>("daily_note_create"),
  /** The quick-notes picker's rows for `query` (today's matching topics, then a new one) and the notes heading. */
  quickNotesList: (query: string) => invoke<QuickNoteTopics>("quick_notes_list", { query }),
  /** Closes the picker and opens the editor on `topic`; `create` adds it to today's note first. */
  quickNotesOpen: (topic: NoteSectionRef, create: boolean) => invoke<void>("quick_notes_open", { topic, create }),
  /** The editor's topic and its text, `null` before one was chosen. */
  quickNoteGet: () => invoke<QuickNote | null>("quick_note_get"),
  /** Writes the topic's text and title unless the note no longer holds `base`, the text the editor started from. */
  quickNoteSave: (topic: NoteSectionRef, newTitle: string, body: string, base: string) =>
    invoke<QuickNoteSaved>("quick_note_save", { topic, newTitle, body, base }),
  quickNotesClose: () => invoke<void>("quick_notes_close"),
  /** Whether the editor is presenting now, for a page that (re)loads. */
  quickNotesPresentation: () => invoke<QuickNotePresentation>("quick_notes_presentation"),
  /** Search results for `text` (words, "phrases", `@scope`) within the chosen scopes; empty without words. */
  searchQuery: (text: string, scopes: string[]) => invoke<SearchResponse>("search_query", { text, scopes }),
  /** The scope names search knows (settings order, then `all`) and what is wrong with the settings' scopes. */
  searchScopes: () => invoke<SearchScopes>("search_scopes"),
  /** A note (relative to the vault) as Markdown for the preview, its links rewritten to `#note=`, `#missing=`, `#asset=`. */
  searchNote: (path: string) => invoke<NotePreview>("search_note", { path }),
  /** A vault image as a data URL. */
  searchAsset: (path: string) => invoke<string>("search_asset", { path }),
  /** Closes the overlay and opens the note in Obsidian. */
  searchOpen: (path: string) => invoke<void>("search_open", { path }),
  /** The note's wikilink, e.g. `[[Plan]]`; the path when the name is not unique. */
  searchLink: (path: string) => invoke<string>("search_link", { path }),
  /** Opens a preview link: http(s), mailto and obsidian only. */
  searchOpenUrl: (url: string) => invoke<void>("search_open_url", { url }),
  /** The links that show now and every other link, in Settings order. */
  linksView: () => invoke<QuickAccess>("links_view"),
  /** The current projects with their linked Project Launcher projects. */
  projectsView: () => invoke<ProjectGroup[]>("projects_view"),
  /** Closes the overlay and opens the issue for a number, key or link; resolves its key, rejects when it can't. */
  jiraOpen: (input: string) => invoke<string>("jira_open", { input }),
  /** Issue keys opened in the last 7 days, newest first. */
  jiraSuggestions: () => invoke<string[]>("jira_suggestions"),
  /** Project Launcher's projects (cached for a minute unless `fresh`); asking may start it. */
  launcherProjects: (fresh: boolean) => invoke<LauncherProjects>("launcher_projects", { fresh }),
  /** Settings → TimeTracker: the command's path, whether TimeTracker can be reached, and its version. */
  timetrackerInfo: () => invoke<TimeTrackerInfo>("timetracker_info"),
  /** TimeTracker's projects, archived ones too, and its state; the list read last unless `fresh`. */
  timetrackerProjects: (fresh: boolean) => invoke<TimeTrackerProjects>("timetracker_projects", { fresh }),
  /** TimeTracker's state and its running timer; later changes come as `timer_changed`. */
  timetrackerTimer: () => invoke<TimerChanged>("timetracker_timer"),
  /** The Activity tab: TimeTracker's `day` (`YYYY-MM-DD`), read-only; later changes come as `activity_changed`. */
  timetrackerDay: (day: string) => invoke<TimeTrackerDay>("timetracker_day", { day }),
  /** Brings TimeTracker to the front, starting it if needed. */
  timetrackerFocus: () => invoke<void>("timetracker_focus"),
  /** The activity overlay's suggestions: meetings, today's tickets, recent timers, in that order. */
  activitySuggestions: () => invoke<ActivitySuggestion[]>("activity_suggestions"),
  /** ↵: starts TimeTracker's timer on the pick (Working first if needed), or keeps it for an unchanged check-in; closes the overlay. */
  activitySubmit: (mode: ActivityMode, pick: ActivityPick) => invoke<void>("activity_submit", { mode, pick }),
  /** Reads Music; the entries and what Music plays now. Each music action answers with the new state. */
  musicState: () => invoke<MusicState>("music_state"),
  /** Plays an entry of Settings → Music. */
  musicPlay: (id: string) => invoke<MusicState>("music_play", { id }),
  /** Pauses, or plays (launching Music and the entry played last when it is closed). */
  musicToggle: () => invoke<MusicState>("music_toggle"),
  /** Music's own volume, clamped to 0 to 100. */
  musicVolume: (volume: number) => invoke<MusicState>("music_volume", { volume }),
  /** The playlists in Music's library, sorted; asking starts Music. */
  musicPlaylists: () => invoke<string[]>("music_playlists"),
  /** Apple's artists named like `term`, best first, and the store searched (the entries' store, else the Mac's region). */
  musicFindArtists: (term: string) => invoke<ArtistSearch>("music_find_artists", { term, storefront: null }),
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
  /** The rail's buttons in order, today's status and the rail settings; later changes come as `rail_model`. */
  railModel: () => invoke<RailModel>("rail_model"),
  /** Whether the rail is (being) shown, on which edge, and whether it stays. */
  railShownState: () => invoke<RailShown>("rail_shown_state"),
  /** The bar's measured height; the rail window is as tall. */
  railSetHeight: (height: number) => invoke<void>("rail_set_height", { height }),
  /** Button `tool` is hovered; `center` is its middle, from the rail window's top. */
  railShowTip: (tool: ToolId, center: number) => invoke<void>("rail_show_tip", { tool, center }),
  /** No button is hovered any more. */
  railHideTip: () => invoke<void>("rail_hide_tip"),
  /** Pops up the menu-bar menu at the pointer. */
  railContextMenu: () => invoke<void>("rail_context_menu"),
  /** The tooltip page drew card `sequence` at `card`, in its window's coordinates. */
  railTipRendered: (sequence: number, card: RailRect) => invoke<void>("rail_tip_rendered", { sequence, card }),
  /** Runs a choice of the tooltip card; Rust runs only the choices on show. */
  railTipRun: (command: string) => invoke<void>("rail_tip_run", { command }),

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
  /** What Music plays, or its volume, changed. */
  onMusicChanged: (handler: (music: MusicState) => void): Promise<UnlistenFn> => listen<MusicState>("music_changed", (e) => handler(e.payload)),
  /** What Slack shows, or the Slack connection, changed. */
  onSlackChanged: (handler: () => void): Promise<UnlistenFn> => listen("slack_changed", handler),
  /** TimeTracker started or stopped a timer, or started, quit or went missing. */
  onTimerChanged: (handler: (change: TimerChanged) => void): Promise<UnlistenFn> => listen<TimerChanged>("timer_changed", (e) => handler(e.payload)),
  /** TimeTracker's day the Activity tab shows changed. */
  onActivityChanged: (handler: () => void): Promise<UnlistenFn> => listen("activity_changed", handler),
  /** To the Daybook window: show the inspector on its Activity tab. */
  onShowActivityTab: (handler: () => void): Promise<UnlistenFn> => listen("show_activity_tab", handler),
  /** Settings were saved, from any window. */
  onSettingsChanged: (handler: (settings: Settings) => void): Promise<UnlistenFn> => listen<Settings>("settings_changed", (e) => handler(e.payload)),
  /** Shortcut overrides were saved, from any window. */
  onKeybindingsChanged: (handler: (bindings: Keybindings) => void): Promise<UnlistenFn> => listen<Keybindings>("keybindings_changed", (e) => handler(e.payload)),
  /** A daily note, or the changes waiting for today's note, changed. */
  onNoteChanged: (handler: (change: NoteChanged) => void): Promise<UnlistenFn> => listen<NoteChanged>("note_changed", (e) => handler(e.payload)),
  /** To the overlay page: the session to render, `null` once the overlay closed. */
  onPanelSession: (handler: (session: PanelSession | null) => void): Promise<UnlistenFn> =>
    listen<PanelSession | null>("panel_session", (e) => handler(e.payload)),
  /** To the quick-notes editor: another topic was chosen. */
  onQuickNoteTarget: (handler: () => void): Promise<UnlistenFn> => listen("quick_note_target", handler),
  /** To the quick-notes editor: presentation started or ended. */
  onQuickNotePresentation: (handler: (presentation: QuickNotePresentation) => void): Promise<UnlistenFn> =>
    listen<QuickNotePresentation>("quick_note_presentation", (e) => handler(e.payload)),
  /** To the rail page: its buttons, the status or the rail settings changed. */
  onRailModel: (handler: (model: RailModel) => void): Promise<UnlistenFn> => listen<RailModel>("rail_model", (e) => handler(e.payload)),
  /** To the rail page: it slid in or out, changed sides or became sticky. */
  onRailShown: (handler: (shown: RailShown) => void): Promise<UnlistenFn> => listen<RailShown>("rail_shown", (e) => handler(e.payload)),
  /** To the rail page: the pointer in window coordinates while it is on the rail, `null` once it left. */
  onRailPointer: (handler: (point: RailPoint | null) => void): Promise<UnlistenFn> => listen<RailPoint | null>("rail_pointer", (e) => handler(e.payload)),
  /** To the tooltip page: the card to draw. */
  onRailTipContent: (handler: (content: RailTipContent) => void): Promise<UnlistenFn> =>
    listen<RailTipContent>("rail_tip_content", (e) => handler(e.payload)),
  /** To the tooltip page: card `sequence` is on screen, so it may fade in. */
  onRailTipPlaced: (handler: (sequence: number) => void): Promise<UnlistenFn> => listen<number>("rail_tip_placed", (e) => handler(e.payload)),
  /** To the tooltip page: the card fades out. */
  onRailTipHidden: (handler: () => void): Promise<UnlistenFn> => listen("rail_tip_hidden", handler),
  /** To the tooltip page: the pointer in window coordinates while it is on the card's choices, `null` off them. */
  onRailTipPointer: (handler: (point: RailPoint | null) => void): Promise<UnlistenFn> =>
    listen<RailPoint | null>("rail_tip_pointer", (e) => handler(e.payload)),
  /** Stage asks every page for its anchors; answer with `stageAnchors`. */
  onStageCollect: (handler: (request: string) => void): Promise<UnlistenFn> =>
    listen<StageCollectRequest>("stage:collect", (e) => handler(e.payload.request)),
  /** Stage asks every page to act on an anchor or keys; answer with `stageDone`. */
  onStageAct: (handler: (request: StageActRequest) => void): Promise<UnlistenFn> => listen<StageActRequest>("stage:act", (e) => handler(e.payload)),
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
  timetracker_not_running: true,
  timetracker_refused: true,
  timetracker_missing: true,
};

export function toAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "kind" in e && "message" in e && Object.hasOwn(ERROR_KINDS, String(e.kind))) {
    return e as AppError;
  }
  const message = e && typeof e === "object" && "message" in e ? String(e.message) : String(e);
  return { kind: "io", message };
}
