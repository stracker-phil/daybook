# Daybook technical documentation

These docs are for developers and coding agents working on the code. For what the app does and how to
use it, see the [project README](../README.md).

| Page | Covers |
|---|---|
| [architecture.md](architecture.md) | Design rules, process model, repository map |
| [backend.md](backend.md) | Rust core: startup, `AppState`, Tauri commands, events, CLI and single-instance IPC, window lifecycle, tray, persistence, errors, native menu, logging, content security policy |
| [frontend.md](frontend.md) | React app: boot, store, API boundary, command registry, components, styling, tests |
| [vault.md](vault.md) | The vault: daily-note location, sections, note ops, the writer, waiting changes, creating notes, the watcher |
| [commands.md](commands.md) | The command bus, tools and their choices, the menu-bar menu, the app menu, global shortcuts, the palette, adding commands |
| [status.md](status.md) | Working, Break and Off: status memory, the change rule, `status_changed` and its listeners, the status and offer-off overlays |
| [windows.md](windows.md) | Every window, its level and focus rules, the overlay's open protocol, focus hand-back, the pointer screen, what needed objc2 |
| [ux.md](ux.md) | Design language: principles, surfaces, appearance and tokens, type, components, inputs, overlays, keys |
| [testing.md](testing.md) | Unit and integration tests, the test agents, scripts that drive the app in the background |
| [adapting.md](adapting.md) | Identity, platform-specific code, common changes, conventions to keep |
| [adr/](adr/README.md) | Architecture decision records: why the design is the way it is |

Suggested reading order: architecture, then the layer you are changing, then adapting.

The rules in [`.claude/rules/`](../.claude/rules/) are binding for changes:
- [`testing.md`](../.claude/rules/testing.md)
- [`commits.md`](../.claude/rules/commits.md)
- [`rust-test-conventions.md`](../.claude/rules/rust-test-conventions.md)
- [`ts-test-conventions.md`](../.claude/rules/ts-test-conventions.md)
