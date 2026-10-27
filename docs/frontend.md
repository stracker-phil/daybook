# Frontend

`src/` holds a React 19 + TypeScript app, bundled by Vite. State lives in one zustand store. The frontend
renders and forwards intent; [Rust owns the logic](architecture.md#design-rules).

## Boot

1. `index.html` loads `main.tsx`.
2. `main.tsx` calls `installModules("main")` (`src/modules.ts`) before the first render:
   `registerCoreSettingsPages()` (`components/settings/pages.tsx`), then `installCommands()` (the core scopes,
   then the command registry).
3. `applyInitialLook()` (`lib/theme.ts`) sets the default look for the OS appearance (light mode Default, dark
   mode Dark) and the System accent, so the first paint has colors before settings load.
4. `installStageHooks("hub")` ([stage.md](stage.md)), then `installNativeMenuGuard()` (`lib/nativeMenu.ts`): a
   right-click that no app menu handled shows nothing, so the WebView's Reload and Inspect Element never appear;
   text fields keep Cut, Copy and Paste.
5. `main.tsx` assigns the store to `window.daybook` only when `import.meta.env.VITE_DEV_HOOKS` is `"1"` (the
   debug bundle of `bun run build:app`, driven by `scripts/dev/js.sh`), then renders
   `<ErrorBoundary><App/></ErrorBoundary>`.
6. `App.tsx` imports the stylesheets (`styles/index.ts`) and runs `useBackendEvents()` once:
   1. `loadSettings()` loads settings and keybindings; then `installCommands()` runs again to apply the
      user's bindings, and `drainLaunches()` pulls the queued launch events (`api.takeLaunchEvents`): `error`
      shows an error toast, `notice` an info toast; `activate` is Rust's and needs nothing from the page.
      `loadUiState()` loads the window layout.
   2. `loadHub()` loads the Daybook window's view model, again every minute and on `settings_changed`,
      `context_changed`, `status_changed`, `note_changed`, `music_changed` and `window_opened`.
   3. It subscribes to `launch` (drain again), `remote` (to `answerRemote`), `settings_changed`,
      `keybindings_changed` (`applyKeybindings`) and `toggle_inspector` (`toggleInspector`).
   4. It adds a capture-phase `keydown` to `handleKeydown`, `error` and `unhandledrejection` to an error
      toast ("Unexpected error: …"), and `beforeunload` to `flushUiState`.
8. `applyAppearance(settings, systemDark)` runs whenever the settings or the OS appearance change.
9. `trackWindowFocus()` (`lib/windowFocus.ts`) keeps `window-blurred` on `<html>` while the window isn't key;
   `shell.css` dims titles, toolbar buttons and the sidebar to 40% and turns accent fills grey then, as Glaze
   does.

```
App
├─ .app
│  └─ Workspace     (.workspace grid; .no-sidebar, .no-inspector from uiState)
│     ├─ Sidebar    the day (weekday, date, ‹ ›), the tool rows (the status with its dot and time), the Settings gear (not rendered while hidden)
│     ├─ main       Board: the toolbar with the greeting, the board's column, the inspector toggle (fixed, lower right)
│     └─ Inspector  the right column: the Todos toolbar (not rendered while hidden)
├─ Dialogs          shortcuts, confirm, prompt
└─ Toasts
```

The Settings window is a page of its own, `src/windows/settings/` (a Vite input like the overlay):
`main.tsx` runs `installModules("settings")` and the same look and guards, then `SettingsApp` loads the
settings, follows `settings_changed` and `keybindings_changed`, shows the page from `?page=` and then from
`open_settings`, and renders `SettingsWindow`, `Dialogs` and `Toasts`.

The `links`, `projects` and `jira` overlay views are `LinksView`, `ProjectsView` (both on `ShortcutListView`) and `JiraView` (`fuzzy.ts` filters its suggestions). The project card's Launcher projects are `LauncherProjectsField` (`useLauncherProjects`: the list from `api.launcherProjects`, renamed projects saved once per list).

The rail and its tooltip are pages of their own: `src/windows/rail/` (`RailApp`, `RailButton`, `useRailLayout`: the model, the shown state and the relayed pointer from Rust) and `src/windows/railtip/` (`RailTipApp`: one card, placed by Rust) ([windows.md](windows.md#the-rail)).

The `search` overlay view lives in `src/windows/panel/search/`: `SearchView` (field, chips, results, preview, keys), `useScopeChips` (the chip rules and `@` suggestions), `useResultActions` (open, copy link, the footer status), `ResultList`, `NotePreview` (react-markdown with remark-gfm; `#note=`, `#missing=` and `#asset=` hrefs from Rust) and `usePreviewHistory` (⌘[ and ⌘]) ([search.md](search.md#the-overlay)).

The quick-notes editor is one too, `src/windows/quicknotes/`: `QuickNotesApp` (title strip, editor, status
line; Esc saves and closes) over `useQuickNote` (loads the topic on `quick_note_target`, autosaves through
`lib/useAutosave.ts` with the loaded text as `base`, reloads on `note_changed` and focus only while nothing is
unsaved, and keeps a refused draft until "Load the note"), `MarkdownEditor` (CodeMirror 6: `lang-markdown`, the
format keymap, a fresh editor per loaded text) and `markdown.ts` (pure ⌘B, ⌘I, ⌘L edits). It has no store and
no toasts; errors go to the log.

`main` keeps its place in the tree when the sidebar or the inspector hides, so the board keeps its state.

## The Daybook window

Rust builds what the window shows (`hub_view`, [backend](backend.md#tauri-commands)); the page renders the
`HubView` of the `hub` slice. Its `tools` are the sidebar's rows in Settings → General → Tools order: `{ kind: "tool",
id, title, command, shortcut, accent, enabled }` or `{ kind: "separator" }`. Rust leaves out tools switched
off for the window, tools with nothing to offer and tools whose command no feature registered yet, and greys
a command that can't run now; `shortcut` is the global shortcut that works in this build, in Settings
notation. Each feature's step adds its part of the view model and the board section or inspector tab that
renders it.

The board (`hub/Board.tsx`) shows `TodayCard` (the reminder line; with an action it is a button that takes the
focus on mount and on `window_opened`, so ↵ runs it: a command through `runCommand`, a call through
`calendarJoin`; then `hub.focus`, the focus list as one button that runs `focus:open`, disabled on another day) and `Events` (the section title, the eye toggle `toggleHidePastEvents` saved as
`uiState.hidePastEvents`, `DayTimeline`, the rows). `Events` keeps the hovered key, so a row and its timeline block
highlight each other ([calendar.md](calendar.md#the-day-model)). `RoutinesRow` and `JournalRow` (`hub/ChipRows.tsx`)
follow: a chip per `hub.routines` routine or `hub.journal` group with its progress dots (a check once done),
running `routine:<id>` or `journal:open:<group>` today. The hub reloads on `calendar_changed` and `call_changed` too.

## Store

`state/store.ts`: one zustand store, `useStore`, assembled from slices. A `Slice<T>` has one `create(set, get)`
that returns its initial state and actions; `registerSlice(slice)` merges them into the live store and works
after creation, so a module can register its slice late. `resetStore(patch)` runs every registered slice's
`create` again and applies `patch`, for tests. The `Store` interface extends every slice's type; a module adds
its members with `declare module "../state/store" { interface Store { … } }`.

`state/requests.ts` has `fail(e, context)`: an error toast `<context>: <message>` (which also logs it), returning
the `AppError`; `runBusCommand(id, failure)` runs a bus command and toasts `<failure>: <reason>` when it fails.
The store re-exports `fail` with `flushUiState` and the `Dialog` and `Toast` types.

| Slice | Holds |
|---|---|
| `settings` | `settings` (`null` until loaded), `keybindings`, `loadSettings` (settings and keybindings in parallel; keybindings fall back to `{}`), `applySettings(settings)` (settings Rust saved elsewhere, from `settings_changed`), `saveSettings(settings, { quiet? })` (stores what `api.updateSettings` returns, toasts "Settings saved" unless `quiet`), `saveKeybindings(bindings)` (stores Rust's answer and calls `setOverrides`), `applyKeybindings(bindings)` (the same for bindings saved in the other window), `setLaunchAtLogin(enabled)` (`api.autostartSet`, then patches `settings.launchAtLogin` so a later save keeps it; resolves whether it is on now, `null` on a toasted failure) |
| `ui` | `toasts`, `toast(kind, message, action?)`, `dismissToast(id)`, `dialog` (a `Dialog` or `null`), `openDialog`, `closeDialog` |
| `viewState` | `uiState` (`UiState` from Rust: `sidebarHidden`, `inspectorHidden`, `hidePastEvents`, `collapsedTodoGroups`, `inspectorTab`, `activityView`; `EMPTY_UI_STATE` until it loads), `loadUiState`, `toggleSidebar`, `toggleInspector`, `toggleHidePastEvents`, `toggleTodoGroup(label)`, `setInspectorTab(tab)`, `setActivityView(view)`, `showActivityTab` |
| `hub` | `hub` (the `HubView`, `null` until loaded), `shownDay` (`null`: today, so the window follows midnight), `loadHub` (`api.hubView(shownDay)`; a stale answer for a day no longer shown is dropped, and an answer that is today resets `shownDay`), `showDay(day)` |

Every `viewState` change is written through `api.setUiState` 500 ms after the last one; `flushUiState` writes a pending change at once.

Components select narrowly (`useStore((s) => s.uiState.sidebarHidden)`), never the whole store.
`lib/useApiResult` loads a value from `api` for a key: `useApiResult(key, fetcher, { onError?, keepData?,
delayMs? })` fetches whenever the key changes (`null` fetches nothing), drops a reply for a superseded key or
after unmount, and returns `{ state, data, error, reload }`.

## API boundary

Only `api.ts` files import `@tauri-apps/api` or `@tauri-apps/plugin-*`: the core's is `src/api/index.ts`, a
module's is `src/<module>/api.ts` ([ADR 0002](adr/0002-ipc-in-api-files.md)); `test/boundary.test.ts` enforces
it. `src/api/index.ts` exports `api`, a flat object of thin `invoke` wrappers for the commands in
[backend.md](backend.md#tauri-commands):

- settings and layout: `getSettings`, `updateSettings` (returns the settings as Rust saved them, repaired by
  `Settings::from_json`), `getKeybindings`, `setKeybindings`, `getUiState`, `setUiState`;
- launch at login: `autostartSet`, `autostartStatus` (whether the LaunchAgent is installed, the truth that
  `settings.launchAtLogin` mirrors);
- the app: `runCommand(id)` (a Rust command, [commands.md](commands.md)), `takeLaunchEvents`,
  `remoteReply(id, result, error)`, `openLogFolder`, `openUrl` (http(s) only), `chooseFolder` (`null` on
  cancel), `logError`, `shortcutsPause(paused)`;
- the palette: `paletteQuery(query)`, `paletteRun(command)`; the vault's and the overlay's calls
  ([vault.md](vault.md#commands), [windows.md](windows.md#commands-and-events));
- the subscriptions `onLaunch`, `onWindowOpened`, `onOpenSettings`, `onRemote`, `onNoteChanged`, `onPanelSession` and `onSettingsChanged` (the saved settings, from any window; `App` hands them to `applySettings`), each resolving to its unlisten
  function.

`toAppError(e)` normalizes a rejection: a value with a known `kind` and a `message` is an `AppError`; anything
else becomes `{ kind: "io", message }`. `ERROR_KINDS` lists every `ErrorKind` as a `Record`, so the compiler
notices a kind Rust adds. An `AppError` may carry `field` (`AppErrorField { path, message }`) for a form field.

The IPC types are generated from Rust with ts-rs ([ADR 0004](adr/0004-generated-ipc-types.md)) into
`src/api/types.gen.ts` (`bun run types`); never edit it. `src/api/types.ts` re-exports them plus the
frontend-only definitions: `TEXT_SIZES` (each `TextSize` with its `ui` and `data` pixel sizes),
`DEFAULT_TEXT_SIZE`, `ACCENTS` (mirrors Rust's list; keep them equal), `SYSTEM_ACCENT` (`"system"`),
`Keybindings` (a map of command id to binding, `null` unbinds; ts-rs cannot export Rust's alias) and
`EMPTY_UI_STATE`.

## Modules

`src/modules.ts` is the install list: the core first, then one line per module. A module is a folder of its
own (`src/<module>/`) with an `api.ts`, its slice (`registerSlice`), its scopes and commands
(`registerScope`, `registerCommands`) and its settings pages (`registerSettingsPage`), installed by one
function called from `installModules(window)`. Removing a module is deleting its folder and its line; the core
never imports a module. Daybook has no module yet.

`registry/settingsPages.ts` holds the Settings pages by id. `registerSettingsPage({ id, group, label, icon,
intro?, Body, order?, sections? })` adds or replaces one; `Body` gets `SettingsPageProps` (`s`, the settings, and
`set(patch)`), and a page that keeps its own state ignores them. `sections` (`{ id, title }`) are former pages
that are now a group of this one, each a `Group anchor` on the page. `settingsPages()` returns them in nav order:
by group (`GROUP_ORDER`: Setup, Obsidian, Workflows, System; other groups follow by name), then `order`,
then label. A page's id is also in Rust's `window::settings::PAGES` and a section's in `SECTIONS`, which
`settings:<page>` and the palette read.
`settingsGroups(query?)` groups them for the nav, only those whose label, nav group or a section title contains
`query`; `resolveSettingsPage(id)` returns the named page, the page holding the named section, or the first one;
and `useSettingsPagesVersion()` re-renders a caller when a page registers, so a module may register after the
window opened.

## Commands

App-level actions are Rust commands ([commands.md](commands.md)); the page runs one with `api.runCommand(id)`.
The page's registry (`commands/registry.ts`) only holds the keys inside one window: `Command` is `{ id,
title, scope, defaultBinding?, when?, run }`; `when` false means the command does not fire.

The core's are `APP_COMMANDS` (`commands/index.ts`). Every window registers all of them, so Settings can list
them; the scope keeps a key to its window: `global` (label "Every window") is active everywhere, `hub`
("Daybook window", a child of `global`) only in the Daybook window (`installCommands("main")`), `quickNotes`
("Quick notes editor", a child of `global`) only in the editor (`installCommands("quicknotes")`).

| Id | Title | Scope | Default |
|---|---|---|---|
| `shortcuts.show` | Keyboard shortcuts (opens the overview of the active scopes; pressed again it closes it) | hub | ⇧? |
| `sidebar.toggle` | Toggle sidebar (`toggleSidebar`) | hub | ⌘⌃S |
| `inspector.toggle` | Toggle inspector (`toggleInspector`) | hub | ⌘⌃I |
| `quickNotes.present` | Present quick note (runs `notes:present`; while a note is loaded, `setQuickNoteLoaded`) | quickNotes | ⌘⌥P |
| `day.previous`, `day.next`, `day.today` | Previous / next day with a note, back to today (`showDay`; not while typing) | hub | P N O |
| `window.close` | Close window (runs `window:close`; not while a text field has the focus) | global | Escape |
| `zoom.in`, `zoom.out` | Larger text, Smaller text (step through `TEXT_SIZES`, saved quietly; past either end nothing changes) | global | ⌘= ⌘- |
| `zoom.reset` | Default text size (`DEFAULT_TEXT_SIZE`) | global | – (⌘0 is the app menu's Daybook) |

Settings ⌘,, the palette ⌘K, Close Window ⌘W and ⌘Q are app menu items running Rust commands; the native
menu takes their keys before the page sees them.

`installCommands(window)` remembers the window, registers the core scopes, registers `APP_COMMANDS` when the registry is empty, and
applies the store's keybindings with `setOverrides`; it is safe to call again. Module scopes must be
registered before their commands: `registerCommands` throws on an unknown scope, which catches typos at
install time. `runCommand(id)` runs a command whose `when` holds (tests use it).

- **Bindings** are strings like `Mod+Shift+K` (`Mod` is ⌘), normalized to the order `Mod, Ctrl, Alt, Shift`
  (`cmd`, `meta` and `cmdorctrl` read as `Mod`; a trailing `++` is the `+` key) and shown as `⌘⇧K` by
  `bindingLabel`. `bindingFor(id)` is the user's override (from `keybindings.json`), else the default, else
  none. `bindingFromEvent` reads a keydown: with ⌥, a dead key or a non-Latin character it falls back to the
  physical key (`e.code`), else it keeps `e.key`, so punctuation follows the layout. `useBindingLabel(id)`
  gives a component the current key cap. `conflicts()` lists two commands with the same binding in
  overlapping scopes.
- **Scopes** (`commands/scopes.ts`) form a tree: `registerScope({ id, parent, label, isActive })` throws on an
  unknown parent; `activeScopes()` are those whose `isActive()` holds; `specificity` is the depth. The core
  registers only `global` ("Everywhere", always active; `registerCoreScopes`). Of the commands bound to a key
  in the active scopes, the most specific scope wins.
- **Keys.** `handleKeydown` runs in the capture phase and resolves the key through `resolveKeydown`. Inside
  text inputs only `Mod` and `Ctrl` combinations and Escape fire. A shortcut field that is recording
  (`[data-recording]`) keeps every key. Bare ↵ and Space on a focused button, link or option stay with it
  (`activatesControl`). While a dialog is open, and inside a context menu (`[role="menu"]`), only
  `MODAL_ALLOWED` commands run (shortcuts and zoom). A matched key is prevented and stopped; auto-repeat is
  swallowed unless the command is in `REPEATABLE` (`zoom.in`, `zoom.out`).
- **Global shortcuts** in Settings notation (`shift+cmd+k`) have their helpers in `lib/shortcuts.ts`:
  `shortcutFromKeyEvent` (a key press by physical key; needs ⌘, ⌃ or ⌥ except for function keys),
  `findShortcutConflict` against `SYSTEM_SHORTCUTS` or other actions whatever the spelling, and
  `shortcutKeys` (`["⌃", "⌥", "⌘", "J"]`).

### Remote

`commands/remote.ts`: `answerRemote({ id, request })` answers a `remote` event from another process (the CLI
through the socket) and always replies with `api.remoteReply(id, result, error)`; a thrown error becomes the
`error` text. `RemoteRequest` has no variants yet, so every request is refused as unknown; a failed reply is
logged.

## Palette

The palette is an overlay view (`windows/panel/PaletteView.tsx`, view `palette`): a search field, the groups
Rust returns for what is typed (`api.paletteQuery` on every change; [commands.md](commands.md#palette)), a
footer with one button per prefix. ↑ ↓ move (wrapping), ↵ or a click runs `api.paletteRun(command)`; a failed
run shows its message in the footer. Rows carry the kind's icon, the label, the detail and the shortcut as a
key cap (`shortcutKeys`).

## Dialogs and toasts

`dialog` in the `ui` slice is one `Dialog` at a time; `Dialogs` renders it:

| Dialog | Renders |
|---|---|
| `shortcuts { scopes }` | `ShortcutsDialog`: the bound commands of the scopes active when it opened, most specific scope first, each with its `KeyCap`; only the list scrolls |

`Modal` (`components/Modal.tsx`) is every overlay's frame, portaled to `document.body`: `variant` `sheet` (a
padded dialog over a scrim), `panel` (the overlay surface laid out by its content), `popover` (placed by
`style`, no scrim) or `bare`. It focuses its first control unless a child took focus, keeps Tab inside, closes
on Escape (in the capture phase, so nothing behind also acts; not while a shortcut field records) and on a
press on the scrim, and gives the focus back to the element that opened it.

`toast(kind, message, action?)` adds a `Toast` (`error`, `info`, `success`). An error toast also goes to the
log through `api.logError`. The same kind and message while it shows only counts up (`×2`). At most five show;
the oldest drops out. `Toasts` dismisses them after 2.5 s (success), 4 s (info) or 8 s (error); a toast with an
`action` (a button, e.g. Undo, that dismisses it when run) stays 10 s or its `ttlMs`. An error toast reads
"Something went wrong", its first line as the headline (cut at 160 characters) and the rest behind Details.
`lib/clipboard.ts`'s `copyText(text, what)` copies and confirms with "Copied <what>", or toasts that the
clipboard is not available.

## Components

Shell components in `components/`:

- `Sidebar`: the drag strip under the window buttons (`data-tauri-drag-region`), the day header (weekday and
  date, a click runs `daily:open` today or opens that day's note; ‹ › step to the closest days with a note
  through `showDay`, disabled while `previousDay`/`nextDay` are null; "Back to Today" on another day), the "Tools" nav of `hub.tools` (a `.si` row per tool with
  its `TOOL_INFO` icon, title and `ShortcutCap`, running its command; separators as hairlines), and the
  footer: what Slack shows (`SlackBadge` from `SlackStatus.tsx`, when `hub.slack` is set) and the Settings gear
  (runs `app:settings`; its tooltip names ⌘,).
- `hub/Board.tsx`: the toolbar titled `hub.title`, over the board, and the inspector toggle (`.inspector-toggle`,
  fixed in the window's lower right corner; its tooltip names the key). Its sections come with their steps, in
  the prototype's order: the Today card (the focus title with `hub.focusShortcut`), events and the timeline, a
  hairline, the routine and journal chips, a hairline, today's notes (the title with `hub.notesShortcut`).
- `hub/Inspector.tsx`: the right column: the toolbar ("N open"; the Todos | Activity switch only when
  `hub.activityTab`), then `hub/TodoList.tsx`: `hub.todos` as foldable groups (`uiState.collapsedTodoGroups`),
  each todo a checkbox (`role="checkbox"`, runs `api.todoComplete`, then `loadHub`) and its text, which opens its
  note through `api.todoOpen`. `useOpenTodos` hides a todo while its tick is written and brings it back with a
  toast when the write is refused; a group it empties goes, and the count follows.
- `hub/TodaysNotes.tsx`: the board's last section, `hub.notes` as rows that open the day's note at the topic
  (`api.dailyNoteOpen(day, heading)`).
- `Toolbar`: the unified toolbar a view renders first: `title`, the actions as children;
  its empty space drags the window.
- `Field.tsx`: `KeyCap` (renders nothing for an unbound command), `ShortcutCap` (a Settings-notation
  shortcut as a key cap, through `shortcutKeys` in `lib/shortcuts.ts`), `InsetField` (the macOS field frame: icon,
  control, optional key cap; a click anywhere focuses the input).
- `SegmentedControl`: mutually exclusive buttons (`aria-pressed`) with optional `labels`.
- `Accent.tsx`: `AccentSwatches` (one swatch per `ACCENTS` hue, after a System swatch with `withSystem`).
- `ErrorBoundary`: a render crash shows the message with Try again and Open log folder, and logs it with the
  component stack.

## Settings

`components/settings/SettingsWindow.tsx` fills the Settings window: a nav of `settingsGroups(query)` under a drag
strip for the window buttons, with a search field on top (`SettingsNav`: it keeps the pages whose label, nav group
or section title contains the text; a page found by a section shows the section's title and opens at it; Enter
opens the first), and the shown page's title (a drag region), `intro` and `Body`. `set(patch)` saves the current
settings with the patch through `saveSettings` quietly, so every change applies at once. The window's `page` may
name a section (a former page, `sections` of `registerSettingsPage`): `resolveSettingsPage` returns the page that
holds it, and `SectionTarget` tells the page's `Group anchor="<id>"` to scroll into view.
`controls.tsx` holds the shared rows: `Group` (with an optional `description` and `anchor`), `Row` (`stacked`,
`nested`), `Switch`, `Segmented`, `BufferedInput` (commits on blur or Enter; number spinner steps debounced),
`TextField` (a row with a `BufferedInput`) and `NumberField`; `fields.tsx` the card-per-item fields (`ColorField`,
`ToggleChips` (a few options picked as chips, the chosen ones filled; `contextChoices` titles the contexts), `DaysField` (weekday chips), `TimeRangeField`, `CommitInput` (trimmed, on blur or Enter), `ItemActions` (↑ ↓ when given `onMove`, and a trash icon when given `onRemove`, named Move Up, Move Down and the remove label), `moveItem`, `replaceItem`); `listEditor.tsx` `listEditor(list, save)` gives a list's `change`, `move`, `remove`, `add` and `actions(index, removeLabel?)`, each saving the whole list; `controls.tsx` also has `ControlRow` (controls without a label: a repeating item's lines, or a value right under its switch), `EntryFooter` (an item card's last row: Add on the left, its `ItemActions` at the end), `AddButton` (adds an item below a list of cards), `PathText` (a path with `~/`, selectable) and `SwitchInput` (a bare switch labelled by the caller);
`SlackStatusField.tsx` `SlackStatusEditor`, a Slack status to set: a Slack-like bar (`aria-label` Edit Slack
status) that opens Emoji, Status text and Presence on one line and the Keep the status checkbox below (emoji and
text disabled while it is checked). `controls.tsx` `LeadSwitch` (a workflow block: switch, name or `title`,
`accessory`, then a `.form-grid`), `FormRow` (a small label and a left-aligned control with an optional `hint`)
and `FormText` lay out Workflows, app triggers, absence rules and reminders; `InlineSwitch` is a switch with its
text to the right (a `FormRow` control) and `Checkbox` a checkbox with its label (`.check`).
`pages.tsx` registers the core pages (`registerCoreSettingsPages`):

| Group | Page (id; sections) | Holds |
|---|---|---|
| Setup | Connections (`connections`; `vault`, `calendar`, `slack`, `jira`, `timetracker`) | `ConnectionsPage`, a group each. `VaultGroups`: Vault (the folder through `api.chooseFolder`, its name, the daily notes' folder and format and today's note from `api.vaultInfo`) and Obsidian CLI (its state and a custom path). `CalendarGroup`: the calendar access with Allow Access… or Open Privacy Settings (`useCalendarInfo`: `api.calendarInfo`, read again on `calendar_changed`). `SlackGroup`: not connected, the user token field and Connect (`api.slackConnect`); connected, the identity, Test Connection (`api.slackTest`) and Remove Token (`api.slackDisconnect`); `api.slackInfo`, read again on `slack_changed` ([slack.md](slack.md)). `JiraGroup`: Issue URL (with an example of where 6805 opens) and Default project; Rust validates both on save. `TimeTrackerGroup`: the command line tool (state, version, path; Open TimeTracker when it isn't running); the project card adds `TimeTrackerProjectsField` (linked TimeTracker projects with a colour dot and client, type-to-add through the shared `LinkAdder`, matching project or client names) |
| Setup | Contexts (`contexts`), Projects (`projects`) | one card per item (`ContextsPage`, `ProjectsPage`); a context's calendars are checkboxes of this Mac's calendars (`ContextCalendars`), a chosen one missing here stays listed |
| Obsidian | Journal (`journal`) | `JournalPage`: a card per group (Name, Callout type, Folded, `TimingFields`: the due switch and window, "Opens by itself"; then one line per question: Question, Answer kind and its `ItemActions`, the Hint below; an `EntryFooter` with Add Question and the group's `ItemActions`); Add Group. A renamed group keeps its reminder rule (`journal::follow_renamed_groups` on save) |
| Obsidian | Search (`search`) | `SearchPage`: one row per scope (Scope name, Folders and scopes as a comma list, Move Up/Down, Remove Scope), Add Scope (the first free `scope<n>`), the problems from `api.searchScopes()`; Never search: Excluded folders as a comma list |
| Obsidian | Routines (`routines`) | `RoutinesPage`: a card per routine (Name, with two or more contexts the Contexts as `ToggleChips`, `TimingFields`; then one line per item: Item and its `ItemActions`, Link and the weekday chips below; an `EntryFooter` with Add Item and the routine's `ItemActions`); Add Routine. A new routine gets its id from its name in Rust on save ([routines.md](routines.md)) |
| Obsidian | Daily Note (`daily-note`) | `DailyNotePage`: the fallback folder and format; the sections in `dailyNote.order`, each with its heading level and text, moved up and down (the notes heading is one line, `dailyNote.notes`) |
| Workflows | Workflows (`workflows`; `calls`) | `WorkflowsPage`, in this order: "When you pick a status" (the Slack channel, then a `LeadSwitch` block per status: its workflow switch in `slack.manualWorkflows`, its suggested channel message in `statusMessages`, the first Working's, Off's "Ask first" = `endOfDay.enabled`; the Slack channel and each status's Slack status, `SlackStatusField`) and "End-of-day check" (reminder, question, link, the messages for yes and not yet; the required ones ignore an empty value) ([status.md](status.md#settings)); "Daily focus" (`focus.enabled`, `focus.endOfDayTitle`); `CallWorkflows` (`useCallsInfo`: `api.callsInfo`, read again on `call_changed` and window focus): App triggers, the `calls` section (Watch apps = `meetings.detection`; Now with Check Now running `meetings:check`; Accessibility, Allowed or Allow Access…, warning while an enabled trigger without a built-in check can't read titles; Automation after a refusal), then a `LeadSwitch` block per trigger in the same card (its switch, "During a [name] call" or "While [name] is running", ↑ ↓ and trash; the built-in check's text or the macOS app with Choose… and the window title; its Slack status), Add <missing built-in> and Add Other App… (`api.callsPickApp`), then Joining a Call (`joining`: Start first with remove buttons and Add App…, the app per service with Use Default and Choose…) ([presence-and-calls.md](presence-and-calls.md)); `AbsenceWorkflows`: "Before an absence", a `LeadSwitch` block per rule ("Absence starts within [N] hours", trash; its Slack status) and Add Rule, then "Absences" (titles, busy blocks, time zone label) |
| Workflows | Reminders (`reminders`) | `RemindersPage`: one card, a `LeadSwitch` block per rule in order (its switch and title, ↑ ↓ and trash for meeting and todo rules): the meeting's minutes before, the todos' span and time range (`TimeRangeField openEnd`), the due time of a routine or journal group; Move Up/Down, Remove for meeting and todo rules, Add Upcoming meeting / Open todos |
| Workflows | Links (`links`) | `LinksPage`: one card "Quick access", a line per link (title; collapsed to a summary of its rules; details: URL, Context and Project `ToggleChips`, `LinkRuleFields`: weekdays, time window, meeting with its window and lead minutes; Pinned), Move Up/Down, Remove Link, Add Link |
| Workflows | Music (`music`) | `MusicPage`: one card "Playlists and stations", one line per entry (Name, Kind, the source: a playlist from the library or by name, an artist found with the search, or an Apple Music link; Shuffle for a playlist; ↑ ↓ and trash), then Add Entry and Load My Playlists. The context card adds `ContextMusicFields` (during a call, pause on Break and Done for today, start with the workday, resume) |
| System | General (`general`; `tools`) | `LaunchAtLoginSwitch`: checked from `api.autostartStatus()` through `useApiResult` (the LaunchAgent, not the stored setting), changed through `setLaunchAtLogin`; `ToolsGroup`, the `tools` section: every tool in order with its `TOOL_INFO` (`toolInfo.ts`: label, description, icon), Rail and Window switches (Window off for the Daybook tool), Move Up and Move Down |
| System | Appearance (`appearance`; `rail`) | `AppearancePage`: Look: "Light mode scheme" and "Dark mode scheme", stacked rows of three tiles each (`SchemePicker`, a miniature window per scheme drawn with `schemeLook`, pressed for `lightScheme` or `darkScheme`), Accent (`AccentSwatches withSystem`); Text: Text size (`Segmented` over `TEXT_SIZES`); Day timeline: Starts at and Ends at (`timeline.firstHour`, `lastHour`); then `RailGroups`: Rail, the `rail` section ("Show or Hide the Rail" runs `rail:toggle`), Showing the Rail (show at the edge, edges, trigger zone and span, delays, during meetings with the call's edge and position), Rail look (appearance, shape, size, highlight, position, captions, fade when idle), Rail tooltips (delay, choices) |
| System | Automation (`automation`) | `AutomationPage`: the `daybook` command (path, linked), the HTTP API (switch, state, port, the token with Copy and New Token, curl examples); `api.automationInfo()` polled every 3 s while open |
| System | Shortcuts (`shortcuts`) | `ShortcutsPage`: Global shortcuts from `api.shortcutOverview()` (`GlobalShortcuts`: each marked Global, recorded in Settings notation by `shortcutFromKeyEvent`, saved into `settings.hotkeys`, a conflict with macOS, another global shortcut or a menu key shown as "Also used by …"; a note when this build registers none), the app menu's keys read-only (`MenuShortcuts`), then the page commands of every window with their scope (a search field, the conflicts; recorded in page notation, an X removes, a reset arrow drops an override; saved through `saveKeybindings`). Both use `ShortcutRecorder` (`data-recording`; shortcuts pause meanwhile; ⌫ clears, Escape cancels) |
| System | Status (`status`) | `StatusPage`: diagnostics from `api.diagnosticsGet()` (a card per group, each row's tone icon and optional command button; reloaded on show and focus), then the action log from `api.actionLogGet()` (`ActionLogSection`: day headers, an article per event with its lines, target and state icons, ×N, Clear Log; reloaded on `action_log_changed`) |

## Styling

The CSS is plain and global, imported by `styles/index.ts` in cascade order: the four palettes
`styles/palettes/{content,sidebar}-{light,dark}.css`, `theme.css` (derived tokens, the sidebar's token
mapping, the accents), `context.css` (a context's colour as `--context`), then one stylesheet per layer:
`base.css` (reset, focus, scrollbars, text utilities), `controls.css` (the AppKit-sized controls of
[ux.md](ux.md#controls)), `shell.css` (the grid, the toolbar, the empty view, the crash screen; the Daybook
window's `--hairline`, `--wash` and `--wash-strong`), `sidebar.css`, `board.css` (the board's sections),
`activity.css` (the Activity tab), `overlays.css` (dialogs, menus, toasts, palette, shortcut overview) and
`settings.css`. Icons come from `lucide-react`. Token names and roles are in
[ux.md](ux.md#appearance-and-tokens).

- **Appearance.** `applyAppearance` picks `lightScheme` or `darkScheme` by `prefers-color-scheme` through
  `resolveLook` and `schemeLook` (Default: light content beside a dark sidebar; Light and Dark: one tone for
  both), sets `data-content` and `data-sidebar` on `<html>`, `data-accent` from `accent`, and
  `--ui-font-size` and `--data-font-size` from the `textSize` entry of `TEXT_SIZES`. `useSystemDark` re-renders
  on a macOS switch. `system` maps `--accent` to WebKit's `AccentColor` and `--on-accent` to
  `--on-system-accent`. The native window theme is never set, so it follows macOS
  ([ADR 0010](adr/0010-a-scheme-per-system-mode.md)). The palettes match `[data-content="light|dark"]` and
  `[data-sidebar="light|dark"]`, so any element can preview a look (the scheme tiles do).
- **Sidebar tokens.** The sidebar uses `--sidebar-*` only; inside `.sidebar`, `theme.css` maps `--text`,
  `--o2`, `--o3`, `--line`, `--fill` and `--sel` to the sidebar set, so shared controls follow it, and
  re-derives `--accent` from the hues of its tone.
- **No color literals** (`#hex`, `rgb(`, `hsl(`) outside `styles/palettes/`
  ([ADR 0007](adr/0007-color-palettes.md)); `styles/colors.test.ts` scans.
- **Sizing.** Chrome is in rem: `html { font-size: var(--ui-font-size) }`, so the text size scales it. Spacing
  uses `--space-*`, type the `--fs-*` scale; the toolbar height and the traffic lights are px.
- **macOS look** ([ADR 0011](adr/0011-native-macos-look.md)). The sidebar spans the window's height under the
  traffic lights; without it the toolbar leaves `--traffic-lights` for them. `data-tauri-drag-region` marks
  draggable areas; `user-select: none` applies globally, inputs and `.selectable` opt back in. The body is
  transparent; `sidebar.css` makes the sidebar transparent where its tone matches `prefers-color-scheme`, so
  the window's vibrancy shows through; the Default scheme's dark sidebar in light mode stays opaque.

## Tests

- **Setup.** Vitest with jsdom (`vite.config.ts`, `bun run test:ui`). `src/test-setup.ts` loads the jest-dom
  matchers, cleans up after each test, registers the core scopes and settings pages before each test
  (imported lazily, so a test file's `vi.mock` is in place first), stubs `ResizeObserver`, `matchMedia` and
  the clipboard, and sets `navigator.platform` to Mac, so `Mod` is ⌘.
- **Mocking.** UI tests mock only the `src/api` module:

  ```ts
  vi.mock("../api", async (orig) => ({ ...(await orig<object>()), api: (await import("../test/apiMock")).createApiMock() }));
  const mocked = vi.mocked(api, true);
  mocked.getSettings.mockResolvedValue(settings({ accent: "blue" }));
  ```

  `createApiMock()` (`src/test/apiMock.ts`) returns every `api` member as a `vi.fn()` with a harmless default
  (subscriptions resolve to a no-op unlisten); a new `api` member needs a default there, or the type check
  fails. It imports from `../api/types` only, never the store or `../api`.
- **Fixtures.** `src/test/settings.ts`: `settings(patch)`, Rust's `Settings::default()` (`src/test/default-settings.json`, kept exact by a Rust drift test; `UPDATE_FIXTURES=1` rewrites it) with the orange accent.
- **Store and commands.** Reset with `resetStore(patch)` and `vi.clearAllMocks()` in `beforeEach`; a test of
  keys or the palette calls `installCommands()` and attaches `handleKeydown` itself.
- **Scans.** `test/boundary.test.ts` (Tauri imports only in `src/api/index.ts` and `src/<module>/api.ts`),
  `styles/colors.test.ts` (no color literals outside the palettes; exactly the four palette files; both
  content palettes and both sidebar palettes define the same custom properties), `styles/contrast.test.ts`
  (the math of `styles/contrast.ts`: OKLCH to sRGB, compositing, WCAG contrast; then each content palette's
  `--o2` at 4.5:1 and `--o3` at 3:1 on its background; each sidebar palette's `--sidebar-text` and
  `--sidebar-muted` at 4.5:1 and its tone's accent hues at 3:1 on `--sidebar-bg`).
- **Style.** Query by role or text, drive the UI with `userEvent`, and assert what is visible or which `api`
  call was made. Test files are written by the `write-ts-unit-test` agent
  (`.claude/rules/ts-test-conventions.md`).
