//! `act`: run a command, reveal, click, type into, scroll or wait for an anchor, or press keys.
//! Parameters are checked (400) before any page is asked.

use serde::Serialize;
use serde_json::{Map, Value};

use super::catalog::CommandStatus;
use super::hub::{Hub, StageHost};
use super::keys::{StageKeys, command_for_keys, hotkey_for, parse_keys};
use super::page::{ActReply, ActRequest};
use super::protocol::{PageRect, ScreenRect, StageResult, required, to_screen_rect, trimmed};
use super::requests::{
    ActKind, Scroll, page_param, parse_scroll, parse_type_mode, parse_wait_timeout,
};
use super::verbs::object;
use super::windows::StagePage;
use crate::automation::{AutomationError, Params};
use crate::menu::command_shortcuts::COMMAND_SHORTCUTS;
use crate::shortcuts::HOTKEY_ACTIONS;

/// What `act` answers, by kind.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(untagged)]
pub enum ActResult {
    Anchor {
        anchor: String,
        window: StagePage,
        rect: Option<ScreenRect>,
        /// Only `reveal` and `wait` report it.
        #[serde(skip_serializing_if = "Option::is_none")]
        visible: Option<Option<ScreenRect>>,
    },
    Command {
        command: String,
    },
    /// Keys that reached the focused element of this window.
    Keys {
        window: StagePage,
    },
}

fn failed(reply: &ActReply) -> AutomationError {
    AutomationError::new(
        reply
            .status
            .filter(|s| (400..=599).contains(s))
            .unwrap_or(500),
        reply
            .error
            .clone()
            .filter(|e| !e.is_empty())
            .unwrap_or_else(|| "The page couldn't do that.".into()),
    )
}

impl<H: StageHost> Hub<H> {
    pub(super) async fn act(&self, params: &Params) -> StageResult<Map<String, Value>> {
        let kind: ActKind = params
            .get("kind")
            .map(String::as_str)
            .unwrap_or_default()
            .parse()?;
        let result = match kind {
            ActKind::Command => self.run_for_stage(required(params, "command")?).await?,
            ActKind::Wait => self.wait(params).await?,
            ActKind::Keys => self.keys(params).await?,
            ActKind::Reveal | ActKind::Click | ActKind::Type | ActKind::Scroll => {
                self.on_anchor(kind, params).await?
            }
        };
        object(result)
    }

    /// Hidden commands and aliases run too.
    async fn run_for_stage(&self, command: &str) -> StageResult<ActResult> {
        match self.host.command_status(command) {
            CommandStatus::Unknown => {
                return Err(AutomationError::new(
                    404,
                    format!("Unknown command “{command}”; describe lists them."),
                ));
            }
            CommandStatus::NeedsVault => {
                return Err(AutomationError::new(
                    409,
                    format!("“{command}” needs a vault: choose one in Settings first."),
                ));
            }
            CommandStatus::Disabled => {
                return Err(AutomationError::new(
                    409,
                    format!("“{command}” isn't available right now."),
                ));
            }
            CommandStatus::Ready => {}
        }
        self.host.run_command(command).await?;
        tokio::time::sleep(self.timeouts.after_command).await;
        Ok(ActResult::Command {
            command: command.into(),
        })
    }

    /// A rectangle a page measured, in screen points; None when its window is gone.
    fn on_screen(&self, page: StagePage, rect: Option<PageRect>) -> Option<ScreenRect> {
        Some(to_screen_rect(rect?, self.host.frame(page)?.content))
    }

    async fn on_anchor(&self, kind: ActKind, params: &Params) -> StageResult<ActResult> {
        let anchor = required(params, "anchor")?.to_string();
        let act = anchor_request(kind, &anchor, params)?;
        let reply = self
            .dispatch(act, &self.shown_pages())
            .await
            .ok_or_else(|| {
                AutomationError::new(
                    404,
                    format!(
                        "No window on screen shows “{anchor}”; describe lists the anchors shown now."
                    ),
                )
            })?;
        if !reply.ok {
            return Err(failed(&reply));
        }
        Ok(ActResult::Anchor {
            window: reply.page,
            rect: self.on_screen(reply.page, reply.rect),
            visible: (kind == ActKind::Reveal).then(|| self.on_screen(reply.page, reply.visible)),
            anchor,
        })
    }

