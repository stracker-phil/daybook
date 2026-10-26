# Windows

`src-tauri/src/window/`. Daybook lives in the menu bar; its windows come and go over whatever the owner works
in. Three windows exist so far; the rail, its tooltip and the quick notes editor come with their steps.

| Window | Label | Kind | Level | Focus |
|---|---|---|---|---|
| Daybook window | `main` | regular window (`tauri.conf.json`) | normal | takes focus and activates Daybook; brings the Dock icon |
| Settings | `settings` | regular window (`window/settings.rs`), built on first use | normal | takes focus and activates Daybook; brings the Dock icon |
| Overlay | `panel` | non-activating `NSPanel` (`window/panel/`) | pop-up menu (101), sticky views floating (3) | takes the keyboard without activating Daybook |

| File | Holds |
|---|---|
| `mod.rs` | the Daybook window: show (on the pointer's screen), hide, the Dock icon; closing a regular window |
| `settings.rs` | the Settings window and its page ids (`PAGES`) |
| `display.rs` | pure geometry: `Rect`, `Screen`, `frame_on_display`, `screen_at`, `top_center` |
| `front_app.rs` | `FrontApp` and the pure close decision `hand_back` |
| `native.rs` | AppKit: screens, the pointer, window frames, the frontmost app, activating and hiding apps |
| `panel/mod.rs` | the overlay: build, open, close, height, focus events |
| `panel/session.rs` | `PanelState`, the pure open and close rules, and `PanelSession` |
| `panel/handshake.rs` | `Handshake`: waits per session for the page's answer |
| `panel/native.rs` | the panel class swap and its Space, level, opacity and key calls |
| `panel/commands.rs` | the Tauri commands |

## What needed objc2

Tauri covers creating, showing, hiding and sizing windows, the Dock icon (`set_dock_visibility`), window
events and running code on the main thread. objc2 (`native.rs`, `panel/native.rs`) covers what it doesn't:

- turning the overlay into a non-activating `NSPanel` that can become key without a title bar (a class swap to
  `DaybookPanel`, as in TimeTracker's timer picker), its collection behaviour, level and alpha;
- `orderFrontRegardless` + `makeKeyWindow` (keyboard without activation), `isKeyWindow`, `orderOut:`;
- `NSScreen` frames and `NSEvent.mouseLocation` (the pointer's screen), `NSWindow` frames in Cocoa
  coordinates;
- `NSWorkspace.frontmostApplication`, `NSRunningApplication` activation, `NSApp isActive` / `hide:`, and the
  on-screen window list (`CGWindowListCopyWindowInfo`) to tell whether an app has a window on the current Space.

The class swap only works while `DaybookPanel` has the same memory layout as tao's `TaoWindow`: the same size
and the `focusable` ivar at the same offset (`same_layout`, which looks through the key-value-observing
subclass AppKit puts on observed windows). tao 0.37.2 keeps that flag in a Rust struct instead, so `Cargo.toml`
pins tao to 0.37.1, as TimeTracker's lock does. On a mismatch the overlay stays a plain window and the log
says so ("The overlay stays a plain window"); it then loses the non-activating behaviour.

Every AppKit call runs on the main thread: window events already do; async code goes through
`panel::on_main` or `AppHandle::run_on_main_thread`.

## The overlay

One window, created hidden at startup (`panel::build`), whose page (`src/windows/panel/`) renders the current
view. Every opening is a new session; the page answers per session.

**Opening** (`panel::open(view, args)`, command `panel_open`):

1. `PanelState::request` decides: a repeat within 250 ms is ignored; the view that is open already closes
   (the same request toggles); anything else becomes a new `PanelSession { session, view, args }`.
2. When the overlay wasn't open yet, the frontmost app (never Daybook itself) and whether Daybook is active are
   remembered for closing.
3. The session goes to the page (`panel_session` event; a page that loads later asks with `panel_session`).
   The page renders it and reports its content height (`panel_ready`), at most 400 ms later.
4. The window takes that height (48 to 640 points), is placed centred on the pointer's screen a fifth of the
   way down (`top_center`), and is shown at alpha 0 with the keyboard (`orderFrontRegardless`,
   `makeKeyWindow`). An overlay already open keeps its place and only resizes.
5. Once the page reports its first painted frame (`panel_painted`, at most 120 ms), alpha goes to 1, so the
   window never flashes the previous view. 200 ms later the panel is made key again if macOS didn't.

While open, the page reports height changes (`panel_set_height`); the window grows downwards from a fixed top
edge. Heights reported after closing are ignored: resizing the hidden window would put it back on screen.

**Sticky views** (`STICKY_VIEWS` in `session.rs`: the activity overlay and `sample-sticky`) are on every Space
at the floating level and ignore focus loss, because macOS can't tell a Space switch from a click elsewhere.
While a sticky view waits unfocused, the app in front is followed every 500 ms, so closing hands focus to
where the owner is now. Other views appear on the active Space (also over a full-screen app) and stay there.

**Keeping open** (`panel_keep_open`, `PanelState::keep_open`): the page can make the open session behave as a
sticky view until it closes, e.g. the end-of-day check while the owner looks at the link it opened. A new
session or closing drops it.

**Focus loss** (`PanelState::lost_focus`): within 400 ms after opening the panel takes the keyboard back
(macOS shuffles focus right after a window appears); later a click elsewhere closes it, and the app that took
focus keeps it.

**A closed overlay is never on screen.** Closing sets alpha 0 before ordering the window out, and a closed
overlay that gets focus anyway (macOS can show it) is hidden again.

## Focus hand-back

Overlays never navigate: closing one hands keyboard focus only to the app the owner looks at now, and never
switches Spaces ([ADR 0016](adr/0016-overlays-never-navigate.md)).

Because the panel doesn't activate Daybook, the app in front usually stays active the whole time; ordering the
panel out returns the keyboard to it. Only when Daybook is active at close (it was activated meanwhile)
`front_app::hand_back` decides:

| Situation | Result |
|---|---|
| Daybook isn't active, or was active when the overlay opened, or another app took focus | nothing |
| the remembered app has a window on the current Space | activate it (`NSRunningApplication`, never `open -b`, which would relaunch or reopen windows) |
| the remembered app is on another Space | nothing: activating it would switch Spaces |
| no remembered app, and no Daybook window visible | hide Daybook, so macOS picks what comes forward |

An activation that fails falls back to hiding Daybook when no Daybook window is visible.

## The Daybook window

- **Pointer screen.** Every show (`window::show_main_in_background`, also `show_main`) first moves the window
  to the screen with the pointer: `frame_on_display` keeps its relative place (a window in a corner stays in
  that corner) and shrinks it to fit a smaller screen.
- **Activation policy.** Daybook is a menu-bar app: the Dock icon shows only while the Daybook window or
  Settings is open. Closing a regular window (close button, ⌘W, ⌘Q, Esc, `daybook hide`) hides it
  (`close_requested`, `hide_regular`); with the last one the Dock icon goes and, when Daybook is active, the
  app hides, so macOS picks what comes forward on this Space instead of Daybook bringing back a remembered app.

## Settings

`window::settings::open(app, page)` (`app:settings` without a page, `settings:<page>` with one; the sidebar
gear and Choose Vault… too):

1. The first time it builds the window: `Settings`, 880 × 720 (at least 760 × 560), not maximizable, an
   overlay title bar with hidden title and the traffic lights at 20/22, loading
   `src/windows/settings/index.html?page=<page>`.
2. Later it sends the page as `open_settings`; without a page the window stays where it was.
3. It shows the Dock icon, moves the window to the pointer's screen, shows it and brings it forward.

Closing hides the window, so it keeps its page and scroll. `PAGES` lists every page id with its title in
the page list's order; `settings:<page>` refuses an id that isn't there (`not_found`), and the palette lists
one "Settings: <Title>" entry per page.

## Page side

`src/windows/panel/` is its own page (`index.html`, a Vite input): `PanelApp` renders the session's view,
reports ready and painted, observes its height, and loads the appearance from Settings for each session.
`commands.ts` registers the `panel` scope with `panel.close` (Escape, also inside text fields) and routes the
window's keys through the registry. `useTabTrap` (`src/lib/`) keeps Tab and ⇧Tab inside the page: leaving the
web content would move focus out of the window, and the overlay would close. Views: `context` (`ContextView`, the switcher), `palette` (`PaletteView`,
[commands.md](commands.md#palette)), `status` (`StatusView` with `EndOfDayListStep` and `EndOfDayStep`), `focus` (`FocusView` with `FocusEditor`, [slack.md](slack.md#the-days-message)), `journal` (`JournalView`, `JournalForm`
with `JournalAnswers`, autosaved through `lib/useAutosave.ts`; [vault.md](vault.md#journal)), `wizard` (`WizardView`, a
routine's checklist; [routines.md](routines.md#the-overlay)) and `offer-off`
(`OfferOffView`) ([status.md](status.md#the-overlay)), and the sample view (`sample`, `sample-sticky`: a text field, a list and a
button; opened in debug builds by `sample:open` and `sample:sticky`).

`scripts/dev/overlay.sh [sticky] [delay]` opens the sample overlay in the debug app after a delay, so you can
switch to another app or Space first.

## Commands and events

| Command | Args | Returns |
|---|---|---|
| `panel_open` | `view: string`, `args: unknown \| null` | – (opens, replaces or toggles the overlay) |
| `panel_close` | – | – |
| `panel_session` | – | `PanelSession \| null` |
| `panel_ready` | `session`, `height` | – |
| `panel_painted` | `session` | – |
| `panel_set_height` | `height` | – |
| `panel_keep_open` | `keep: bool` | – (the open session stays while focus is elsewhere) |

`panel_session` (event, to the `panel` window only): the `PanelSession` to render, `null` once it closed.
