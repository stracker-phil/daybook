# 0021. A local HTTP API, off by default and guarded

## Status

Accepted (2026-10-27), implemented.

## Context

Scripts, Shortcuts, Stream Deck buttons and the Stage presenter drive the prototype over a small HTTP API on
localhost. TimeTracker, the blueprint, has only its CLI. A local port is reachable by every process on the Mac and,
through DNS rebinding, by web pages in a browser.

## Decision

- Daybook keeps the API: `127.0.0.1:<port>` (default 27180), `GET|POST /v1/<action>`, running the same actions as
  the CLI and `daybook://` links (`automation::run`). It is off unless the owner turns it on (or the imported
  prototype settings had it on).
- Every request needs the token (`Authorization: Bearer`, `X-Daybook-Token`; the prototype's
  `X-Companion-Token` and `?token=` keep old scripts working). The token lives in the Keychain, never in a file.
- Requests with an `Origin` header (a browser) and requests whose `Host` isn't `127.0.0.1` or `localhost` with
  the configured port (DNS rebinding) are refused. Bodies are at most 16 KB, JSON or form; chunked bodies are
  refused.
- No new crates: a plain HTTP/1.1 reader on `tokio::net::TcpListener`, as another app's Stage server.

## Consequences

- A taken port shows in Settings → Automation instead of failing silently.
- The Stage hook (step 22) rides on this server.