    /// Collects until the anchor is on screen or the time is up.
    async fn wait(&self, params: &Params) -> StageResult<ActResult> {
        let anchor = required(params, "anchor")?;
        let timeout = parse_wait_timeout(params.get("timeout").map(String::as_str))?;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(timeout);
        loop {
            if let Some(found) = self.collect().await.into_iter().find(|a| a.id == anchor) {
                return Ok(ActResult::Anchor {
                    anchor: found.id,
                    window: found.window,
                    rect: Some(found.rect),
                    visible: Some(found.visible),
                });
            }
            let now = tokio::time::Instant::now();
            if now >= deadline {
                return Err(AutomationError::new(
                    404,
                    format!("“{anchor}” didn't appear within {timeout} ms."),
                ));
            }
            tokio::time::sleep(self.timeouts.wait_poll.min(deadline - now)).await;
        }
    }

    /// A global shortcut or a command key runs its command (a synthetic key press reaches neither);
    /// other keys go to the focused element of `window`, or of the focused window.
    async fn keys(&self, params: &Params) -> StageResult<ActResult> {
        let keys = parse_keys(required(params, "keys")?).ok_or_else(|| {
            AutomationError::bad("“keys” isn't a key combination (e.g. ⌘F2, cmd+shift+k, Enter).")
        })?;
        let target = trimmed(params, "window")
            .map(|_| page_param(params))
            .transpose()?;
        if let Some(target) = target {
            self.focus(target)?;
            tokio::time::sleep(self.timeouts.settle).await;
        }
        if let Some(ran) = self.run_shortcut(&keys).await {
            return ran;
        }
        let pages = self.keys_pages(target);
        let act = ActRequest::Keys { keys, page: target };
        let reply = self.dispatch(act, &pages).await.ok_or_else(|| {
            AutomationError::new(
                409,
                "No Daybook window has keyboard focus; give “window” or open an overlay first.",
            )
        })?;
        if !reply.ok {
            return Err(failed(&reply));
        }
        Ok(ActResult::Keys { window: reply.page })
    }

    /// A global shortcut runs its command (even a disabled one answers); a command key only when its
    /// command can run, else the keys go on to a page.
    async fn run_shortcut(&self, keys: &StageKeys) -> Option<StageResult<ActResult>> {
        if let Some(action) = hotkey_for(keys, &self.host.hotkeys())
            && let Some(hotkey) = HOTKEY_ACTIONS.iter().find(|h| h.action == action)
        {
            return Some(self.run_for_stage(hotkey.command).await);
        }
        let command = command_for_keys(keys, &COMMAND_SHORTCUTS)
            .filter(|command| self.host.command_status(command) == CommandStatus::Ready)?;
        Some(self.run_for_stage(command).await)
    }

    /// `target`, or else the shown windows that have focus.
    fn keys_pages(&self, target: Option<StagePage>) -> Vec<StagePage> {
        match target {
            Some(target) => vec![target],
            None => self
                .shown_pages()
                .into_iter()
                .filter(|page| self.host.frame(*page).is_some_and(|f| f.focused))
                .collect(),
        }
    }
}

