# Design language

The reference for how Daybook looks and behaves. The design language is TimeTracker's, kept as it is: the look
of a native macOS app, the token sets, the controls. Update this page with every change that builds or changes an
item. Decisions: [ADR 0007](adr/0007-color-palettes.md) (token sets, no literals),
[ADR 0009](adr/0009-three-schemes-four-palettes.md) (schemes and palettes),
[ADR 0010](adr/0010-a-scheme-per-system-mode.md) (a scheme per macOS mode),
[ADR 0011](adr/0011-native-macos-look.md) (the native look). Implementation (files, registries, CSS structure):
[frontend.md](frontend.md#styling).

At this point the app is a shell: the tray icon, the Daybook window with the sidebar and an empty main pane, the
Settings dialog, the command palette, the shortcuts dialog, toasts and the native menu. The surfaces later steps
build (the overlay panel, the rail, the quick notes editor, the Settings window, the views of the Daybook window)
are described here when their step builds them.

## Principles

- **A companion, not a second Obsidian.** Daybook guides the day and gives fast access to the vault; Obsidian
  stays the editor. The app never shows a note as a document to edit, and it never rewrites what the owner typed:
  editors offer editor keys only (list continuation, indent), input is saved verbatim.
- **Calm and native.** A daily companion, not a dev tool: generous spacing, no repeated information, no button
  for what a key already does. It reads as a macOS app beside Calendar and System Settings (unified toolbar, inset
  groups, AppKit-sized controls; [ADR 0011](adr/0011-native-macos-look.md)), never as a web page. The sidebar
  of the Daybook window is dark in the Default scheme.
- **Overlays never navigate.** Closing an overlay hands focus only to the app the owner looks at now; it never
  switches Spaces or brings back the app it opened over. Inside the window, closing a dialog returns the focus to
  where it was.
- **Tab never leaves a dialog** (`Modal`); clicking outside closes every overlay except the quick notes editor
  (step 16).
- **All configuration lives in Settings**, never in vault files or frontmatter. A concept with several options
  gets its own page with one card per item; multi-selects are an inline list with ✕ and a type-to-add field that
  keeps focus.
- Fully keyboard-usable, light and dark. Every action is a command with a rebindable shortcut and a palette
  entry.
- The system font (SF Pro) for all text, titles included, at the macOS text styles; numbers in tabular figures;
  mono (SF Mono) only for machine text such as paths, URLs, ids and commands.
- Hierarchy comes from weight, opacity and space: a strong title, then a normal subtitle, then meta in `--o2`.
- The accent marks interactive things only: keys, links, focus, selection.
- Light: the window opens instantly; the app lives in the menu bar and stays out of the way.

## Surfaces

The window is a native macOS window ([ADR 0011](adr/0011-native-macos-look.md)): neutral greys, no tinted page,
no outlined cards.

| Token | Used for |
|---|---|
| `--sidebar-bg` | the sidebar, full height, under the window buttons; the window's sidebar material instead where the scheme's sidebar tone matches the macOS mode |
| `--bg` | content: the window background under the toolbar and the view |
| `--bg-quiet` | a pane beside the active one |
| `--chrome` | the Settings page list |
| `--pop` | overlays: palette, popovers, context menus, dialogs, toasts |
| `--fill-quiet` | inset groups (System Settings style): the Settings cards |

Separators are `--line` hairlines (`--sidebar-line` beside the sidebar). Bars inside content stay on `--bg`.
Grouped content is an **inset group** (`.settings-card`): `--fill-quiet`, `--radius` (10px), no border, rows
divided by `--line` hairlines inside the group's 12px padding, no hover fill on plain rows; its heading (13px
semibold `--text`) sits above it, outside the fill. In the Default scheme the dark sidebar is opaque; in the
Light scheme in light mode and the Dark scheme in dark mode the sidebar is transparent, so the window's
`sidebar` material (vibrancy) shows through, and it greys out while the window is inactive.

## Shell

