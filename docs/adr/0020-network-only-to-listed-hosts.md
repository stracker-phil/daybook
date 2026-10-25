# 0020. Daybook talks to Slack (and later Apple's catalog), nothing else

## Status

Accepted (2026-10-25), implemented.

## Context

TimeTracker, the blueprint, never touches the network. Daybook sets the owner's Slack status and posts their
status messages (step 11), and later searches Apple Music's catalog (step 19). Both need HTTP from the app.
The token for Slack is a secret that must not end up in a file, a log or the page.

## Decision

- One module, `net.rs`, is the only way out: `Http` (`reqwest` with rustls, a 10 s timeout) posts only to the
  hosts in `net::HOSTS` (`slack.com`; step 19 adds Apple's), and only over HTTPS. Plain HTTP is allowed to
  `127.0.0.1` alone, for the fake servers in tests, and only when a test lists it.
- No telemetry, no update check, no other request.
- The webview stays IPC-only: the CSP's `connect-src` is unchanged, and the page never sees a token.
- Secrets live in the login Keychain (`secrets.rs`), as generic passwords under the app's identifier, so the
  debug build keeps its own. `Secret` wipes its memory and never prints its value.

## Consequences

- A new remote service means adding its host to `net::HOSTS` and a line to `docs/adapting.md`.
- Network failures cross IPC as the `network` error kind.
- An ad hoc signed debug build may ask for Keychain access again after a rebuild; the release build keeps it.