/// The page's request for an anchor act, with its parameters checked.
fn anchor_request(kind: ActKind, anchor: &str, params: &Params) -> StageResult<ActRequest> {
    let anchor = anchor.to_string();
    Ok(match kind {
        ActKind::Reveal => ActRequest::Reveal { anchor },
        ActKind::Click => ActRequest::Click { anchor },
        ActKind::Type => ActRequest::Type {
            anchor,
            text: params.get("text").cloned().unwrap_or_default(),
            mode: parse_type_mode(params.get("mode").map(String::as_str))?,
        },
        ActKind::Scroll => {
            let (by, to) = match parse_scroll(params)? {
                Scroll::By(by) => (Some(by), None),
                Scroll::To(to) => (None, Some(to)),
            };
            ActRequest::Scroll { anchor, by, to }
        }
        ActKind::Command | ActKind::Wait | ActKind::Keys => {
            return Err(AutomationError::new(500, "That act needs no anchor."));
        }
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use super::*;
    use crate::stage::test_host::{
        Call, FakeHost, anchor, declined, handled, hub, page_rect, params, rect,
    };
    use crate::store::settings::Hotkeys;

    async fn act(hub: &Hub<FakeHost>, items: &[(&str, &str)]) -> StageResult<Map<String, Value>> {
        hub.run("act", &params(items)).await
    }

    async fn act_ok(hub: &Hub<FakeHost>, items: &[(&str, &str)]) -> Value {
        Value::Object(act(hub, items).await.expect("the act succeeds"))
    }

    async fn act_error(hub: &Hub<FakeHost>, items: &[(&str, &str)]) -> AutomationError {
        act(hub, items).await.expect_err("the act fails")
    }

    fn not_shown(anchor: &str) -> AutomationError {
        AutomationError::new(
            404,
            format!("No window on screen shows “{anchor}”; describe lists the anchors shown now."),
        )
    }

    fn measured() -> ActReply {
        ActReply {
            rect: Some(page_rect(10.0, 20.0, 100.0, 30.0)),
            visible: Some(page_rect(10.0, 20.0, 100.0, 15.0)),
            ..handled()
        }
    }

    /// The Daybook window with a page that handled the act, and a focus.
    fn hub_with_hub_page() -> std::sync::Arc<Hub<FakeHost>> {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host.set_focused(StagePage::Hub);
        hub.host.page_answers(StagePage::Hub, handled());
        hub
    }

    #[tokio::test]
    async fn a_missing_or_unknown_kind_is_400() {
        let hub = hub();
        let cases: [Vec<(&str, &str)>; 2] = [vec![], vec![("kind", "dance")]];
        for items in cases {
            let err = act_error(&hub, &items).await;

            assert_eq!(
                err,
                AutomationError::new(
                    400,
                    "“kind” must be command, reveal, click, type, keys, scroll or wait."
                ),
                "{items:?}"
            );
        }
    }

    #[tokio::test]
    async fn bad_parameters_are_400_before_any_page_is_asked() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        let cases: [(&[(&str, &str)], &str); 10] = [
            (&[("kind", "reveal")], "“anchor” is missing."),
            (&[("kind", "click")], "“anchor” is missing."),
            (
                &[("kind", "type"), ("anchor", "f"), ("mode", "insert")],
                "“mode” must be replace or append.",
            ),
            (
                &[("kind", "scroll"), ("anchor", "a")],
                "Give “by” (points, positive downwards) or “to” (top or bottom).",
            ),
            (
                &[
                    ("kind", "scroll"),
                    ("anchor", "a"),
                    ("by", "10"),
                    ("to", "top"),
                ],
                "Give “by” or “to”, not both.",
            ),
            (
                &[("kind", "scroll"), ("anchor", "a"), ("to", "middle")],
                "“to” must be top or bottom.",
            ),
            (&[("kind", "command")], "“command” is missing."),
            (&[("kind", "keys")], "“keys” is missing."),
            (&[("kind", "wait")], "“anchor” is missing."),
            (
                &[("kind", "wait"), ("anchor", "a"), ("timeout", "soon")],
                "“timeout” must be a positive whole number of milliseconds.",
            ),
        ];
        for (items, message) in cases {
            let err = act_error(&hub, items).await;

            assert_eq!(err, AutomationError::new(400, message), "{items:?}");
        }
        assert!(hub.host.sent().is_empty());
    }

    #[tokio::test]
    async fn an_anchor_no_page_handled_is_404() {
        let kinds: [(&str, &[(&str, &str)]); 4] = [
            ("reveal", &[]),
            ("click", &[]),
            ("type", &[("text", "x")]),
            ("scroll", &[("by", "10")]),
        ];
        for scenario in ["nothing on screen", "declined", "silent"] {
            let hub = hub();
            match scenario {
                "declined" => {
                    hub.host.open(StagePage::Hub);
                    hub.host.page_answers(StagePage::Hub, declined());
                }
                "silent" => hub.host.open(StagePage::Hub),
                _ => {}
            }
            for (kind, extra) in kinds {
                let mut items = vec![("kind", kind), ("anchor", "nope")];
                items.extend_from_slice(extra);

                let err = act_error(&hub, &items).await;

                assert_eq!(err, not_shown("nope"), "{scenario}: {kind}");
            }
        }
    }

    #[tokio::test]
    async fn a_failed_reply_keeps_error_statuses_and_turns_other_codes_into_500() {
        let cases = [
            (Some(409), 409),
            (Some(400), 400),
            (Some(599), 599),
            (Some(399), 500),
            (Some(600), 500),
            (Some(200), 500),
            (None, 500),
        ];
        for (status, expected) in cases {
            let hub = hub();
            hub.host.open(StagePage::Hub);
            hub.host.page_answers(
                StagePage::Hub,
                ActReply {
                    ok: false,
                    error: Some("“a” is disabled".into()),
                    status,
                    ..handled()
                },
            );

            let err = act_error(&hub, &[("kind", "click"), ("anchor", "a")]).await;

            assert_eq!(
                err,
                AutomationError::new(expected, "“a” is disabled"),
                "{status:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_failed_reply_without_a_message_gets_a_default_one() {
        for error in [None, Some(String::new())] {
            let hub = hub();
            hub.host.open(StagePage::Hub);
            hub.host.page_answers(
                StagePage::Hub,
                ActReply {
                    ok: false,
                    error: error.clone(),
                    ..handled()
                },
            );

            let err = act_error(&hub, &[("kind", "click"), ("anchor", "a")]).await;

            assert_eq!(
                err,
                AutomationError::new(500, "The page couldn't do that."),
                "{error:?}"
            );
        }
    }

    #[tokio::test]
    async fn reveal_answers_where_the_anchor_is_and_what_shows_of_it() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host.page_answers(StagePage::Hub, measured());

        let out = act_ok(&hub, &[("kind", "reveal"), ("anchor", "a")]).await;

        assert_eq!(
            out,
            json!({
                "anchor": "a",
                "window": "hub",
                "rect": {"x": 110, "y": 100, "width": 100, "height": 30},
                "visible": {"x": 110, "y": 100, "width": 100, "height": 15}
            })
        );
    }

    #[tokio::test]
    async fn reveal_answers_null_when_nothing_of_the_anchor_shows() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host.page_answers(
            StagePage::Hub,
            ActReply {
                visible: None,
                ..measured()
            },
        );

        let out = act_ok(&hub, &[("kind", "reveal"), ("anchor", "a")]).await;

        assert_eq!(out["visible"], Value::Null);
    }

    #[tokio::test]
    async fn click_type_and_scroll_answer_the_rect_without_visible() {
        let cases: [(&str, &[(&str, &str)]); 3] = [
            ("click", &[]),
            ("type", &[("text", "x")]),
            ("scroll", &[("to", "top")]),
        ];
        for (kind, extra) in cases {
            let hub = hub();
            hub.host.open(StagePage::Hub);
            hub.host.page_answers(StagePage::Hub, measured());
            let mut items = vec![("kind", kind), ("anchor", "a")];
            items.extend_from_slice(extra);

            let out = act_ok(&hub, &items).await;

            assert_eq!(
                out,
                json!({
                    "anchor": "a",
                    "window": "hub",
                    "rect": {"x": 110, "y": 100, "width": 100, "height": 30}
                }),
                "{kind}"
            );
        }
    }

    #[tokio::test]
    async fn the_rect_is_measured_from_the_content_of_the_page_that_handled_it() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host
            .open_at(StagePage::Settings, rect(200, 300, 500, 400));
        hub.host.page_answers(StagePage::Hub, declined());
        hub.host.page_answers(
            StagePage::Settings,
            ActReply {
                rect: Some(page_rect(1.0, 2.0, 30.0, 40.0)),
                ..handled()
            },
        );

        let out = act_ok(&hub, &[("kind", "click"), ("anchor", "a")]).await;

        assert_eq!(out["window"], "settings");
        assert_eq!(
            out["rect"],
            json!({"x": 201, "y": 302, "width": 30, "height": 40})
        );
    }

    #[tokio::test]
    async fn the_first_page_that_handled_the_act_wins() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host.open(StagePage::Settings);
        hub.host.page_answers(StagePage::Hub, handled());
        hub.host.page_answers(StagePage::Settings, handled());

        let out = act_ok(&hub, &[("kind", "click"), ("anchor", "a")]).await;

        assert_eq!(out["window"], "hub");
    }

    #[tokio::test]
    async fn a_reply_from_a_page_that_was_not_asked_does_not_count() {
        let hub = hub_with_hub_page();
        hub.host.page_answers_unasked(
            StagePage::Rail,
            ActReply {
                ok: false,
                error: Some("not me".into()),
                ..handled()
            },
        );

        let out = act_ok(&hub, &[("kind", "click"), ("anchor", "a")]).await;

        assert_eq!(out["window"], "hub");
    }

    #[tokio::test]
    async fn acts_go_only_to_pages_on_screen() {
        let hub = hub_with_hub_page();
        hub.host.hide_window(StagePage::Settings);
        hub.host.page_answers(StagePage::Settings, handled());

        let out = act_ok(&hub, &[("kind", "click"), ("anchor", "a")]).await;

        assert_eq!(out["window"], "hub");
        let asked: Vec<_> = hub.host.acts_sent().into_iter().map(|s| s.pages).collect();
        assert_eq!(asked, [[StagePage::Hub]]);
    }

    #[tokio::test]
    async fn pages_are_sent_the_act_in_the_events_shape() {
        let cases: [(&[(&str, &str)], Value); 6] = [
            (
                &[("kind", "reveal"), ("anchor", "f")],
                json!({"kind": "reveal", "anchor": "f"}),
            ),
            (
                &[("kind", "click"), ("anchor", "f")],
                json!({"kind": "click", "anchor": "f"}),
            ),
            (
                &[
                    ("kind", "type"),
                    ("anchor", "f"),
                    ("text", "hi"),
                    ("mode", "append"),
                ],
                json!({"kind": "type", "anchor": "f", "text": "hi", "mode": "append"}),
            ),
            (
                &[("kind", "type"), ("anchor", "f")],
                json!({"kind": "type", "anchor": "f", "text": "", "mode": "replace"}),
            ),
            (
                &[("kind", "scroll"), ("anchor", "f"), ("by", "-240")],
                json!({"kind": "scroll", "anchor": "f", "by": -240.0}),
            ),
            (
                &[("kind", "scroll"), ("anchor", "f"), ("to", "bottom")],
                json!({"kind": "scroll", "anchor": "f", "to": "bottom"}),
            ),
        ];
        for (items, expected) in cases {
            let hub = hub_with_hub_page();

            act_ok(&hub, items).await;

            assert_eq!(hub.host.last_act(), expected, "{items:?}");
        }
    }

    #[tokio::test]
    async fn a_command_runs_and_is_named_in_the_answer() {
        let hub = hub();
        hub.host.set_status("journal:open", CommandStatus::Ready);

        let out = act_ok(&hub, &[("kind", "command"), ("command", "journal:open")]).await;

        assert_eq!(out, json!({"command": "journal:open"}));
        assert_eq!(hub.host.ran(), ["journal:open"]);
    }

    #[tokio::test]
    async fn a_command_that_cannot_run_is_refused() {
        let cases = [
            (
                CommandStatus::Unknown,
                AutomationError::new(404, "Unknown command “c:x”; describe lists them."),
            ),
            (
                CommandStatus::NeedsVault,
                AutomationError::new(409, "“c:x” needs a vault: choose one in Settings first."),
            ),
            (
                CommandStatus::Disabled,
                AutomationError::new(409, "“c:x” isn't available right now."),
            ),
        ];
        for (status, expected) in cases {
            let hub = hub();
            hub.host.set_status("c:x", status);

            let err = act_error(&hub, &[("kind", "command"), ("command", "c:x")]).await;

            assert_eq!(err, expected, "{status:?}");
            assert!(hub.host.ran().is_empty(), "{status:?}");
        }
    }

    #[tokio::test]
    async fn wait_answers_when_the_anchor_is_on_screen() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host.page_shows(
            StagePage::Hub,
            vec![anchor("other", 0.0, 0.0), anchor("a", 5.0, 6.0)],
        );

        let out = act_ok(&hub, &[("kind", "wait"), ("anchor", "a")]).await;

        assert_eq!(
            out,
            json!({
                "anchor": "a",
                "window": "hub",
                "rect": {"x": 105, "y": 86, "width": 100, "height": 20},
                "visible": null
            })
        );
    }

    #[tokio::test]
    async fn wait_keeps_looking_until_the_anchor_appears() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host.page_shows(StagePage::Hub, vec![]);
        let later = hub.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(20)).await;
            later
                .host
                .page_shows(StagePage::Hub, vec![anchor("late", 0.0, 0.0)]);
        });

        let out = act_ok(&hub, &[("kind", "wait"), ("anchor", "late")]).await;

        assert_eq!(out["anchor"], "late");
    }

    #[tokio::test]
    async fn wait_times_out_with_404() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host.page_shows(StagePage::Hub, vec![]);

        let err = act_error(
            &hub,
            &[("kind", "wait"), ("anchor", "a"), ("timeout", "60")],
        )
        .await;

        assert_eq!(
            err,
            AutomationError::new(404, "“a” didn't appear within 60 ms.")
        );
    }

    #[tokio::test]
    async fn wait_does_not_see_the_anchors_of_a_hidden_window() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host.page_shows(StagePage::Hub, vec![]);
        hub.host.hide_window(StagePage::Settings);
        hub.host
            .page_shows(StagePage::Settings, vec![anchor("a", 0.0, 0.0)]);

        let err = act_error(
            &hub,
            &[("kind", "wait"), ("anchor", "a"), ("timeout", "30")],
        )
        .await;

        assert_eq!(
            err,
            AutomationError::new(404, "“a” didn't appear within 30 ms.")
        );
    }

    fn with_hotkeys(hub: &Hub<FakeHost>, hotkeys: Hotkeys) {
        hub.host.set_hotkeys(hotkeys);
    }

    #[tokio::test]
    async fn a_global_shortcut_runs_its_command() {
        let hub = hub_with_hub_page();
        with_hotkeys(
            &hub,
            Hotkeys {
                journal: "ctrl+alt+cmd+j".into(),
                ..Hotkeys::none()
            },
        );
        hub.host.set_status("journal:open", CommandStatus::Ready);

        let out = act_ok(&hub, &[("kind", "keys"), ("keys", "⌃⌥⌘J")]).await;

        assert_eq!(out, json!({"command": "journal:open"}));
        assert_eq!(hub.host.ran(), ["journal:open"]);
        assert!(hub.host.acts_sent().is_empty());
    }

    #[tokio::test]
    async fn a_global_shortcut_beats_a_command_key() {
        let hub = hub_with_hub_page();
        with_hotkeys(
            &hub,
            Hotkeys {
                search: "cmd+k".into(),
                ..Hotkeys::none()
            },
        );
        hub.host.set_status("search:open", CommandStatus::Ready);
        hub.host.set_status("palette:open", CommandStatus::Ready);

        let out = act_ok(&hub, &[("kind", "keys"), ("keys", "cmd+k")]).await;

        assert_eq!(out, json!({"command": "search:open"}));
    }

    #[tokio::test]
    async fn a_global_shortcut_whose_command_is_disabled_is_409() {
        let hub = hub_with_hub_page();
        with_hotkeys(
            &hub,
            Hotkeys {
                journal: "ctrl+alt+cmd+j".into(),
                ..Hotkeys::none()
            },
        );
        hub.host.set_status("journal:open", CommandStatus::Disabled);

        let err = act_error(&hub, &[("kind", "keys"), ("keys", "ctrl+alt+cmd+j")]).await;

        assert_eq!(
            err,
            AutomationError::new(409, "“journal:open” isn't available right now.")
        );
        assert!(hub.host.acts_sent().is_empty());
    }

    #[tokio::test]
    async fn a_command_key_runs_its_command_when_it_is_ready() {
        let hub = hub_with_hub_page();
        hub.host.set_status("palette:open", CommandStatus::Ready);

        let out = act_ok(&hub, &[("kind", "keys"), ("keys", "cmd+k")]).await;

        assert_eq!(out, json!({"command": "palette:open"}));
        assert_eq!(hub.host.ran(), ["palette:open"]);
    }

    #[tokio::test]
    async fn a_command_key_that_is_not_ready_goes_to_the_focused_page() {
        let hub = hub_with_hub_page();
        hub.host.set_status("palette:open", CommandStatus::Disabled);

        let out = act_ok(&hub, &[("kind", "keys"), ("keys", "cmd+k")]).await;

        assert_eq!(out, json!({"window": "hub"}));
        assert!(hub.host.ran().is_empty());
        assert_eq!(
            hub.host.last_act(),
            json!({
                "kind": "keys",
                "keys": {
                    "key": "k", "code": "KeyK",
                    "meta": true, "shift": false, "alt": false, "ctrl": false
                }
            })
        );
    }

    #[tokio::test]
    async fn other_keys_go_only_to_the_page_whose_window_has_focus() {
        let hub = hub_with_hub_page();
        hub.host.open(StagePage::Settings);
        hub.host.page_answers(StagePage::Settings, handled());

        let out = act_ok(&hub, &[("kind", "keys"), ("keys", "Enter")]).await;

        assert_eq!(out, json!({"window": "hub"}));
        let asked: Vec<_> = hub.host.acts_sent().into_iter().map(|s| s.pages).collect();
        assert_eq!(asked, [[StagePage::Hub]]);
    }

    #[tokio::test]
    async fn keys_with_a_window_focus_it_and_go_to_it() {
        let hub = hub_with_hub_page();
        hub.host.open(StagePage::Settings);
        hub.host.page_answers(StagePage::Settings, handled());

        let out = act_ok(
            &hub,
            &[("kind", "keys"), ("keys", "Enter"), ("window", "settings")],
        )
        .await;

        assert_eq!(out, json!({"window": "settings"}));
        assert_eq!(hub.host.calls(), [Call::Focus(StagePage::Settings)]);
        assert_eq!(
            hub.host.last_act(),
            json!({
                "kind": "keys",
                "keys": {
                    "key": "Enter", "code": "Enter",
                    "meta": false, "shift": false, "alt": false, "ctrl": false
                },
                "page": "settings"
            })
        );
    }

    #[tokio::test]
    async fn keys_with_no_focused_window_are_409() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host.page_answers(StagePage::Hub, handled());
        let expected = AutomationError::new(
            409,
            "No Daybook window has keyboard focus; give “window” or open an overlay first.",
        );

        let unfocused = act_error(&hub, &[("kind", "keys"), ("keys", "Enter")]).await;
        let declined_by_page = {
            let hub = hub_with_hub_page();
            hub.host.page_answers(StagePage::Hub, declined());
            act_error(&hub, &[("kind", "keys"), ("keys", "Enter")]).await
        };

        assert_eq!(unfocused, expected);
        assert_eq!(declined_by_page, expected);
    }

    #[tokio::test]
    async fn keys_for_a_window_that_cannot_take_them_are_refused_before_they_are_sent() {
        let hub = hub();
        hub.host.hide_window(StagePage::Hub);
        hub.host.open(StagePage::Rail);
        let cases = [
            (
                "hub",
                AutomationError::new(409, "The hub window isn't open; show it first."),
            ),
            (
                "rail",
                AutomationError::new(409, "The rail never takes focus."),
            ),
            (
                "attic",
                AutomationError::new(
                    404,
                    "“window” must be one of hub, settings, panel, rail, quick-notes.",
                ),
            ),
        ];
        for (window, expected) in cases {
            let err = act_error(
                &hub,
                &[("kind", "keys"), ("keys", "Enter"), ("window", window)],
            )
            .await;

            assert_eq!(err, expected, "{window}");
        }
        assert!(hub.host.acts_sent().is_empty());
    }

    #[tokio::test]
    async fn keys_that_are_not_a_combination_are_400() {
        let hub = hub_with_hub_page();

        let err = act_error(&hub, &[("kind", "keys"), ("keys", "nonsense")]).await;

        assert_eq!(
            err,
            AutomationError::new(
                400,
                "“keys” isn't a key combination (e.g. ⌘F2, cmd+shift+k, Enter)."
            )
        );
    }
}
