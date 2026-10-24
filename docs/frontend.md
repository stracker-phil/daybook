# Frontend

`src/` holds a React 19 + TypeScript app, bundled by Vite. State lives in one zustand store. The frontend
renders and forwards intent; [Rust owns the logic](architecture.md#design-rules).

## Boot

1. `index.html` loads `main.tsx`.
2. `main.tsx` calls `installModules()` (`src/modules.ts`) before the first render:
   `registerCoreSettingsPages()` (`components/settings/pages.tsx`), then `installCommands()` (the core scopes,
   then the command registry).
3. `applyInitialLook()` (`lib/theme.ts`) sets the default look for the OS appearance (light mode Default, dark
   mode Dark) and the System accent, so the first paint has colors before settings load.
4. `installNativeMenuGuard()` (`lib/nativeMenu.ts`): a right-click that no app menu handled shows nothing, so
   the WebView's Reload and Inspect Element never appear; text fields keep Cut, Copy and Paste.
5. `installInputModality()` (`lib/inputModality.ts`), see [Input modality](#input-modality).
6. `main.tsx` assigns the store to `window.daybook` only when `import.meta.env.VITE_DEV_HOOKS` is `"1"` (the
   debug bundle of `bun run build:app`, driven by `scripts/dev/js.sh`), then renders
   `<ErrorBoundary><App/></ErrorBoundary>`.
7. `App.tsx` imports the stylesheets (`styles/index.ts`) and runs `useBackendEvents()` once:
   1. `loadSettings()` loads settings and keybindings; then `installCommands()` runs again to apply the
      user's bindings, and `drainLaunches()` pulls the queued launch events (`api.takeLaunchEvents`): `error`
      shows an error toast, `notice` an info toast; `activate` is Rust's and needs nothing from the page.
      `loadUiState()` loads the window layout.
   2. It subscribes to `launch` (drain again), `command` (menu and tray items, to `runMenuCommand`) and
      `remote` (to `answerRemote`).
   3. It adds a capture-phase `keydown` to `handleKeydown`, `error` and `unhandledrejection` to an error
      toast ("Unexpected error: …"), and `beforeunload` to `flushUiState`.
8. `useMenuSync()` starts `installMenuSync()` ([Menu](#menu)).
9. `applyAppearance(settings, systemDark)` runs whenever the settings or the OS appearance change.

```
App
├─ .app grid
│  └─ Workspace     (.workspace, .no-sidebar while uiState.sidebarHidden)
│     ├─ Sidebar    the app name; the Settings gear at the bottom (not rendered while hidden)
│     └─ main       EmptyView
├─ Dialogs          settings, shortcuts, confirm, prompt
├─ CommandPalette
└─ Toasts
```

`main` keeps its place in the tree when the sidebar hides, so a view keeps its state.

## Store

`state/store.ts`: one zustand store, `useStore`, assembled from slices. A `Slice<T>` has one `create(set, get)`
that returns its initial state and actions; `registerSlice(slice)` merges them into the live store and works
after creation, so a module can register its slice late. `resetStore(patch)` runs every registered slice's
`create` again and applies `patch`, for tests. The `Store` interface extends every slice's type; a module adds
its members with `declare module "../state/store" { interface Store { … } }`.

`state/requests.ts` has `fail(e, context)`: an error toast `<context>: <message>` (which also logs it), returning
the `AppError`. The store re-exports it with `flushUiState` and the `Dialog` and `Toast` types.

| Slice | Holds |
|---|---|
| `settings` | `settings` (`null` until loaded), `keybindings`, `loadSettings` (settings and keybindings in parallel; keybindings fall back to `{}`), `saveSettings(settings, { quiet? })` (stores what `api.updateSettings` returns, toasts "Settings saved" unless `quiet`), `saveKeybindings(bindings)` (stores Rust's answer and calls `setOverrides`), `setLaunchAtLogin(enabled)` (`api.autostartSet`, then patches `settings.launchAtLogin` so a later save keeps it; resolves whether it is on now, `null` on a toasted failure) |
| `ui` | `toasts`, `toast(kind, message, action?)`, `dismissToast(id)`, `dialog` (a `Dialog` or `null`), `openDialog`, `closeDialog`, `paletteOpen`, `paletteSeed`, `setPaletteOpen(open, seed?)` |
| `viewState` | `uiState` (`UiState` from Rust: `sidebarHidden`, `sidebarWidth`, `paneSizes`; `EMPTY_UI_STATE` until it loads), `loadUiState`, `setPaneSize(pane, px)`, `toggleSidebar` |

Every `viewState` change is written through `api.setUiState` 500 ms after the last one (drags fire
constantly); `flushUiState` writes a pending change at once.

Components select narrowly (`useStore((s) => s.uiState.sidebarHidden)`), never the whole store.
`lib/useApiResult` loads a value from `api` for a key: `useApiResult(key, fetcher, { onError?, keepData?,
delayMs? })` fetches whenever the key changes (`null` fetches nothing), drops a reply for a superseded key or
after unmount, and returns `{ state, data, error, reload }`.

## API boundary

Only `api.ts` files import `@tauri-apps/api` or `@tauri-apps/plugin-*`: the core's is `src/api/index.ts`, a
module's is `src/<module>/api.ts` ([ADR 0002](adr/0002-ipc-in-api-files.md)); `test/boundary.test.ts` enforces
it. `src/api/index.ts` exports `api`, a flat object of thin `invoke` wrappers for the commands in
[backend.md](backend.md#tauri-commands):

- settings and layout: `getSettings`, `updateSettings` (returns the settings as Rust saved them, clamped by
  `Settings::sanitized`), `getKeybindings`, `setKeybindings`, `getUiState`, `setUiState`;
- launch at login: `autostartSet`, `autostartStatus` (whether the LaunchAgent is installed, the truth that
  `settings.launchAtLogin` mirrors);
- the app: `menuSync`, `takeLaunchEvents`, `remoteReply(id, result, error)`, `hideWindow`, `hideToMenuBar`,
  `quitApp`, `openLogFolder`, `openReadme`, `openUrl` (http(s) only), `chooseFolder` (`null` on cancel),
  `revealPath`, `logError`;
- the subscriptions `onLaunch`, `onWindowOpened`, `onCommand` and `onRemote`, each resolving to its unlisten
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
function called from `installModules()`. Removing a module is deleting its folder and its line; the core
never imports a module. Daybook has no module yet.

`registry/settingsPages.ts` holds the Settings pages by id. `registerSettingsPage({ id, group, label, icon,
intro?, Body, order? })` adds or replaces one; `Body` gets `SettingsPageProps` (`s`, the settings, and
`set(patch)`), and a page that keeps its own state ignores them. `settingsPages()` returns them in nav order:
by group (`GROUP_ORDER`, only `App` so far; other groups follow by name), then `order`, then label.
`settingsGroups()` groups them for the nav, `resolveSettingsPage(id)` returns the named page or the first one,
and `useSettingsPagesVersion()` re-renders a caller when a page registers, so a module may register after the
dialog opened.

## Commands

Every user action that has a shortcut or appears in the palette is a `Command` (`commands/registry.ts`):
`{ id, title, scope, defaultBinding?, when?, run, menu?, checked?, menuOnly? }`. `when` false means the
command neither fires nor shows in the palette; `menu` marks a native menu item; `checked` gives a check item
its mark; `menuOnly` marks a menu item not meant to take a key, which Settings → Shortcuts leaves out.

The core commands are `APP_COMMANDS` (`commands/index.ts`), all in `global`:

| Id | Title | Default | Menu |
|---|---|---|---|
| `settings.toggle` | Settings… | ⌘, | yes |
| `app.launchAtLogin` | Launch at login (`menuOnly`, checked by `settings.launchAtLogin`, enabled once settings loaded) | | yes |
| `shortcuts.show` | Keyboard shortcuts (opens the overview of the active scopes; pressed again closes it) | ⇧? | yes |
| `palette.toggle` | Command palette | ⌘K | yes |
| `sidebar.toggle` | Toggle sidebar (`toggleSidebar`) | ⌘⌃S | yes |
| `zoom.in`, `zoom.out` | Larger text, Smaller text (step through `TEXT_SIZES`, saved quietly; past either end nothing changes) | ⌘= ⌘- | |
| `zoom.reset` | Default text size (`DEFAULT_TEXT_SIZE`) | ⌘0 | |
| `window.hide` | Close window (`api.hideWindow`) | ⌘W | yes |
| `logs.open` | Open log folder | | yes |
| `help.readme` | Open README (`api.openReadme`; a failure toasts) | | yes |
| `app.background` | Hide to Menu Bar (`api.hideToMenuBar`) | ⌘Q | yes |
| `app.quit` | Quit Daybook (`api.quitApp`) | | yes |

`installCommands()` registers the core scopes, registers `APP_COMMANDS` when the registry is empty, and
applies the store's keybindings with `setOverrides`; it is safe to call again. Module scopes must be
registered before their commands: `registerCommands` throws on an unknown scope, which catches typos at
install time. `runCommand(id)` runs a command whose `when` holds (the palette and tests use it).

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
  (`activatesControl`). While a dialog or the palette is open, and inside a context menu (`[role="menu"]`),
  only `MODAL_ALLOWED` commands run (settings, shortcuts, palette, zoom, close, hide, quit). A matched key is
  prevented and stopped; auto-repeat is swallowed unless the command is in `REPEATABLE` (`zoom.in`,
  `zoom.out`), so the native accelerator does not repeat it either.

### Menu

Native menu and tray items emit `command` with a command id; `runMenuCommand(id)` runs it when
`canRunFromMenu` says so (an active scope, `when` not false, not held back by an overlay). A command with
`menu: true` has the same id and default binding as its `MENU_ITEMS` entry in `src-tauri/src/menu/items.rs`.
`installMenuSync()` (`commands/menu.ts`) reports `menuState()`, the menu commands that can run now
(`enabled`) and the checked ones (`checked`), on every store change and every `focusin` and `focusout`;
`syncMenu` sends it through `api.menuSync` only when it changed (a failed send is retried on the next
change), so Rust greys out and checks the items.

### Remote

`commands/remote.ts`: `answerRemote({ id, request })` answers a `remote` event from another process (the CLI
through the socket) and always replies with `api.remoteReply(id, result, error)`; a thrown error becomes the
`error` text. `RemoteRequest` has no variants yet, so every request is refused as unknown; a failed reply is
logged.

## Palette

`CommandPalette` is a `Modal` panel opened by `palette.toggle` (`paletteOpen`), mounted afresh per opening and
per `paletteSeed`, the text it opens with. ↑ ↓ move, ↵ or a click runs the item. It lists
two kinds, from `components/palette/`:

- `parse.ts`: `parsePrefix(input)` splits off the prefix (`>`, commands only) and the needle; `score(fields,
  needle)` ranks a match: 5 equal, 4 prefix, 3 the start of a word, 2 inside a word, 0 none, ignoring case
  (an empty needle matches everything with 1).
- `usePaletteItems(query)`: one provider per kind. Settings pages (`Settings: <label>`, only for a typed needle
  without a prefix) open `openDialog({ type: "settings", page })`; commands are those of the active scopes
  whose `when` holds, with their key caps, except `palette.toggle`. Each section's matches best first, the
  sections by their best match, ties in provider order, at most 80 items. An item closes the palette before it
  acts, so a command that opens it again with a seed wins.
- `PaletteList.tsx`: `PaletteList` (section labels, rows with `KeyCap`) and `PaletteFooter`.

## Dialogs and toasts

`dialog` in the `ui` slice is one `Dialog` at a time; `Dialogs` renders it:

| Dialog | Renders |
|---|---|
| `settings { page? }` | `SettingsDialog`, on the named page or the first ([Settings](#settings)) |
| `shortcuts { scopes }` | `ShortcutsDialog`: the bound commands of the scopes active when it opened, most specific scope first, each with its `KeyCap`; only the list scrolls |
| `confirm { title, message, confirmLabel, cancelLabel?, onConfirm }` | a sheet whose confirm button (danger, focused) closes it, then runs `onConfirm` |
| `prompt { title, label, initial, confirmLabel, onSubmit }` | `PromptDialog`: one line; the confirm button stays disabled while blank, and `onSubmit` gets the text trimmed |

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

- `Sidebar`: the drag strip under the window buttons (`data-tauri-drag-region`), "Daybook", and the Settings
  gear in the footer (its tooltip shows the binding of `settings.toggle`).
- `EmptyView`: the main pane until the first view exists.
- `Toolbar`: the unified toolbar a view renders first: `title`, an optional `detail`, the actions as children;
  its empty space drags the window. No view uses it yet.
- `Field.tsx`: `KeyCap` (renders nothing for an unbound command), `InsetField` (the macOS field frame: icon,
  control, optional key cap; a click anywhere focuses the input) and `FormField` (a labelled field with its
  `AppError.field` error under it).
- `SegmentedControl`: mutually exclusive buttons (`aria-pressed`) with optional `labels`.
- `Accent.tsx`: `AccentSwatches` (one swatch per `ACCENTS` hue, after a System swatch with `withSystem`).
- `ErrorBoundary`: a render crash shows the message with Try again and Open log folder, and logs it with the
  component stack.

## Settings

`components/settings/SettingsDialog.tsx` is a `Modal` panel: a nav of `settingsGroups()` and the shown page's
title, `intro` and `Body`. `set(patch)` saves the current settings with the patch through `saveSettings`
quietly, so every change applies at once. `controls.tsx` holds the shared rows: `Group`, `Row` (`stacked`,
`nested`), `Switch`, `Segmented`, `TextField` and `NumberField`. `pages.tsx` registers the core pages
(`registerCoreSettingsPages`), all in group App:

| Page | Holds |
|---|---|
| General (order 0) | `LaunchAtLoginSwitch`: checked from `api.autostartStatus()` through `useApiResult` (the LaunchAgent, not the stored setting), changed through `setLaunchAtLogin` |
| Appearance (10) | Look: "Light mode scheme" and "Dark mode scheme", stacked rows of three tiles each (`SchemePicker`, a miniature window per scheme drawn with `schemeLook`, pressed for `lightScheme` or `darkScheme`), Accent (`AccentSwatches withSystem`); Text: Text size (`Segmented` over `TEXT_SIZES`) |
| Shortcuts (20) | `ShortcutsPage`: a search field, the conflicts, then every command except `menuOnly` ones with its scope's label; a click records (`ShortcutField`, `data-recording`: the next key is the binding, Backspace unbinds, Escape cancels), an X removes a bound shortcut (saves `null`), a reset arrow drops an override. Saves through `saveKeybindings` |

## Styling

The CSS is plain and global, imported by `styles/index.ts` in cascade order: the four palettes
`styles/palettes/{content,sidebar}-{light,dark}.css`, `theme.css` (derived tokens, the sidebar's token
mapping, the accents), then one stylesheet per layer: `base.css` (reset, focus, scrollbars, text utilities),
`controls.css` (the AppKit-sized controls of [ux.md](ux.md#controls)), `shell.css` (the grid, the toolbar,
the empty view, the crash screen), `sidebar.css`, `overlays.css` (dialogs, menus, toasts, palette, shortcut
overview), `forms.css` and `settings.css`. Icons come from `lucide-react`. Token names and roles are in
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

### Input modality

`installInputModality()` sets `<html data-input="keyboard">` on a key press outside a text field (or Tab in
one; bare modifiers do not count) and removes it on a pointer press, both in the capture phase. Key focus
styles meant only for the keyboard select on it, so a click draws no outline; `base.css` draws the focus ring
with `:focus-visible`. `isEditable` (`lib/nativeMenu.ts`) decides what counts as a text field for both guards.

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
- **Fixtures.** `src/test/settings.ts`: `settings(patch)`, Rust's `Settings::default()` with the orange accent.
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
