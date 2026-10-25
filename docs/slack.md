# Slack

Daybook sets the owner's Slack profile status and presence from the status, calls and calendar absences, and
posts the channel message they confirmed in the status overlay. It works only in contexts with the Slack
feature, once a token is stored. The owner never sets the status by hand in Slack, so Daybook never protects
a manual one.

| File | Holds |
|---|---|
| `net.rs` | `Http`: form posts to `net::HOSTS` only ([ADR 0020](adr/0020-network-only-to-listed-hosts.md)) |
| `secrets.rs` | `Secret`, `SecretStore`, `Keychain` |
| `slack/api.rs` | `SlackApi`: `auth.test`, `users.profile.set`, `users.setPresence`, `chat.postMessage`; `apply` sets a status; `is_token` |
| `slack/token.rs` | `Tokens`: the token in the Keychain (service = the identifier, account `slack-token`), read once |
| `slack/decide.rs` | pure: `decide` (call, then heads-up, then status), `with_project`, `status_text`, `describe_status`, `fill_template` |
| `slack/status.rs` | pure: `Live`, the state between changes; each change returns the `Step` to take |
| `slack/sender.rs` | `Sender`: one retry after 30 s, the newest status wins |
| `slack/shown.rs` | `ShownStatus` (`slack-shown.json`): what Slack shows, as Daybook last set it |
| `slack/channel.rs` | pure: `status_message`, `end_of_day_message`, `LastMessage` |
| `slack/day_message.rs` | `DayMessage` (`day-message.json`): the day's first Working message |
| `slack/commands.rs` | Settings → Slack: `slack_info`, `slack_connect`, `slack_test`, `slack_disconnect` |
| `slack/mod.rs` | `Slack` (managed state), `SlackFiles` (in `AppState`), the listeners, `shown_now` |
| `status_rules/rules.rs` | pure: `is_absence`, `describe_when`, `pick_heads_up` |
| `status_rules/mod.rs` | the minute check and its triggers |

## What Slack shows

`decide` ranks three sources:

1. **A call** (step 10's `calls::subscribe`): `Started` takes the Slack status of the app trigger with the
   call's app name (Settings → Calls); `Ended` gives it up. A new title for the same call changes nothing.
2. **A heads-up** before an absence (below). Its presence "No change" follows the picked status's presence
   (active while Working, away on a break), or stays unchanged when that status's workflow is off.
3. **The picked status** (`status_changed`): `settings.slack.statuses[status]` with `{note}` filled by the
   confirmed message. While Working the text gets the project (`9:00 - 18:00 | Oscorp`, `Focus: Oscorp`
   without text, Internal left out); the project comes with step 24, until then there is none. Before the
   day's first status, Off counts. A status whose workflow is off (`slack.manualWorkflows`) leaves Slack alone.

`Live` keeps what was picked, the call and the heads-up between changes:

- A call's status counts as shown only once Slack took it (`call_sent`). While it shows, a picked status
  waits and comes back when the call ends; an ended call that Slack shows is replaced in every context.
- A call that starts outside a Slack context isn't sent; switching to a context with Slack sends it.
- A heads-up is sent only when it changes, and not over a shown call.

`push` sends in the background. A status that changes nothing (keep status, presence "No change") isn't sent.
The sender retries a failure once after 30 s and notifies ("Slack status not updated"); a newer status
replaces an older one and its retry. A token without `users:write` still sets the status; presence is left.
What Slack took is recorded in `slack-shown.json` and announced as `slack_changed`.

## Channel messages

A status change posts `status_message`: the message confirmed in the overlay, trimmed; nothing when it is
empty or the status's workflow is off. Going Off through the end-of-day check wraps it in
`endOfDay.done` / `endOfDay.incomplete` (`Tracked: Done | _off for today_`); without a message the part around
`{message}` goes and the fixed template alone is posted. The same message twice in a row (`LastMessage`,
case-insensitive) isn't posted. It goes to `settings.slack.channel` (empty: none) and needs `chat:write`.
A failure notifies ("Slack message not sent").

The day's first Working message is remembered in `day-message.json` (channel id, timestamp, its text) for
the focus list and the end-of-day reply of step 12.

## Absence heads-ups

`status_rules::refresh` runs every minute and on `calendar_changed`, `context_changed` and `settings_changed`,
one at a time. In a context with Slack and with enabled rules, it reads the current context's events from
today through the longest rule (`calendar::events_ahead`, cached five minutes, all-day events included) and
picks:

- **Absences**: events titled like `statusRules.absenceTitles` (`Out of office`, from the HR calendar), or busy
  events of at least `minBusyHours`. Canceled and declined ones don't count.
- **The rule**: for the next absence that hasn't started, the enabled rule with the fewest hours that reaches
  its start (`Absence within 8 h`). Its text fills `{when}` (`from 2pm to 7pm CEST`, `on 2026-10-05`,
  `from 2026-10-05 to 2026-10-09`), `{start}`, `{end}`, `{startDate}`, `{endDate}` and `{tz}`; an unknown
  placeholder stays. `{tz}` is `statusRules.tzLabel`, else the Mac's zone abbreviation at the start.
- It is sent with the absence's start as Slack's expiry, so Slack clears it then and the status comes back.

## The sidebar

`HubView.slack` is `shown_now`: in a context with Slack and with a token, what `slack-shown.json` holds,
without emoji and text after their expiry. The sidebar's footer shows it (emoji, text, presence dot).

## Settings

- **Slack** (Setup): the user token (`xoxp-…`), checked with `auth.test` before it is stored; Test Connection;
  Remove Token. The token never goes back to the page.
- **Workflows**: the channel; a Slack status per status; "Before an absence" (absence titles, busy blocks,
  the time-zone label) and one card per rule.
- **Calls**: a Slack status per app trigger.

## Commands

| Command | Arguments | Returns |
|---|---|---|
| `slack_info` | – | `SlackInfo`: `connected`, the `identity` from the last check |
| `slack_connect` | `token` | `SlackIdentity` (team, user); `invalid_input` for a malformed token, `network` when Slack refuses it, `io` when the Keychain fails |
| `slack_test` | – | `SlackIdentity`; `not_found` without a token, `network` |
| `slack_disconnect` | – | –; `io` when the Keychain fails |

Not yet: the action log (step 28), `daybook slack …` (step 21), the focus list in the day's message (step 12).
