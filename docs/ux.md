# Design language

The reference for how Daybook looks and behaves. The design language is TimeTracker's, kept as it is: the look
of a native macOS app, the token sets, the controls. Update this page with every change that builds or changes an
item. Decisions: [ADR 0007](adr/0007-color-palettes.md) (token sets, no literals),
[ADR 0009](adr/0009-three-schemes-four-palettes.md) (schemes and palettes),
[ADR 0010](adr/0010-a-scheme-per-system-mode.md) (a scheme per macOS mode),
[ADR 0011](adr/0011-native-macos-look.md) (the native look),
[ADR 0026](adr/0026-the-daybook-window-follows-the-prototypes-look.md) (the Daybook window follows the
prototype's look), [ADR 0028](adr/0028-overlays-follow-the-prototype.md) (the overlay panel follows the
prototype's look). Implementation (files, registries, CSS structure):
[frontend.md](frontend.md#styling).

It covers the menu-bar menu, the Daybook window (sidebar, board, inspector), the overlay panel and its views,
the rail, the quick notes editor, the Settings window, dialogs, toasts and the native menu.

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
- **Tab never leaves a dialog** (`Modal`); clicking outside closes every overlay except the quick notes editor.
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
| `--chrome` | the Settings page list |
| `--pop` | overlays inside a window: popovers, context menus, dialogs, toasts (the overlay panel is translucent material, see [Overlays](#overlays)) |
| `--fill-quiet` | inset groups (System Settings style): the Settings cards |

Separators are `--line` hairlines (`--sidebar-line` beside the sidebar). Bars inside content stay on `--bg`.
Grouped content is an **inset group** (`.settings-card`): `--fill-quiet`, `--radius` (10px), no border, rows
divided by `--line` hairlines inside the group's 12px padding, no hover fill on plain rows; its heading (13px
semibold `--text`) sits above it, outside the fill. In the Default scheme the dark sidebar is opaque; in the
Light scheme in light mode and the Dark scheme in dark mode the sidebar is transparent, so the window's
`sidebar` material (vibrancy) shows through, and it greys out while the window is inactive.

## Shell

The Daybook window has three columns: the sidebar, the board and the inspector.

The Daybook window takes the prototype's sizes, spacing and colours
([ADR 0026](adr/0026-the-daybook-window-follows-the-prototypes-look.md)); type stays the system font. Its
vertical edges (sidebar | board, board | inspector) are hairlines that fade out at the top and bottom; its
hairlines (`--hairline`) are `--text` at 6%, as light as Glaze's separators. The sidebar, the board and the
inspector each scroll below their titlebar row, with macOS's overlay scrollbars.

- **Sidebar** (`Sidebar`, `.sidebar`): 220px (px, at every text size), full height on its own palette (dark in the
  Default scheme: #212121, white labels and icons, #b1b1b1 for the date). On top a drag strip under the window
  buttons (`.sidebar-titlebar`), then the day (`.day-header`, 16px above, 12px below, 8px at the sides): the
  weekday in 18px semibold (24px line) over the date in 11px `--sidebar-muted`, both in the Mac's locale
  ("Dienstag", "27. Oktober 2026"), one button (8px by 10px padding, 8px radius) that opens the day's note in
  Obsidian, with ‹ › beside it (24 by 20px, white, 30% while there is no day that way; P and N), and on another
  day "Back to Today" (O) under it in `--accent-text`. Below it the tools in Settings → General → Tools order (`.si` rows,
  see [Rows](#rows)): a 16px icon, the title (Music reads "Music · <entry>" while an entry plays) and, where one
  works, the global shortcut as a key chip; the context row in the context's colour (`--context-text`, 500); a
  row that only tells (the call) reads in `--sidebar-muted`; `--sidebar-line` hairlines for the separators (6px
  above and below, 18px in). A tool shows only once its feature exists and while it has something to offer. At
  the bottom, floating 10px from the edges (`.sidebar-footer`): the Slack status as Slack draws it (a 32px pill,
  8px radius, `#1a1d21` with a white 15% frame, 13px `#d1d2d3` text, the emoji at most 64px, "No status" italic
  at 50%, the presence dot `#59d499` while active), then the Settings gear, a borderless 28px `.icon-btn` in
  white whose tooltip names its key (`Settings (⌘,)`). ⌘⌃S hides the sidebar and drops its column; the state is
  kept in `ui_state.json` (`sidebarHidden`).
- **Board** (`.main` on `--bg`, `Board`): the toolbar's title (15px, 500) is the greeting (Good morning, Good
  afternoon, Good evening) or, for another day, its name in the Mac's locale ("Dienstag, 27. Oktober"); below it
  the board (`.board`), whose sections come with their features: the Today card, events and the timeline, the
  routine and journal chips, today's notes. See [The board](#the-board). The main and inspector toolbars have no
  rule below them.
- **Inspector** (`Inspector`, `.inspector`): a 24.5rem column on `--bg` with its own toolbar ("12 open"), its body
  padded 0 8px 16px; it holds the open todos: groups (Today, This Week, Last Week, a month in the Mac's locale, a
  year) whose header ("Oktober (3)", 11px 500 `--o3`, a 12px chevron, padding 12px 8px 2px) folds them, kept in
  `ui_state.json` (`collapsedTodoGroups`). Each todo is a row (10px gap, padding 6px 8px, 8px radius, a 5% hover
  fill): a 16px checkbox (6px radius, the control fill; never shown ticked) and its text over
  "27. Okt. · Notes › Topic" in 11px `--o3`. The checkbox ticks the todo in its note
  ([vault.md](vault.md#open-todos)): it disappears at once, and comes back with an error toast when the note
  changed meanwhile; a click on the text opens its note in Obsidian. Where TimeTracker is offered, a Todos |
  Activity switch in the toolbar (the pressed tab filled with the accent, `--on-accent` text, 13px 500) shows
  TimeTracker's day instead (`ActivityTab`, [timetracker.md](timetracker.md#the-activity-tab)), kept in
  `ui_state.json` (`inspectorTab`); with one tab there is no switch. ⌘⌃I and the `PanelRight` button
  (`.inspector-toggle`, `aria-pressed`, fixed 8px from the window's lower right corner, transparent, `--o3`) show
  and hide it; the state is kept in `ui_state.json` (`inspectorHidden`).
- **An inactive window** dims like the prototype's: while the window isn't key (`window-blurred` on `<html>`),
  toolbar titles and buttons, the inspector toggle and the sidebar's content sit at 40%, and the pressed
  inspector tab turns grey (`--text` at 20%).
- **Unified toolbar** (`Toolbar`, `.toolbar`): every view starts with one 52px row beside the window buttons
  (which sit centered in it, 20px from the left): the view's title (`.toolbar-title`, `--fs-title`, bold,
  `--text-strong`), then its actions at the trailing end,
  with a hairline below (not in the Daybook window). Empty parts drag the window. There is no app title and no Settings button (⌘, the app
  menu, the sidebar gear and the palette open Settings). Without the sidebar the row leaves `--traffic-lights` for
  the window buttons.
- There is no status bar.

## The board

The prototype's column: at most 680px, centred, padded 16px 32px 64px, its blocks 32px apart. Section titles
(`.board-section-title`, `.chip-row-title`) are 11px, 500, `--o3`, 12px in so they line up with the rows, 6px
above them; Focus and Today's notes show their global shortcut beside the title as a key chip, like Journal.

- **Today card** (`.today-card`): a light card (`--text` at 2.5%, 12px radius, 6px padding) holding the reminder
  as one line (`.reminder`, 8px 12px): the label (`Now`, `Next`, `Routine`, …) in 11px `--o3`, the text, and the
  action's name in 11px `--accent-text` at the end. With an action the whole line is the button (hover and focus
  a 5% fill, focus ring) and it takes the focus whenever the window comes up, so ↵ runs it. Under a hairline the
  focus list (its "Focus" title as above): one button over all its lines (6px apart) that opens the focus overlay
  (today only; another day's list is read-only). A task is a 16px checkbox (5px radius, a 10% frame; done: filled
  `--context` with a check in `--on-context`; in progress: a `--context` dot; cancelled: a minus and the text
  struck through in `--o3`), 12px before its project as a tag, then the text; other lines are sub-items, indented
  and 11px `--o2`. Empty today it offers "Set today's focus…" in `--accent-text`.
- **Events** (`.board-section`): "Today's events" (or "Events" on another day), an eye toggle (a bare 16px icon;
  today, with events) that hides past events from the list, never from the timeline, with "N past events hidden"
  under it.
- **Day timeline** (`.timeline`): hour labels (11px on a 16px line, `--text` at 40%, tabular) over 6px ticks, then
  one rounded 36px bar on a 5% fill. The context's work hours are a 3px line along the bar's top (`--context` at
  55%), whole-day events a 3px line along its bottom; event blocks fill the bar between with 4px insets
  (`--context` at 45%, past ones at 22%, a 1px gap where one event runs into the next). The now-line is a 2px
  `--style-red` (Apple's red) line from the labels down. Hovering a block highlights its row (`--context-tint`)
  and the other way round; the highlighted block turns full `--context` and pulses gently (not with reduced
  motion).
- **Event rows** (`.event-row`, 8px 12px): the time (96px, 13px `--o3`, tabular), the title, the call app as a
  badge when the event links a call (a pill on a 10% fill, 11px 500 `--o2`), the calendar in 11px `--o3`. A row
  with a call is the button that joins it. Past rows sit at 50% until hovered.
- **Routines and journal** (`.board-checklists`): one block between two hairlines (12px in), its rows 12px apart.
  Each row has its title in a 112px column and the chips beside it: 36px tall, 8px radius, 12px inside, the 16px
  icon 10px from the title, a 5% fill (10% on hover), then the progress dots (6px; filled `--context`) or a check
  in `--context-text`.
- **Today's notes**: "Open in Obsidian" (11px, `--accent-text`) beside the title; a row per topic (8px 12px, 8px
  radius); empty today "Write a note…" with its icon in `--accent-text`.

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
| `--a-<name>`, `--a-green`, `--accent-text-max-l` | Apple's light system colours, `0.56` | Apple's dark system colours, `1` | the nine accent hues (`ACCENTS`) and green, also the context colours (blue `#007aff`/`#0a84ff`, red `#ff3b30`/`#ff453a`, …); the highest OKLCH lightness of `--accent-text`. They also apply inside a sidebar of the same tone |
| `--style-green` | `#24853a` | `#59d499` | the success tint and the Working dot; in the hue block, so the dark sidebar takes the dark one |
| `--rail-bg`, `--rail-text`, `--rail-line` | `#fcfcfc`, black, black 10% | `#0f0f10`, white, white 10% | the rail's bar and its tooltip card, as the prototype's popover surface |
| `--on-system-accent` | white | white | text on the System accent, as macOS draws it |

**Sidebar palette** tokens (everything inside the sidebar uses only these; `theme.css` maps the generic tokens of
shared controls to them inside `.sidebar`, and the accent hues come from the content palette of the sidebar's
tone):

| Token | Dark | Light | Role |
|---|---|---|---|
| `--sidebar-bg` | `#212121` | `#ebebeb` | the surface where it is opaque |
| `--sidebar-text`, `--sidebar-strong` | white, white | `#262626`, black | entries and their icons, the arrows and the gear; the selected entry and the weekday |
| `--sidebar-muted` | `#b1b1b1` | `#636363` | the date, meta, a row that only tells |
| `--slack-pill-bg`, `-line`, `-text`, `-empty`, `--slack-active` | `#1a1d21`, white 15%, `#d1d2d3`, white 50%, `#59d499` | the same | the Slack status pill, as Slack draws it |
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
| `--on-accent` | white on a hue under OKLCH lightness 0.69, near-black on a lighter one (yellow, orange, green, teal) | text on an accent fill; `--on-system-accent` for System |
| `--context-text`, `--on-context` | `--context` capped like `--accent-text`; the text on a context fill, picked like `--on-accent` | the context's colour as text (the sidebar's context row); a ticked focus box |
| `--focus-ring` | a 3px ring of the accent at 45% | focus on fields, buttons, segments |
| `--ui-font-size`, `--data-font-size` | 13px, 12px | set from the Text size setting by `applyAppearance` (`lib/theme.ts`) |
| `--fs-heading`, `--fs-title`, `--fs-body`, `--fs-meta`, `--fs-caption` | 17, 15, 13, 11 and 10px in rem | the type scale |
| `--space-1` … `--space-6`, `--margin` | 4 to 24px in rem; 20px | spacing; content margins |
| `--control-h`, `--toolbar-control-h` | 22px, 28px in rem | push buttons, fields, popup buttons, menu items; toolbar buttons |
| `--pad` | 3px | the padding above and below a row's line |
| `--font-ui`, `--font-mono` | the system font (SF Pro), SF Mono | fixed |
| `--radius-sm`, `--radius`, `--radius-lg` | 6, 10, 12px | controls; menus, popovers and inset groups; sheets and panels |
| `--sidebar-width` | 220px | the sidebar column (px, as the prototype's) |
| `--toolbar-height`, `--traffic-lights` | 52px, 84px | the unified toolbar (centered on the window buttons); its left inset without the sidebar (px: they don't scale) |

**Accent:** Settings → Appearance "Accent" picks the app accent: System first (a swatch of the accent hues around
a circle; it follows the macOS accent color through WebKit's `AccentColor`, with white text on it), then the
nine hues. System is the default (`SYSTEM_ACCENT`); an unknown value falls back to it.

**Fonts:** none are bundled. The UI uses the system font (`-apple-system`, SF Pro) and machine text SF Mono
(`ui-monospace`); neither is a setting.

## Type

| Token | Size | Used for |
|---|---|---|
| `--fs-heading` | 17px | large headings inside a view |
| `--fs-title` | 15px | toolbar titles, the sidebar's app name, Settings page titles, the palette's field |
| `--fs-body` | 13px (the UI size) | everything else, sheet titles (bold) and group headings (semibold) included |
| `--fs-meta` | 11px | secondary text: meta lines, labels, descriptions, counts |
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

Spacing sits on an 8px grid with 4px half steps (`--space-1` 4px to `--space-6` 24px, in rem so it zooms);
content keeps 20px margins (`--margin`). Rows are at least 28px (inset group rows 36px). Inset groups and
popovers have a 10px radius; sheets 20px padding and a 12px radius. The overlay panel and the Daybook window
use the prototype's px values instead, which the text size doesn't scale.

## Controls

AppKit sizes and looks ([ADR 0011](adr/0011-native-macos-look.md)), in `styles/controls.css`:

| Kind | Class | Look |
|---|---|---|
| Push button | `.btn` | 22px high, 12px side padding, 6px radius, `--control` fill with `--control-shadow`, regular weight; pressed `--control-pressed`; no hover change |
| Default button | `.btn-primary` | the push button filled with the accent and `--on-accent` text (500). Only a sheet's or window's default action, last in its row |
| Destructive | `.btn-danger` | a push button with `--style-red` text (500) |
| Borderless | `.btn-quiet`, `.icon-btn` | no bezel until hovered (`--fill`), pressed `--fill-strong`; icons in `--o2`. `.icon-btn` is a 22px square (`.tiny` 18px) |
| Toolbar button | `.btn` or `.icon-btn` inside `.toolbar` | borderless, 28px high, icon and label, hover bezel |
| Link | `.link-btn` | plain `--accent-text`, no bezel |
| Popup button | `select.input` | WebKit's native chevrons inside the push button bezel |
| Text field | `.input` | 22px, `--field` with a 1px inset `--field-line` frame, 6px radius; focus adds `--focus-ring`; `.input.mono` for machine text |
| Search field | `InsetField` (`.inset-field`) | a `--fill` capsule with a leading icon and an optional key cap; on focus the field surface, frame and ring |
| Segmented control | `SegmentedControl` (`.segmented`) | a 22px `--fill` track with 2px padding; segments in `--text`; the pressed one raised on `--raised` with `--shadow-small`, its label `--text-strong`. The overlay panel has its own (`PanelSegmented`, see [Overlays](#overlays)) |
| Switch | `.switch` | 32 by 18px, a `--fill-strong` track, a white `--knob` thumb with `--shadow-small`; on, the track is the accent |
| Key cap | `KeyCap` (`.keycap`) | the UI font at `--fs-caption`, `--o3` on a `--fill` rounded rect. The overlay panel uses outline caps (`PanelKey`, see [Overlays](#overlays)) |
| Swatches | `AccentSwatches` (`.accent-swatches`) | 16px circles with a hairline; the chosen one ringed; `withSystem` adds the System swatch first |

Buttons in a row end with the default button: Cancel, then Save.

## Components

| Component | Where | Look |
|---|---|---|
| `Toolbar` (`components/Toolbar.tsx`) | the first row of every view | see [Shell](#shell); props `title` and the actions as children |
| Sheet | `Modal` (default `variant="sheet"`): `ShortcutsDialog` | `--pop`, `--radius-lg`, `--shadow`, 20px padding, the title 13px bold, over `--scrim`; buttons in `.actions`, at least 70px wide |
| Toasts | `Toasts` (`.toasts`, `.toast`) | bottom right on `--pop` with `--radius` and `--shadow`; an icon in the kind's tint (`--style-red`, `--style-green`, `--accent-text`), the text selectable, an optional action (Undo) as a link |
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

Every list in a window shares one row: sidebar entries, settings lists (the overlay panel has its own, see
[Overlays](#overlays)). A 20px line with padding above and
below (at least 28px; 36px in inset groups), hairlines between rows, no hover fill on plain rows, `--sel` when
selected. Sidebar entries (`.si`) are the prototype's: 32px tall, 10px inside, the icon 10px from the title, 2px apart, an
8px inset and a 6px radius; a shortcut hint is a key chip (16px tall, 4px inside, 4px radius, SF Mono 11px, a
`--text` 10% frame, `--o3`, at 60%; the same beside board section titles). The active one (`.si.sel`)
is `--sidebar-selected` with its label in `--sidebar-strong` (500) and its icon in the accent. Dimmed rows
(`.dimmed`) take 55% opacity.

## Inputs

Inputs look like AppKit's (see [Controls](#controls)), never like rows. A text field is a 22px frame on
`--field` with a 1px inset `--field-line` and the accent focus ring; a search field (`InsetField`,
`components/Field.tsx`) a `--fill` capsule with a leading icon, the placeholder in `--o3` and an optional key cap
that follows the user's bindings. A select is a popup button.

### Focus rings

A focus ring is for the keyboard only; a click never draws one. Buttons, rows and links show theirs on
`:focus-visible`; text fields keep the accent ring on any focus, since they take typing.

## Overlays

Inside a window: toasts and dialogs, both `--pop` with `--shadow` (a hairline ring and a soft drop; no border).
Toasts take `--radius` (10px), sheets `--radius-lg` (12px) over `--scrim`. Overlays take `Esc` first
(`Modal` stops it in the capture phase). A click on the scrim closes the dialog. Tab cycles inside it in reading
order: fields first, then Cancel and the default button; the first control, or the one a dialog marks with
`autoFocus`, has the focus, and closing returns it to where it was.

The native WebKit context menu is suppressed outside text fields (`lib/nativeMenu.ts`).

Daybook's own overlay panel (a non-activating window over other apps), the rail and the quick notes editor are
separate windows; the rail has [its own section](#the-rail).

**The overlay panel** follows the prototype's look ([ADR 0028](adr/0028-overlays-follow-the-prototype.md)).
`src/windows/panel/panel.css` holds its values in px, so the text size doesn't scale overlays; the shared
parts are in `src/windows/panel/parts/` ([frontend.md](frontend.md#overlay-parts)).

- **Material.** The window is translucent popover material ([windows.md](windows.md#the-overlay)); the card
  (`.panel-card`) is transparent with a 12px radius and a 0.5px hairline (the foreground at 12%). In the card
  the text is the tone's pure foreground (`--text-strong`), secondary `--o2` 60%, tertiary `--o3` 40%, rules
  and the grey selection `--line` / `--list-selection` 10%.
- **Grabber and dragging.** A 10px strip over the card's top (`.panel-grabber`) shows a 36 by 4px bar while the
  pointer is over the overlay. The grabber, the prompt and the footer drag the overlay; buttons, fields and rows
  in them stay clickable, and the caret stays in the field. Every fresh open is centred again.
- **Prompt and field.** The prompt (`PanelPrompt`) is 11px/500 in `--o2`. The field row (`PanelInput`) is 48px:
  a 20px icon in `--o2`, an optional leading part, the 16px field; no rule under it.
- **Rows and selection.** A list (`PanelList`) has a rule on top and 4px 6px padding; rows (`PanelOption`) are
  36px with a 10px gap and 6px radius, 16px icons in `--o3` (`--o2` in the status view) and a 32px shortcut
  column ("⌘1"; 20px for the context switcher's "1"). The selected row is grey (`--list-selection`) in status,
  context, links, projects, quick notes and the wizard, and the context's tint (`--context-tint`) in activity,
  Jira, music, the palette and search. The text stays `--text`; nothing is accent-filled.
- **Section headers** (`SectionHeader`) are 11px/500 in `--o3`, 8px 10px 4px.
- **Current** (`CurrentMarker`): an 11px/500 pill, `--context-text` on `--context-tint`; the row's own context
  in the switcher, else the current context.
- **Footer** (`PanelFooter`): a rule on top, 8px 16px; the error (red) or a status on the left, the key hints
  on the right, 16px apart. A hint is one outline key cap per key (`PanelKey`: 16px tall, 4px radius, a 1px
  `--line` frame, no fill, 11px mono in `--o3`) and a Title Case label in `--o3`, e.g. `↑ ↓ Status`, `↵ Set`,
  `esc Cancel`; views that only show things end with `esc Close`.
- **Project keys** (`ProjectKeys`, activity and focus): ⌘1…⌘9 as key caps with the project names, wrapping.
- **Segmented control** (`PanelSegmented`): 28px with an 8px radius on a 5% track, segments 13px/500, the
  picked one accent-filled with `--on-accent`. Clicks leave the keyboard in the field.
- **Buttons** in the end-of-day and offer-off bodies are 32px with an 8px radius and an inline key cap.

**The focus overlay** (`focus` view, ⌘F11 by default): "Today's focus" with where the list goes on the right
(`--o3`: "Updates your morning message in #team", "Today's note only"); a plain monospace editor (`--font-mono`,
`--fs-meta`, 4 to 14 rows) that saves exactly what was typed; the projects ⌘1…9 insert under it (`ProjectKeys`);
the footer has `⌘ L Check`, `⌘ S Send to Slack` and `⌘ ↵ Save`, with "Sent to Slack" or an error as its status. ⌘L cycles the line's checkbox (open, in progress, done,
cancelled), ⌘1…9 sets the line's project emoji (the same key again makes it a plain sub-item), ↵ continues the
list and ends it on an empty item, Tab and ⇧Tab indent, ⌘↵ saves and closes, ⌘S sends to Slack and stays open.
It stays open while focus is elsewhere. The end-of-day list step uses the same editor.

**The journal overlay** (`journal` view, ⌘J in the app, `journal:open:<group>` for one group): "Journal" with
the groups as the pill segmented control on the right (⌘1…⌘9 switch; `⌘ 1–N Group` in the footer); it opens on the asked group, else the first with
open questions. Each question is its title (semibold), its hint (`--fs-meta`, `--o3`) and its answers as plain
rows: numbered for a list, a checkbox per task (disabled while the row is empty, done ones struck through in
`--o2`), a growing text field for text; a free-text group is one growing field. There is always one empty row at
the end (the only one with a placeholder); ↵ goes to the next row, ↑↓ move between fields, ⌫ on an empty row
removes it. Answers save 0.8 s after typing stops and when the group or the overlay closes; a re-read of the
note replaces the answers only when its trimmed content differs, so the note echoing a save never moves the
caret. The footer has the keys, "Saved once today's note exists" while it doesn't, or the error of a failed save.

**The wizard overlay** (`wizard` view, `routine:<id>`): the routine's name with "2 of 4" or "All done" on the
right (`--o3`, tabular figures), then the day's items as 40px rows with an 8px radius and grey selection: an 18px
round check (filled green, `--a-green`, with a white tick once done) and the title, struck through in `--o3` when
done; an item with a link has a link button on the right. ↑↓ cycle through all items, Space or ↵ ticks, ⌘O opens the link. It opens on the first open item and closes
by itself 0.7 s after the last one is ticked; there is no skip. The footer has the keys (⌘O only for an item with
a link) or the error of a failed tick.

**Quick notes** (⌘F3 by default, "Write a note…" on the board): the picker overlay lists today's topics under
the notes heading (`###` and deeper, their `#`s in `--o3` and indented by level) and filters them as you type;
a title that isn't a topic yet adds a "New topic “…”" row (`###`); the selection is grey. ↑↓ and ↵ (or a click) open the topic in the
editor window; a new one is added to today's note at once, so it shows in Obsidian too. ⌘F3 while the editor is
open brings it forward instead.

**Search** (⌘F in the app; `search:open`): the widest overlay. The field with the scope chips (Glaze's blue
badge: 13px/500 pills on blue 10%, `@work`) on top, the results in a 380px column on the left (name 500, folder
11px in `--o3`, a two-line 11px snippet; rows with an 8px radius; hits marked with an accent tint; the
highlighted row in the context tint) and the preview on the right (the note rendered with GFM; note links follow
in place, missing ones in `--o3`). The pointer never moves the highlight: a click highlights, a second one
opens. The footer has the status (`Copied [[Plan]]`, `Indexing… N notes so far`, an error) and the keys
(`↵ Open`, `⌘ ↵ Open preview`, `⌘ C Copy link`, `⌘ [ Back` and `⌘ ] Forward` when there is history, `esc Cancel`).

**Links, Projects and Jira** (`links:open`, `projects:open`, `jira:open`): Links lists Now, then More links;
Projects lists each project's Launcher projects under its name. Both share `ShortcutListView`: the first nine
rows show ⌘1…⌘9 and open directly, typing filters by every word, a click selects (grey) and a second click
opens; the footer has `↑ ↓ Link` (or `Project`), `↵ Open` (or `Show`) and `esc Close`. The Jira overlay (context
tint) is an address bar: you type a number, key or link, ↓ and ↑ put the last 7 days' keys into the field
(fuzzy-filtered, at most 8), ↵ opens what the field shows.

**Activity** (⌘F2, `activity:switch`, and the check-ins): sticky. The prompt on top ("What are you working on?" or
"Still on Oscorp · QRS-6805?"), then two rows that separate what you work on from the timer
([timetracker.md](timetracker.md#the-activity-overlay)):

- **Activity:** an icon (tooltip "What you work on"), the Daybook projects that count now as a segmented
  control, and the note field, prefilled. Tab, ⇧Tab and ⌘1–9 pick the project. When the projects leave the
  note less than 160px, the row wraps and the note moves under the picker (`panel.css`, the activity input row).
- **Timer:** a timer icon (tooltip "TimeTracker timer"), then "None" (first, the default) and the picked
  project's TimeTracker projects. Shown only when that project links more than one; ⌥1–9 pick one, ⌥0 None.

Then the suggestions (context tint; Meetings, Tickets Today, Recent Timers: the call, today's events, tickets,
recent timers; a recent timer fills both rows), the project keys and the footer (`⇥ Next project` with two or
more projects, `↑ ↓ Suggestions`, `↵ Start` or `↵ Still on it` for an unchanged check-in, `esc Cancel`;
TimeTracker's state as its status). When TimeTracker isn't running the overlay says so with Open TimeTracker
and keeps the text.

**The quick notes editor** (`src/windows/quicknotes/`): a small floating window that keeps its size and place.
The title strip beside the window buttons has the heading's `#`s (`--o3`, mono) and the topic's title, editable
(a new title renames the heading; ↵ goes to the text). Below it CodeMirror 6 with Markdown in `--font-mono`: the
syntax stays visible and dimmed, headings in `--style-yellow`, bold `--style-purple`, italic teal, links
`--accent`, quotes with a bar. ↵ continues a list or quote, Tab and ⇧Tab indent, ⌘L makes a line a task or ticks
it, ⌘B and ⌘I wrap the selection, ⌘Z undoes within the topic. Text saves exactly as typed 0.8 s after typing
stops and when the window loses focus; the status line says "Saving…", "Saved to today's note" or the error.
While nothing is unsaved, a change made in Obsidian shows at once; with unsaved text the save is refused
instead ("Changed in the note meanwhile: your text isn't saved.") and the text stays, until "Load the note"
copies it to the clipboard and shows the note's text. Neither side is overwritten. Esc saves and closes; it
stays open when another app takes focus.

**Presentation** (⌘⌥P in the editor, again or Esc to leave): the editor fills the screen it is on, without title
strip or window buttons, the text in the sans font, `--text-strong` on `--bg`, every line centred and the block
centred vertically. The text starts at 156 pt and shrinks as it grows, down to 42 pt, then scrolls
(`DEFAULT_PRESENTATION_FONT` in `presentation.ts`; `fitFontSize` measures a hidden copy of the text). Typing
saves as before; only an error or the "changed" notice shows, small at the bottom. Nothing animates.

**Other days.** ‹ › (or P, N) show the closest day with a daily note, O and "Back to Today" return; today always
counts. Another day is read-only: its title is the day's name, there is no reminder, the focus list and the
chips don't open overlays, and the date and note topics open that day's note. Commands, keys and overlays always
work on today; the open todos are the same on every day. The board's last section lists the note's topics
("Today's notes", or "Notes") with "Open in Obsidian" in its header: today a row opens its topic in the quick
notes editor ("Write a note…" while there is none), another day's opens the note at it.

## The rail

A slim bar at the screen edge (`src/windows/rail/`) shows when the pointer rests at the edge for the show delay,
or on `rail:toggle`, and hides after the pointer leaves; it never takes focus. From the top: the context's badge,
the status dot (break minutes as its caption), then the tools in their order with short captions (journal "0/6",
the link count), the call button when a call runs. Shapes: pill (a rounded bar with one gliding hover
highlight), docked (flush with the edge) or floating (round buttons that grow on hover); sizes compact, regular
and large; light, dark or the system look. Hovering shows a tooltip card beside the bar (`src/windows/railtip/`):
the label, the key caps, a detail line and, for tools with choices, a menu (contexts, playlists, links) that can
be clicked while the pointer stays on the card. Settings → Appearance holds the rail's edges, trigger, delays,
look and tooltips; Settings → General → Tools chooses which tools it shows. The bar and the tooltip card take the
prototype's surface (`--rail-bg`: near-black in dark, near-white in light; `--rail-text` white or black,
secondary and tertiary at 60 and 40%).

## Contexts

Each context has a colour (Settings → Contexts). `styles/context.css` turns `data-context="<colour>"` on any
element into `--context` (the palette's hue for that colour, Apple's system colours as in the prototype:
`--a-blue`, `--a-magenta` for pink, `--a-gold` for yellow, `--a-green` for green, …), `--context-tint` (16 % of
it), `--context-text` (capped for text on light surfaces) and `--on-context` (text on its fill). Both windows put the current context's
colour on the root (`useContextColor`), so pills, selection tints and progress dots use `var(--context)`; there
is no colour literal outside the palettes.

**The switcher** (⌥⌘F1 with the prototype's keys, `context:switch`, the menu-bar menu's Context submenu): an
overlay listing the contexts, numbered 1…9, the current one marked by a "Current" pill in its colour. ↑↓
move, ↵ or n (also ⌘n) switches, Esc closes; choosing the current one only closes. The menu bar shows the current
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
Results are the overlay's rows with Glaze's command items' 8px radius; the active one takes the context's tint
([Overlays](#overlays)). Group headings are section headers; a row's detail follows its label in 11px `--o3`
(at most 45% of the row). The footer's prefix buttons are an outline key cap with the group's name.

## Settings

- A window of its own (880 × 720, at least 760 × 560), on the pointer's screen; Esc, ⌘W and the close button
  hide it, and it reopens on the page it showed. It has the System Settings layout: the page list on `--chrome`
  (`.settings-nav`, 190px wide, Glaze's sidebar: a drag strip for the window buttons and a search field that
  narrows the list by page, nav group or section title stay put, only the list below scrolls; 26px rows under
  small `--o3` group headings, so the 16 pages fit 720px without scrolling; the current page a grey
  `--fill-strong` fill) and the page on `--bg` (`.settings-pane`): the title in `--fs-title` semibold in the
  strip beside the window buttons (it drags the window), the intro in regular `--text` (what it touches in
  **bold**), then groups 24px apart. Every change applies at once; there is no Save.
- A page has either an intro or group descriptions that say the same, never both; a row's description adds
  what its label doesn't say, or is left out.
- The controls follow Glaze's: a group (`Group`, `.settings-group`) is a 13px semibold `--text` heading, indented
  to the card's content, over its inset group (`.settings-card`, 12px radius, 16px side padding); each row
  (`Row`, `.settings-row`, 14px above and below, at least 48px, very light hairlines between rows) has the
  label and, 4px below it, its description
  (`--fs-meta`, `--o2`, at most 27rem wide, balanced lines) on the left and the control on the right, hairlines
  between rows. A stacked row
  (`.stacked`) puts the control under the label; a row that depends on a switch sits indented under it
  (`.nested`). Number fields and number popups share one width. Counts use tabular figures; paths, bundle ids and
  commands are mono and selectable. A failure reads in red on a light red tint (`.settings-alert`).
- A group may carry a description under its heading (`.settings-group-desc`, `--fs-meta`, `--o2`). A lookup
  reads Found in `--style-green` or Missing in `--style-yellow` (`.settings-state`).
- Pages register in `registry/settingsPages.ts`, in four groups: **Setup** (Connections, Contexts, Projects),
  **Obsidian** (Daily Note, Routines, Journal, Search), **Workflows** (Workflows, Reminders, Links, Music) and
  **System** (General, Appearance, Shortcuts, Automation, Status). Connections holds Vault, Obsidian CLI,
  Calendar, Slack, Jira and TimeTracker as groups; Workflows holds the call triggers and Joining a Call; General
  holds Launch at login and Tools; Appearance holds the Rail. A former page's id (`vault`, `calendar`, `slack`,
  `jira`, `timetracker`, `calls`, `tools`, `rail`) opens its page scrolled to that group.
- Fields and popups are 32px tall with a 10px radius (fields with a quiet border, popups filled); push buttons
  and segmented controls are 28px; checkboxes are
  quiet 16px tiles that fill with the accent and a tick. A few options picked together (contexts, projects,
  weekdays) are chips in one strip, the chosen ones filled with the accent. A repeating item (a routine's item, a
  journal question, a search scope, a daily-note section) is one compact line: its field, then ↑ ↓ and a
  trash icon (quiet `--o2` icon buttons, lighter when disabled), its secondary fields on a line below; an
  item card ends in a row with its Add button and the card's own ↑ ↓ and trash. A due time range sits right
  under its switch.
- A workflow (a status, an app trigger, an absence rule, a reminder) is a block: its switch on the left and its
  name (or a sentence with a field, "During a [Zoom] call") in semibold, its move and remove at the end; below,
  indented past the switch, a two-column form of small `--o2` labels (120px) and controls left-aligned at
  290px; a block switched off stays visible at half opacity; blocks sit 32px apart in one card.
- A list of one-line items (music entries, links, search scopes) shares one card: each line holds all its
  fields, then ↑ ↓ and trash.
- A Slack status to set reads as Slack draws it: a dark bar (the sidebar pill's `--slack-*` tokens) with the
  emoji, the text in white, a divider and the presence. Clicking it opens Emoji, Status text and Presence on
  one line with Keep the status as a checkbox below. Shortcuts lists the global shortcuts first, each marked Global (`.shortcut-global`), then the
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
Settings → General → Tools order (a tool with things to pick is a submenu: its overlay first, then the picks, the current
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
- `--on-accent` (white or near-black, derived per hue) at least 3:1 on every palette accent; Apple's system
  colours leave no single text colour at 4.5:1 on all of them (the System accent takes white, as macOS does).
- Sidebar: `--sidebar-text` and `--sidebar-muted` at least 4.5:1 on `--sidebar-bg`; every accent of the dark
  sidebar's tone at least 3:1 on it. On the light sidebar the light hues (yellow, orange, green, teal) fall below;
  text uses `--context-text`/`--accent-text`, capped in lightness.

Also held, checked by eye: `--text` at least 7:1 on `--bg`, each accent at least 3:1 on `--bg`, and `--line`
visible against `--bg`.