The Daybook window has three columns: the sidebar, the board and the inspector.

- **Sidebar** (`Sidebar`, `.sidebar`): full height on its own palette (dark in the Default scheme). On top a drag
  strip under the window buttons (`.sidebar-titlebar`), then the day (`.day-header`): the weekday in
  `--fs-title` bold `--sidebar-strong` over the date in `--fs-meta` `--sidebar-muted`, one button that opens the
  day's note in Obsidian, with ‹ › beside it (borderless, 30% while there is no day that way). Below it the tools
  in Settings → Tools order (`.si` rows, see [Rows](#rows)): icon, title and, where one works, the global
  shortcut as a dimmed key cap; the context row in the context's colour (`--context`, taken in the sidebar's
  tone); a row that only tells (the call) reads in `--sidebar-muted`; `--sidebar-line` hairlines for the
  separators. A tool shows only once its feature exists and while it has something to offer. At the bottom
  (`.sidebar-footer`) a slot for the Slack status, then the Settings gear, a borderless `.icon-btn` in
  `--sidebar-muted` whose tooltip names its key (`Settings (⌘,)`). ⌘⌃S hides the sidebar and drops its column;
  the state is kept in `ui_state.json` (`sidebarHidden`).
- **Board** (`.main` on `--bg`, `Board`): the toolbar's title is the greeting (Good morning, Good afternoon,
  Good evening) or, for another day, its name; below it the board (`.board`), whose sections come with their
  features: the Today card, events and the timeline, the routine and journal chips, today's notes.
- **Inspector** (`Inspector`, `.inspector`): a 24.5rem column on `--bg-quiet` with a `--line` hairline at its
  left edge and its own toolbar; it holds the open todos, then the Activity tab. ⌘⌃I and the toolbar's
  `PanelRight` button (`aria-pressed`) show and hide it; the state is kept in `ui_state.json`
  (`inspectorHidden`).
- **Unified toolbar** (`Toolbar`, `.toolbar`): every view starts with one 52px row beside the window buttons
  (which sit centered in it, 20px from the left): the view's title (`.toolbar-title`, `--fs-title`, bold,
  `--text-strong`), its detail (`.toolbar-detail`, `--fs-meta`, `--o2`), then its actions at the trailing end,
  with a hairline below. Empty parts drag the window. There is no app title and no Settings button (⌘, the app
  menu, the sidebar gear and the palette open Settings). Without the sidebar the row leaves `--traffic-lights` for
  the window buttons.
- There is no status bar.

## Appearance and tokens

The app always follows the macOS appearance; there is no app mode. Settings → Appearance picks a **scheme per
macOS mode**: two rows, "Light mode scheme" and "Dark mode scheme", each with the three scheme tiles under it; a
tile sets that mode's scheme (`lightScheme`, `darkScheme`). The look is the scheme of the current mode, so dark
mode with the Light scheme is all light ([ADR 0010](adr/0010-a-scheme-per-system-mode.md)). The accent is one
setting for both modes.

| Scheme | Sidebar | Content | Default for |
|---|---|---|---|
| Default | dark (opaque) | light | light mode |
| Light | light (vibrant in light mode) | light | |
| Dark | dark (a shade lighter than the content; vibrant in dark mode) | dark | dark mode |

`resolveLook` (`lib/theme.ts`) picks the current mode's scheme, `schemeLook` turns it into a content tone and a
sidebar tone, and `applyAppearance` sets `data-content` and `data-sidebar` on `<html>` (Settings' scheme tiles
set them on their own element); `applyInitialLook` paints the defaults before the settings load. Four palettes in
`src/styles/palettes/` answer them, neutral macOS greys: `content-light`, `content-dark`, `sidebar-light`,
`sidebar-dark`. Palettes of a kind define the same tokens; no color literal appears outside them
(`src/styles/colors.test.ts`).

**Content palette** tokens:

| Token | Light | Dark | Role |
|---|---|---|---|
| `--bg` | `#ffffff` | `#1e1e1e` | content surface, the window background |
| `--bg-quiet` | `#f7f7f7` | `#222222` | a pane beside the active one |
| `--chrome` | `#f5f5f5` | `#252525` | the Settings page list |
| `--pop` | `#ffffff` | `#2c2c2c` | overlays |
| `--text`, `--text-strong` | `#262626`, black | `#e6e6e6`, white | primary text (`--o2`, `--o3` derive from it); titles and pressed segments |
| `--fill-quiet`, `--fill`, `--fill-strong` | black at 3.5, 5.5 and 10% | white at 4.5, 8 and 14% | translucent fills over any surface: inset groups; hover bezels, the segmented track, search fields, key caps; pressed bezels, the switch track |
| `--line` | black at 12% | white at 10% | hairlines |
| `--control`, `--control-pressed` | white, `#ebebeb` | `#4a4a4a`, `#5c5c5c` | push button and popup button bezel; pressed |
| `--control-shadow` | a hairline ring and a 1px drop | a top highlight, a dark ring | the push button bezel's edge |
| `--field`, `--field-line` | white, black at 16% | white at 5%, white at 16% | text field fill and its 1px inset frame |
| `--raised`, `--knob` | white, white | `#5a5a5a`, white | the pressed segment; the switch thumb |
| `--shadow`, `--shadow-small` | | | overlays (a hairline ring and a soft drop; no border); the raised segment and the switch thumb |
| `--scrim`, `--shade` | black 14% | black 35% | behind sheets and panels; black for `color-mix` darkening |
| `--a-<name>`, `--accent-text-max-l` | deep hues, `0.56` | light hues, `1` | the nine accent hues (`ACCENTS`); the highest OKLCH lightness of `--accent-text`. They also apply inside a sidebar of the same tone |
| `--on-accent` | white | `#1a1a1a` | text on any palette accent's fill, at least 4.5:1 on every hue; like the hues, it follows the sidebar's tone inside the sidebar |
| `--on-system-accent` | white | white | text on the System accent, as macOS draws it |
| `--style-green` | | | the success tint; red, yellow, blue and purple are accent hues (`theme.css`) |

**Sidebar palette** tokens (everything inside the sidebar uses only these; `theme.css` maps the generic tokens of
shared controls to them inside `.sidebar`, and the accent hues come from the content palette of the sidebar's
tone):

| Token | Dark | Light | Role |
|---|---|---|---|
| `--sidebar-bg` | `#262626` | `#ebebeb` | the surface where it is opaque |
| `--sidebar-text`, `--sidebar-strong` | `#e6e6e6`, white | `#262626`, black | entries; the selected entry and the app's name |
| `--sidebar-muted` | `#9e9e9e` | `#636363` | icons (the Settings gear), key caps, meta |
| `--sidebar-hover`, `--sidebar-selected` | white at 5 and 12% | black at 4 and 9% | hovered and selected entry (translucent, so they work on the material) |
| `--sidebar-line` | white at 10% | black at 10% | its edge, separators |

Derived tokens in `src/styles/theme.css`, the same for every palette:

