# Stage hook (presentations)

Stage is a separate macOS presenter app. It drives other apps during a demo: it spotlights parts of a window,
shows captions, moves a drawn cursor, and clicks, types and runs commands. Daybook implements the Stage hook,
protocol 1, revision 2026-10-16, with every optional feature (`reveal visible sections ui type.append
keys.window scroll wait`), as the prototype does. The hook does nothing without a presenter.

## Transport

Stage rides on the HTTP API ([automation.md](automation.md#the-http-api)): `/v1/stage/{describe,window,act,state}`
on the same port, with the same token and guards (no `Origin`, a local `Host`). It answers only while the API runs
and Settings → Automation → Presentations is on (`settings.api.presentations`); otherwise `/v1/stage/…` is 404.

**Discovery.** While Presentations is on and the server runs, Daybook writes
`~/Library/Application Support/glaze-stage/apps/<identifier>.json` (atomic, mode 0600): `protocol`, `name`, `id`,
`url`, `token`, `pid`, `updatedAt`. Stage lists the app from it. The file is rewritten after New Token and removed
when the server stops, Presentations goes off, or Daybook quits. Settings also shows `stage register <url>
--token <token>` with Copy, for registering by hand. The debug build shows up as "Daybook (Debug)".

```sh
URL="http://127.0.0.1:27180/v1/stage"; TOKEN="<token from Settings → Automation>"
api() { curl -s -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" "$URL/$1" ${2:+-d "$2"}; echo; }
api describe
api act '{"kind":"command","command":"palette:open"}'
api act '{"kind":"type","anchor":"palette:search","text":"journal","mode":"append"}'
api act '{"kind":"keys","keys":"Enter","window":"panel"}'
```

## Windows and anchors

Five windows are Stage pages: `hub` (the Daybook window, label `main`; view `today` or `day:<date>`,
`ui.inspector` open or closed), `settings` (with its `sections`, the pages in order, and the current one; `window` with a `section` also takes a
former page's id from `window::settings::SECTIONS`, e.g. `calendar`, and opens its page scrolled to that group), `panel` (the overlay, with
its view), `rail` and `quick-notes`. The rail tooltip is not a page.

Commands, state, hotkeys and window frames come from Rust ([ADR 0017](adr/0017-commands-live-in-rust.md));
anchors and their rects come from the pages. An anchor is any element with `data-stage="<id>"`; optional
`data-stage-label`, `data-stage-role`, `data-stage-list` (on an ancestor) and the element's aria and data states
complete it. The ids follow the prototype: `tool:<id>` (sidebar tools), `hub:*` (board parts, the inspector and
its toggle), `panel:input`, `palette:search` and `palette:<command>`, `status:<status>`, `project:<id>`,
`rail:<id>`, `settings-nav:<page>` and `settings:<page>`, `quick-notes:editor`.

`describe.hotkeys` lists the configured global shortcuts even in release builds, which register none while the
prototype runs ([ADR 0018](adr/0018-global-shortcuts-only-in-debug-builds.md)); `keys` then runs the command.

## Code

| Rust (`src-tauri/src/stage/`) | Holds |
|---|---|
| `protocol.rs` | pure: constants, rects, bounds, `flat_state`, the stage URL, `register_command`, `discovery_record` |
| `keys.rs` | pure: `parse_keys`, `canonical_keys`, the notation, `hotkey_for`, `command_for_keys` (`menu::COMMAND_SHORTCUTS`) |
| `requests.rs` | pure: the verbs' parameters (`ActKind`, `WindowRequest`, type mode, scroll, wait timeout, page) |
| `page.rs` | the bridge messages (`StageCollectRequest`, `StageActRequest`, `StageAnchorsReply`, `StageActReply`) |
| `windows.rs` | `StagePage` and its window label, window info, anchors in screen points, sorting, Settings sections |
| `catalog.rs`, `state.rs` | the commands (with `enabled` and `shortcut`), hotkeys and flat state for `describe` |
| `hub.rs`, `inbox.rs` | `Hub`: one request to several pages, replies collected per request id under `Timeouts`; `StageHost` abstracts the windows so tests use a fake |
| `verbs.rs`, `acts.rs` | `describe`, `state`, `window` and `act` with the protocol's status codes and messages |
| `host.rs`, `discovery.rs`, `commands.rs`, `mod.rs` | the Tauri host, the discovery file, the page's commands (`stage_anchors`, `stage_done`, `stage_reveal_file`), install and publish |

The page side is `src/stage/`: `anchors.ts` (collect and measure anchors), `rect.ts`, `actions.ts` (reveal, click as
a pointer sequence at the visible center, type, scroll, keys), `meta.ts` (`useStageMeta`: view, section, ui) and
`hooks.ts` (`installStageHooks(page)`, called from each window's `main.tsx`: answers `stage:collect` and
`stage:act`).
