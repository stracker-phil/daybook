# Automation: the CLI, links and the HTTP API

Other apps and scripts drive Daybook three ways, and all three run the same actions (`automation::run`) or a
command by its id. Errors never open a window: the CLI prints them, a link shows a notification, the HTTP API
answers with a status code.

## Actions

`automation/request.rs` (pure) reads an action and its parameters; `automation/mod.rs` runs it and adds the
current `state` to every answer.

| Action | Parameters | Does | Answer |
|---|---|---|---|
| `state` | – | – | the state below |
| `show` | – | shows the Daybook window without taking focus | `{}` |
| `status` | `status` (working, break, off), `note` | sets the status like the overlay, without opening it | `{ result: "set" \| "unchanged" }` |
| `slack` | `status` and `note`, or `emoji`, `text`, `presence` (auto, away) | sets only the Slack status | `{}` |
| `command` | `id` | runs a bus command by id ([commands.md](commands.md)) | `{}` |

Errors: an unknown action is 404 (`Unknown action “x”. Actions: state, show, status, slack, command.`), an action
that needs a vault without one is 409 (`Choose your vault in the app’s Settings first.`; a command's own
`needs_vault` is checked here so Settings never opens), a bad parameter or a disabled command is 400, anything
that fails while running is 422.

`automation/state.rs`: `AutomationState` (camelCase) is `status`, `since` (`HH:MM`), `context`, `contextTitle`,
`call { app, title, since }`, `event { title, start, end }` (the one running now), `music { player, track }`
(`null` while Music is closed) and `waiting` (note changes waiting for today's note).

## The CLI

The running app parses the words (`cli/grammar.rs`, pure, a port of the prototype's grammar), because it knows
every command and each family's choices (`automation/cli.rs` `catalog`):
- Own words: `status <status> [note]`, `slack …`, `state`, `show`, `help [group]`, `commands`, and alone
  `focus`, `hide`, `quit`, `status` (the app's own report).
- Any registered command by the words of its id (`journal open` → `journal:open`); a family's argument matches a
  choice's word or title (`music play Focus` → `music:play:<id>`).
- `--help` anywhere prints the help for what was typed and runs nothing; `help --json` and `commands --json`
  print `[{ id, help }]`.
`cli/help.rs` builds the help text and the command list. See [backend.md](backend.md#cli-and-single-instance-handoff)
for the socket and the exit codes.

## Links

`daybook://<action>?<params>` runs the same actions: `daybook://show`, `daybook://status?status=break&note=Lunch`,
`daybook://command?id=journal:open`. Debug builds answer `daybook-debug://` instead. A link that starts Daybook
waits until the page has loaded. A failure shows a notification.

## The HTTP API

`http_api/` serves `127.0.0.1:<port>` (`settings.api.port`, default 27180) while `settings.api.enabled` is on
([ADR 0021](adr/0021-a-local-http-api-with-guards.md)): `GET` or `POST /v1/<action>`, parameters from the query,
a JSON object or a form body. `http_api/request.rs` (pure) checks, in order:
- the token: `Authorization: Bearer …`, `X-Daybook-Token`, or the prototype's `X-Companion-Token` and `?token=`
  (401 otherwise);
- no `Origin` header and a `Host` of `127.0.0.1` or `localhost` with the port (403 otherwise, against browsers
  and DNS rebinding);
- the method (405) and the path (404);
- the body: at most 16 KiB (413), never chunked (411), valid JSON or form (400).
Answers are `{ "ok": true, …, "state": … }` or `{ "ok": false, "error": "…" }` with the status code.

The token lives in the Keychain (`http_api/token.rs`, account `api-token`, 64 hex digits); a token imported
from the prototype into `settings.json` moves there on the first start and leaves the file. Settings →
Automation shows the server's state (a taken port is reported), the port, the token with Copy and New Token,
and curl examples. Saving `api` restarts the server.