| Token | Value | Role |
|---|---|---|
| `--accent` | `AccentColor` for `system`, else `var(--a-<choice>)`, via `[data-accent]`; re-derived inside the sidebar | interactive things only |
| `--style-red`, `-yellow`, `-blue`, `-purple` | `--a-red`, `--a-gold`, `--a-blue`, `--a-purple` | status tints (an error background is a `color-mix()` of `--style-red`) |
| `--o2`, `--o3` | `--text` at 68% and 52% | secondary and tertiary text |
| `--sel` | `--accent` at 20% (`--sidebar-selected` in the sidebar) | selected row |
| `--accent-text` | `--accent` with its lightness capped at `--accent-text-max-l` | the accent as text |
| `--focus-ring` | a 3px ring of the accent at 45% | focus on fields, buttons, segments |
| `--ui-font-size`, `--data-font-size` | 13px, 12px | set from the Text size setting by `applyAppearance` (`lib/theme.ts`) |
| `--fs-display`, `--fs-heading`, `--fs-title`, `--fs-body`, `--fs-meta`, `--fs-caption` | 22, 17, 15, 13, 11 and 10px in rem | the type scale |
| `--space-1` … `--space-6`, `--space-8`, `--margin` | 4 to 24px and 32px in rem; 20px | spacing; content margins |
| `--control-h`, `--toolbar-control-h` | 22px, 28px in rem | push buttons, fields, popup buttons, menu items; toolbar buttons |
| `--row-h`, `--pad` | at least 20px plus 2 × 3px | the row anatomy (px: it grows only for large data sizes) |
| `--font-ui`, `--font-mono` | the system font (SF Pro), SF Mono | fixed |
| `--radius-sm`, `--radius`, `--radius-lg` | 6, 10, 12px | controls; menus, popovers and inset groups; sheets and panels |
| `--sidebar-width` | 16rem | the sidebar column |
| `--toolbar-height`, `--traffic-lights` | 52px, 84px | the unified toolbar (centered on the window buttons); its left inset without the sidebar (px: they don't scale) |
| `--scrollbar-size`, `--scrollbar-thumb` (`-hover`, `-active`) | 8px; `--o3` at 55%, `--o3`, `--o2` | the one scrollbar (`base.css`) |

**Accent:** Settings → Appearance "Accent" picks the app accent: System first (a swatch of the accent hues around
a circle; it follows the macOS accent color through WebKit's `AccentColor`, with white text on it), then the
nine hues. System is the default (`SYSTEM_ACCENT`); an unknown value falls back to it.

**Fonts:** none are bundled. The UI uses the system font (`-apple-system`, SF Pro) and machine text SF Mono
(`ui-monospace`); neither is a setting.

## Type

| Token | Size | Used for |
|---|---|---|
| `--fs-display` | 22px | large numbers |
| `--fs-heading` | 17px | large headings inside a view |
| `--fs-title` | 15px | toolbar titles, the sidebar's app name, Settings page titles, the palette's field |
| `--fs-body` | 13px (the UI size) | everything else, sheet titles (bold) and group headings (semibold) included |
| `--fs-meta` | 11px | secondary text: meta lines, labels, descriptions, counts, toolbar details |
| `--fs-caption` | 10px | key caps |

Numbers use tabular figures (the body sets `tabular-nums`); `--data-font-size` (12px) sizes data values. Both
follow one setting, Appearance → "Text size": Small (12 and 11px), Medium (13 and 12px, the default) or Large
(15 and 14px), a segmented control without a pixel readout; ⌘= and ⌘- step through the three (⌘0 returns to
Medium). Chrome sizes are in rem of the UI size. Weights 400, 500, 600 and 700 (titles). Never mono in the UI:
mono is for paths, bundle ids, URLs and commands only.

## Words

One word for each thing, in every label, menu item, palette title, toast and in this page. The UI says
**Project** (the prototype's code says client). Undo is "Undo" (and "Redo") everywhere. Code identifiers keep
their names; accessible names follow the visible labels.

## Spacing and radius

Spacing sits on an 8px grid with 4px half steps (`--space-1` 4px to `--space-8` 32px, in rem so it zooms);
content keeps 20px margins (`--margin`). Rows are at least 28px (inset group rows 36px). Inset groups and
popovers have a 10px radius; sheets 20px padding and a 12px radius.

## Controls

AppKit sizes and looks ([ADR 0011](adr/0011-native-macos-look.md)), in `styles/controls.css`:

| Kind | Class | Look |
|---|---|---|
| Push button | `.btn` | 22px high, 12px side padding, 6px radius, `--control` fill with `--control-shadow`, regular weight; pressed `--control-pressed`; no hover change |
| Default button | `.btn-primary` | the push button filled with the accent and `--on-accent` text (500). Only a sheet's or window's default action, last in its row |
| Destructive | `.btn-danger` | a push button with `--style-red` text (500) |
| Borderless | `.btn-quiet`, `.icon-btn` | no bezel until hovered (`--fill`), pressed `--fill-strong`; icons in `--o2`. `.icon-btn` is a 22px square (`.tiny` 18px) |
| Split button | `.split-btn` with `.split-btn-menu` | two push buttons joined, the second a chevron for a menu |
| Toolbar button | `.btn` or `.icon-btn` inside `.toolbar` | borderless, 28px high, icon and label, hover bezel |
| Link | `.link-btn` | plain `--accent-text`, no bezel |
| Popup button | `select.input` | WebKit's native chevrons inside the push button bezel |
| Text field | `.input` | 22px, `--field` with a 1px inset `--field-line` frame, 6px radius; focus adds `--focus-ring`; `.input.mono` for machine text |
| Search field | `InsetField` (`.inset-field`) | a `--fill` capsule with a leading icon and an optional key cap; on focus the field surface, frame and ring |
| Segmented control | `SegmentedControl` (`.segmented`) | a 22px `--fill` track with 2px padding; segments in `--text`; the pressed one raised on `--raised` with `--shadow-small`, its label `--text-strong` |
| Switch | `.switch` | 32 by 18px, a `--fill-strong` track, a white `--knob` thumb with `--shadow-small`; on, the track is the accent |
| Key cap | `KeyCap` (`.keycap`) | the UI font at `--fs-caption`, `--o3` on a `--fill` rounded rect |
| Secondary label | `.secondary-label` | plain `--fs-meta` `--o2` text after a name; never a chip or badge |
| Swatches | `AccentSwatches` (`.accent-swatches`) | 16px circles with a hairline; the chosen one ringed; `withSystem` adds the System swatch first |

Buttons in a row end with the default button: Cancel, then Save; a destructive or secondary action sits at the
leading end (`.actions-start`).

## Components

| Component | Where | Look |
|---|---|---|
| `Toolbar` (`components/Toolbar.tsx`) | the first row of every view | see [Shell](#shell); props `title`, `detail` and the actions as children |
| Sheet | `Modal` (default `variant="sheet"`): `PromptDialog`, `ShortcutsDialog` | `--pop`, `--radius-lg`, `--shadow`, 20px padding, the title 13px bold, over `--scrim`; buttons in `.actions`, at least 70px wide |
| Panel | `Modal variant="panel"`: the palette, Settings | the overlay surface laid out by its content, `--radius-lg`, over `--scrim` |
| Popover | `Modal variant="popover"` | `--pop`, `--radius`, `--shadow`, placed under its control, no scrim |
| Context menu | `ContextMenu` (`.menu`) | see [Overlays](#overlays) |
| Toasts | `Toasts` (`.toasts`, `.toast`) | bottom right on `--pop` with `--radius` and `--shadow`; an icon in the kind's tint (`--style-red`, `--style-green`, `--accent-text`), the text selectable, an optional action (Undo) as a link |
| Splitter | `Splitter` (`.splitter-x`, `.splitter-y`) | a `--line` hairline with a wider invisible grab area, like an NSSplitView divider; the accent while it has the keyboard focus |
| Crash page | `ErrorBoundary` (`.crash`) | the error in mono `--style-red` |

Toasts dismiss themselves (success after 2.5 s, info 4 s, errors 8 s, one with an action 10 s); an error reads
"Something went wrong" over its first line, with Details for the rest, and repeats count up (`×3`) instead of
stacking.

## Opacity levels

| Level | Token | Opacity | Used for |
|---|---|---|---|
| Primary | `--text` | 100% | the thing you came for |
| Secondary | `--o2` | 68% | labels, units, relative times, inactive items, meta (at least 4.5:1 on `--bg`) |
| Tertiary | `--o3` | 52% | counts, empty states, placeholders (at least 3:1 on `--bg`) |

## Rows

Every list shares one row: sidebar entries, palette results, settings lists. A 20px line with padding above and
below (at least 28px; 36px in inset groups), hairlines between rows, no hover fill on plain rows, `--sel` when
selected. Sidebar entries (`.si`) have 2px more padding, an 8px inset and a 6px radius; the active one (`.si.sel`)
is `--sidebar-selected` with its label in `--sidebar-strong` (500) and its icon in the accent. Dimmed rows
(`.dimmed`) take 55% opacity.

## Inputs

Inputs look like AppKit's (see [Controls](#controls)), never like rows. A text field is a 22px frame on
`--field` with a 1px inset `--field-line` and the accent focus ring; a search field (`InsetField`,
`components/Field.tsx`) a `--fill` capsule with a leading icon, the placeholder in `--o3` and an optional key cap
that follows the user's bindings. A select is a popup button. A labelled form field (`FormField`, `.form-field`)
puts its label in `--fs-meta` `--o2` above the control and the error from `AppError.field` in `--style-red`
below it.

### Focus rings

A focus ring is for the keyboard only; a click never draws one. Buttons, rows and links show theirs on
`:focus-visible`; text fields keep the accent ring on any focus, since they take typing. Key focus that a view
draws itself (a `.focused` element) shows only while `<html data-input="keyboard">` is set
(`lib/inputModality.ts`): a key press outside a text field (or Tab in one) sets it, a pointer press clears it.

## Overlays

Inside the window: palette, popovers, context menus, toasts and dialogs, all `--pop` with `--shadow` (a hairline
ring and a soft drop; no border). Menus, toasts and popovers take `--radius` (10px), sheets and panels
`--radius-lg` (12px). Sheets and panels sit over `--scrim`; popovers sit over nothing. Overlays take `Esc` first
(`Modal` stops it in the capture phase). A click on the scrim closes the dialog. Tab cycles inside it in reading
order: fields first, then Cancel and the default button; the first control, or the one a dialog marks with
`autoFocus`, has the focus, and closing returns it to where it was.

Context menus use the UI font, 22px items with a 6px radius; the hovered or focused item is `--accent` with
`--on-accent` text, disabled items `--o3`, separators an inset 1px `--line`; an item can carry a detail line
(`.with-detail`) or a check (`.checkable`). The native WebKit context menu is suppressed outside text fields
(`lib/nativeMenu.ts`).

Daybook's own overlay panel (a non-activating window over other apps), the rail and the quick notes editor are
separate windows; they come with steps 04, 16 and 20 and get their sections here then.

## Contexts

Each context has a colour (Settings → Contexts). `styles/context.css` turns `data-context="<colour>"` on any
element into `--context` (the palette's hue for that colour: `--a-blue`, `--a-magenta` for pink, `--a-gold` for
yellow, `--style-green` for green, …) and `--context-tint` (16 % of it). Both windows put the current context's
colour on the root (`useContextColor`), so pills, selection tints and progress dots use `var(--context)`; there
is no colour literal outside the palettes.

**The switcher** (⌥⌘F1 with the prototype's keys, `context:switch`, the menu-bar menu's Context submenu): an
overlay listing the contexts, numbered ⌘1…⌘9, the current one marked by a "Current" pill in its colour. ↑↓
move, ↵ or ⌘n switches, Esc closes; choosing the current one only closes. The menu bar shows the current
context's emoji.

**Settings → Contexts and Projects** (group Setup) have one card per item, with Move Up, Move Down and
Remove, and Add below the cards. A context: name, emoji, features, colour swatches, search scope, work hours
(weekday toggles and times) and switching automatically when they start. A project: name, active, contexts
(an inline list with ✕ and a type-to-add field that keeps the focus), meeting words and Slack emoji.

## Palette

⌘K, the palette tool and its global shortcut open it as an overlay over any app (`PaletteView`); ⌘K again
closes it. The field reads "Search commands, links, projects, music and contexts…". Empty, each group shows
its first six; typing ranks the matches (the name's start, then a word's start, then every word somewhere,
then the letters in order) and shows eight per group. Groups: Commands, Links, Projects, Music, Contexts. A
first character narrows to one group and lists all of it: `>` commands, `#` links, `/` projects, `:` music,
`@` contexts; the footer has a button per prefix. A command reads as its menu path (`Music: Pause`), with
its global shortcut as a key cap; a link names its project as detail, a current context or playlist says
"Current". Disabled commands are absent.

↑ ↓ move, ↵ or a click runs, Esc closes. A pick that brings a window or another app forward (the Daybook
window, Settings, a note, a link) closes the palette first and leaves that in front; a pick that opens another
overlay replaces the palette; anything else runs and the palette closes, handing focus back to where you were.
Results use the row anatomy with a 6px radius; the active one is filled with the accent and `--on-accent`
text, as in Spotlight. Group headings are small `--o3` text; a row's detail follows its label in `--o3`.

## Settings

- A window of its own (880 × 720, at least 760 × 560), on the pointer's screen; Esc, ⌘W and the close button
  hide it, and it reopens on the page it showed. It has the System Settings layout: the page list on `--chrome`
  (`.settings-nav`, under a drag strip for the window buttons; 28px rows under small `--o3` group headings; the
  current page filled with the accent and `--on-accent` text, as a focused macOS sidebar) and the page on
  `--bg` (`.settings-pane`): the title in `--fs-title` bold in a 52px row that drags the window, the intro in
  `--o2`, then groups. Every change applies at once; there is no Save.
- A group (`Group`, `.settings-group`) is a 13px semibold `--text` heading over its inset group
  (`.settings-card`); each row (`Row`, `.settings-row`, at least 36px) has the label and its description
  (`--fs-meta`, `--o2`) on the left and the control on the right, hairlines between rows. A stacked row
  (`.stacked`) puts the control under the label; a row that depends on a switch sits indented under it
  (`.nested`). Number fields and number popups share one width. Counts use tabular figures; paths, bundle ids and
  commands are mono and selectable. A failure reads in red on a light red tint (`.settings-alert`).
- A group may carry a description under its heading (`.settings-group-desc`, `--fs-meta`, `--o2`). A lookup
  reads Found in `--style-green` or Missing in `--style-yellow` (`.settings-state`).
- Pages register in `registry/settingsPages.ts`, in four groups: **Setup** (Vault, Contexts, Projects),
  **Obsidian** (Daily Note), **Workflows** (with its first page) and **System** (General, Tools, Shortcuts,
  Appearance). Shortcuts lists the global shortcuts first, each marked Global (`.shortcut-global`), then the
  app menu's keys (fixed), then the windows' own keys; a key field records the next key, Escape stops, ⌫
  unbinds; conflicts show in `--style-yellow`. A concept with several items gets a page with one card per item;
  a fixed list (sections, tools) moves its rows up and down.
- A scheme tile (`.scheme-option`, `.scheme-tile`) is a miniature window in the scheme's look; the chosen one is
  ringed in the accent. There is no Logging page: Help → Open Log Folder opens the logs and `--log-level` sets
  their level for a run.

## Keys

App-level actions are Rust commands ([commands.md](commands.md)): the app menu gives them their keys in a
Daybook window, global shortcuts (Settings notation, `ctrl+alt+cmd+f`) run them from anywhere. The page's
registry (`APP_COMMANDS` in `src/commands/index.ts`) only has the keys inside a window, rebindable in
Settings → Shortcuts (`keybindings.json`). `?` lists the bound page commands of the scopes active when it
opens, most specific first, in a sheet: only the list scrolls, Close stays below it.

Rules of the page's keys (`handleKeydown`):

- Bare keys never fire in text fields; there only ⌘ and ⌃ combinations and Escape do.
- While a dialog is open, only `?` and the text size keys run (`MODAL_ALLOWED`); every other key belongs to
  the dialog. A context menu keeps its own keys the same way.
- ↵ and Space on a focused button or link activate it.
- A held key repeats only the text size steps.
- A command has one binding.
- `Tab` is never bound: it moves the keyboard focus, and dialogs keep it inside (see [Overlays](#overlays)).

| Command | Label | Default key | Scope |
|---|---|---|---|
| `shortcuts.show` | Keyboard shortcuts | ? | Daybook window |
| `sidebar.toggle` | Toggle sidebar | ⌘⌃S | Daybook window |
| `inspector.toggle` | Toggle inspector | ⌘⌃I | Daybook window |
| `window.close` | Close window (not while editing text; a dialog takes Escape first) | Esc | every window |
| `zoom.in` / `zoom.out` / `zoom.reset` | Larger text / Smaller text / Default text size | ⌘= / ⌘- / – | every window |

Keys of controls, not commands (they act on the focused control and are not rebindable):

| Where | Keys |
|---|---|
| Dialogs | Tab and ⇧Tab cycle inside; Escape closes |
| Overlays | Tab stays inside; Escape closes |
| Palette | ↑↓ move, ↵ runs, Escape closes |
| Context menu | ↑↓ items, ↵ runs, Escape closes |
| Settings → Shortcuts | a key field records the next key (global shortcuts and the menu's keys pause meanwhile); Escape stops, ⌫ unbinds |

Global shortcuts are registered only by the debug build while the prototype runs
([ADR 0018](adr/0018-global-shortcuts-only-in-debug-builds.md)).

## Menu bar

The app menu (only visible while a Daybook window is in front) runs Rust commands; an item whose command
does not exist yet is left out, a disabled one is greyed out.

| Menu | Items (command; key) |
|---|---|
| Daybook | About; Settings… (`app:settings`; ⌘,); Services; Hide, Hide Others, Show All; Close Window (`window:close`; ⌘Q) |
| File | Command Palette… (`palette:open`; ⌘K), Search Vault… (⌘F), Journal… (⌘J), Daybook (`hub:open`; ⌘0); Close Window (`window:close`; ⌘W) |
| Edit | Undo, Redo; Cut, Copy, Paste, Select All (native) |
| Window | Minimize, Zoom, Bring All to Front (native) |
| Help | Open Log Folder (`app:logs`), README (`app:readme`); Sample Overlay, Sample Overlay (Sticky) in debug builds |

⌘Q only closes the window in front, as ⌘W does: Daybook keeps running in the menu bar, and Quit is in the
menu-bar menu.

The menu-bar menu: Create Today's Note while it is missing (with how many changes wait for it), the tools in
Settings → Tools order (a tool with things to pick is a submenu: its overlay first, then the picks, the current
one ticked), the links, Open Today's Note, Settings…, Quit Daybook; without a vault only Choose Vault….
Global shortcuts show next to their items. The item's title is the current context's label (an emoji).

## Scheme audit

The three schemes are checked in both modes. `scripts/dev/screenshot-looks.sh <dir>` sets the current macOS
mode's scheme to each of the three through `scripts/dev/js.sh`, captures the window as
`<dir>/<scheme>-<mode>.png` and restores the settings; the app follows macOS, so switch the system appearance
and run it again for the other mode. Each scheme is one look in either mode; the mode changes the native window
chrome and, where the sidebar's tone matches it, lets the sidebar material show through. Fixes live in
`src/styles/palettes/` only.

Targets, computed by `src/styles/contrast.ts` (WCAG contrast; OKLCH as the CSS computes it) and checked by
`src/styles/contrast.test.ts`:

- `--o2` at least 4.5:1 and `--o3` at least 3:1 on `--bg`, in both content palettes.
- `--on-accent` at least 4.5:1 on every palette accent (the System accent takes white, as macOS does).
- Sidebar: `--sidebar-text` and `--sidebar-muted` at least 4.5:1 on `--sidebar-bg`; every accent of the
  sidebar's tone at least 3:1 on it.

Also held, checked by eye: `--text` at least 7:1 on `--bg`, each accent at least 3:1 on `--bg`, and `--line`
visible against `--bg`.
