# AGENTS.md

Instructions for a coding agent that clones, builds or adapts Daybook. Humans: see [README.md](README.md).

Daybook is one developer's personal tool, published as source. There are no releases, binaries or support. As
shipped it builds and runs on **macOS on Apple Silicon** only; everything else is a port.

## Repository facts

- Tauri 2 app: Rust backend in `src-tauri/`, React 19 + TypeScript frontend in `src/`, bun as package manager.
- Rust owns all logic; React only renders. Read [docs/architecture.md](docs/architecture.md) before changing
  code, and the page of the layer you touch ([docs/README.md](docs/README.md) is the index).
- [CLAUDE.md](CLAUDE.md) and [.claude/rules/](.claude/rules/) are the maintainer's conventions (tests,
  commits). Follow them in this repo; in a fork, keep what serves you.
- IPC types in `src/api/types.gen.ts` are generated from Rust with ts-rs (`bun run types`); never edit them.
- The data is an Obsidian vault plus small JSON files in the config folder; there is no database
  ([ADR 0012](docs/adr/0012-vault-and-json-no-database.md)).

## Prerequisites (macOS)

| Tool | Check | Notes |
|---|---|---|
| Xcode Command Line Tools | `xcode-select -p` | `xcode-select --install` |
| rustup, stable toolchain | `rustup show` | `rust-toolchain.toml` pins `stable` with rustfmt and clippy. A Homebrew rustup is keg-only: put `/opt/homebrew/opt/rustup/bin` on `PATH` |
| bun | `bun --version` | |

## Clone and build

```sh
git clone <this repository>
cd daybook
bun install
bun run build:app        # debug bundle: src-tauri/target/debug/bundle/macos/Daybook (Debug).app
bun run release          # release bundle: src-tauri/target/release/bundle/macos/Daybook.app and a .dmg
```

The first Rust build takes several minutes. `bun run release` also installs the build (`scripts/install.sh`):
the app in `~/Applications` and the `daybook` command in `~/.local/bin` link to the release build, and a running
Daybook restarts on it. Debug builds are never installed; call them by path.

## Verify

| Command | Expect |
|---|---|
| `bun run test` | Vitest and Rust unit tests pass |
| `bun run lint` | exits 0: types drift check, `tsc`, ESLint, `clippy -D warnings` |
| `cargo fmt --manifest-path src-tauri/Cargo.toml --check` | no differences |
| `bun run release` | `Daybook.app` and a `.dmg` in `src-tauri/target/release/bundle/` |

Try the app: `scripts/dev/launch.sh` starts the debug build next to the terminal without taking focus. Its
settings live in `~/Library/Application Support/com.philippstracker.daybook.debug/`, apart from the release
build's `com.philippstracker.daybook/`, so the release app keeps running and its data stays untouched.
[docs/testing.md](docs/testing.md) covers the suites and the scripts that drive the running app.

## Adapt

[docs/adapting.md](docs/adapting.md) lists every place that is specific to macOS or to the name, and the
Identity table for renaming the app (bundle id, binary, URL scheme, socket, paths). When done, run the Verify
table on the target platform and update the docs pages your change affects.
