//! Slack's Web API: the token check, the profile status, presence and channel messages.

use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, Ordering};

use regex::Regex;
use serde::Serialize;
use serde_json::{Value, json};
use ts_rs::TS;
use url::Url;

use super::decide::status_text;
use crate::net::{HOSTS, Http};
use crate::secrets::Secret;
use crate::store::settings::{Presence, SlackStatus};

const BASE: &str = "https://slack.com/api/";

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum SlackError {
    /// Slack answered `ok: false`.
    #[error("Slack {method} failed: {code}")]
    Api { method: String, code: String },
    #[error("Slack {method}: {reason}")]
    Net { method: String, reason: String },
}

/// The workspace and the user a token belongs to.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct SlackIdentity {
    pub team: String,
    pub user: String,
}

/// Where a posted message went, as Slack names it.
#[derive(Clone, Debug, PartialEq)]
pub struct Posted {
    pub channel: String,
    pub ts: String,
}

/// What Slack took of a status.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Applied {
    /// Emoji, text and the expiry in Unix seconds (0: none).
    pub profile: Option<(String, String, i64)>,
    pub presence: Option<Presence>,
    /// Why presence wasn't set.
    pub problem: Option<&'static str>,
}

pub const MISSING_PRESENCE_SCOPE: &str = "Presence not set: the token lacks users:write";

static TOKEN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^xox[a-z]-[A-Za-z0-9-]+$").unwrap());

/// A Slack token's shape (`xoxp-…`).
pub fn is_token(text: &str) -> bool {
    TOKEN.is_match(text)
}

pub struct SlackApi {
    http: Http,
    base: Url,
    presence_scope_warned: AtomicBool,
}

impl Default for SlackApi {
    fn default() -> Self {
        Self::with_base(Http::new(HOSTS), BASE)
    }
}

impl SlackApi {
    /// `base` ends with `/`; the method's name is appended.
    pub fn with_base(http: Http, base: &str) -> Self {
        Self {
            http,
            base: Url::parse(base).expect("a valid base URL"),
            presence_scope_warned: AtomicBool::new(false),
        }
    }

    async fn call(
        &self,
        method: &str,
        token: &Secret,
        params: &[(&str, &str)],
    ) -> Result<Value, SlackError> {
        let net = |reason: String| SlackError::Net {
            method: method.into(),
            reason,
        };
        let url = self.base.join(method).map_err(|e| net(e.to_string()))?;
        let body = self
            .http
            .post_form(&url, token.expose(), params)
            .await
            .map_err(|e| net(e.to_string()))?;
        let data: Value = serde_json::from_slice(&body).map_err(|e| net(e.to_string()))?;
        if data["ok"] != json!(true) {
            return Err(SlackError::Api {
                method: method.into(),
                code: data["error"].as_str().unwrap_or("unknown_error").into(),
            });
        }
        Ok(data)
    }

    pub async fn auth_test(&self, token: &Secret) -> Result<SlackIdentity, SlackError> {
        let data = self.call("auth.test", token, &[]).await?;
        let text = |key: &str| data[key].as_str().unwrap_or_default().to_string();
        Ok(SlackIdentity {
            team: text("team"),
            user: text("user"),
        })
    }

    pub async fn post_message(
        &self,
        token: &Secret,
        channel: &str,
        text: &str,
    ) -> Result<Posted, SlackError> {
        let data = self
            .call(
                "chat.postMessage",
                token,
                &[("channel", channel), ("text", text)],
            )
            .await?;
        Ok(Posted {
            channel: data["channel"].as_str().unwrap_or(channel).into(),
            ts: data["ts"].as_str().unwrap_or_default().into(),
        })
    }

    /// Sets the profile status (unless kept) and presence (unless kept). A token without users:write
    /// still sets the status; `Applied::problem` says so.
    pub async fn apply(
        &self,
        token: &Secret,
        mapping: &SlackStatus,
        note: &str,
        expiration: i64,
    ) -> Result<Applied, SlackError> {
        let mut applied = Applied::default();
        if !mapping.keep_status {
            applied.profile = Some(self.set_profile(token, mapping, note, expiration).await?);
        }
        if mapping.presence != Presence::Keep {
            match self.set_presence(token, mapping.presence).await? {
                None => applied.presence = Some(mapping.presence),
                problem => applied.problem = problem,
            }
        }
        Ok(applied)
    }

    async fn set_profile(
        &self,
        token: &Secret,
        mapping: &SlackStatus,
        note: &str,
        expiration: i64,
    ) -> Result<(String, String, i64), SlackError> {
        let text = status_text(mapping, note);
        let profile = json!({
            "status_text": text,
            "status_emoji": mapping.emoji,
            "status_expiration": expiration,
        })
        .to_string();
        self.call("users.profile.set", token, &[("profile", &profile)])
            .await?;
        Ok((mapping.emoji.clone(), text, expiration))
    }

    /// Why presence wasn't set, when the token lacks the scope.
    async fn set_presence(
        &self,
        token: &Secret,
        presence: Presence,
    ) -> Result<Option<&'static str>, SlackError> {
        let presence = if presence == Presence::Away {
            "away"
        } else {
            "auto"
        };
        match self
            .call("users.setPresence", token, &[("presence", presence)])
            .await
        {
            Ok(_) => Ok(None),
            Err(SlackError::Api { code, .. }) if code == "missing_scope" => {
                if !self.presence_scope_warned.swap(true, Ordering::Relaxed) {
                    log::warn!("The Slack token lacks users:write, so presence isn't set");
                }
                Ok(Some(MISSING_PRESENCE_SCOPE))
            }
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::fake_slack::FakeSlack;
    use super::*;

    const OK: &str = r#"{"ok":true}"#;

    fn token() -> Secret {
        Secret::new("xoxp-test-token")
    }

    fn mapping(presence: Presence) -> SlackStatus {
        SlackStatus::new(":coffee:", "Break {note}", presence)
    }

    #[test]
    fn user_and_bot_tokens_are_recognized_by_their_shape() {
        for (text, expected) in [
            ("xoxp-123-abc", true),
            ("xoxb-123-abc", true),
            ("hello", false),
            ("xoxp-", false),
            ("xoxp-123 abc", false),
            (" xoxp-123-abc", false),
            ("", false),
        ] {
            assert_eq!(is_token(text), expected, "{text:?}");
        }
    }

    #[tokio::test]
    async fn auth_test_returns_the_team_and_user_and_sends_the_token_as_bearer() {
        let server = FakeSlack::start(vec![(
            200,
            r#"{"ok":true,"team":"Acme","user":"alex","url":"https://acme.slack.com/"}"#,
        )])
        .await;

        let identity = server.api.auth_test(&token()).await.unwrap();

        assert_eq!(
            identity,
            SlackIdentity {
                team: "Acme".into(),
                user: "alex".into()
            }
        );
        let requests = server.requests();
        assert_eq!(server.paths(), vec!["/api/auth.test"]);
        assert_eq!(
            requests[0].authorization.as_deref(),
            Some("Bearer xoxp-test-token")
        );
    }

    #[tokio::test]
    async fn slack_refusing_the_token_is_an_api_error() {
        let server = FakeSlack::start(vec![(200, r#"{"ok":false,"error":"invalid_auth"}"#)]).await;

        let err = server.api.auth_test(&token()).await.unwrap_err();

        assert_eq!(
            err,
            SlackError::Api {
                method: "auth.test".into(),
                code: "invalid_auth".into()
            }
        );
    }

    #[tokio::test]
    async fn an_answer_without_an_error_code_is_an_unknown_api_error() {
        let server = FakeSlack::start(vec![(200, r#"{"ok":false}"#)]).await;

        let err = server.api.auth_test(&token()).await.unwrap_err();

        assert_eq!(
            err,
            SlackError::Api {
                method: "auth.test".into(),
                code: "unknown_error".into()
            }
        );
    }

    #[tokio::test]
    async fn an_http_error_status_is_a_net_error() {
        let server = FakeSlack::start(vec![(500, "oops")]).await;

        let err = server.api.auth_test(&token()).await.unwrap_err();

        assert!(
            matches!(&err, SlackError::Net { method, .. } if method == "auth.test"),
            "{err:?}"
        );
    }

    #[tokio::test]
    async fn an_answer_that_is_not_json_is_a_net_error() {
        let server = FakeSlack::start(vec![(200, "<html>")]).await;

        let err = server.api.auth_test(&token()).await.unwrap_err();

        assert!(matches!(err, SlackError::Net { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn a_host_that_is_not_allowed_is_a_net_error() {
        let api = SlackApi::with_base(Http::new(&["slack.com"]), "http://127.0.0.1:1/api/");

        let err = api.auth_test(&token()).await.unwrap_err();

        assert!(matches!(err, SlackError::Net { .. }), "{err:?}");
    }

    #[tokio::test]
    async fn post_message_sends_channel_and_text_and_returns_where_it_went() {
        let server = FakeSlack::start(vec![(
            200,
            r#"{"ok":true,"channel":"C0123","ts":"1700000000.000100"}"#,
        )])
        .await;

        let posted = server
            .api
            .post_message(&token(), "#general", "Hello & welcome")
            .await
            .unwrap();

        assert_eq!(
            posted,
            Posted {
                channel: "C0123".into(),
                ts: "1700000000.000100".into()
            }
        );
        let requests = server.requests();
        assert_eq!(requests[0].path, "/api/chat.postMessage");
        assert_eq!(requests[0].field("channel"), Some("#general"));
        assert_eq!(requests[0].field("text"), Some("Hello & welcome"));
    }

    #[tokio::test]
    async fn apply_sets_the_profile_with_the_note_filled_and_then_the_presence() {
        let server = FakeSlack::start(vec![(200, OK), (200, OK)]).await;

        let applied = server
            .api
            .apply(&token(), &mapping(Presence::Away), "lunch", 1_700_000_000)
            .await
            .unwrap();

        assert_eq!(
            applied,
            Applied {
                profile: Some((":coffee:".into(), "Break lunch".into(), 1_700_000_000)),
                presence: Some(Presence::Away),
                problem: None,
            }
        );
        let requests = server.requests();
        assert_eq!(
            server.paths(),
            vec!["/api/users.profile.set", "/api/users.setPresence"]
        );
        let profile: Value = serde_json::from_str(requests[0].field("profile").unwrap()).unwrap();
        assert_eq!(
            profile,
            json!({
                "status_text": "Break lunch",
                "status_emoji": ":coffee:",
                "status_expiration": 1_700_000_000,
            })
        );
        assert_eq!(requests[1].field("presence"), Some("away"));
    }

    #[tokio::test]
    async fn auto_presence_is_sent_as_auto() {
        let server = FakeSlack::start(vec![(200, OK), (200, OK)]).await;

        let applied = server
            .api
            .apply(&token(), &mapping(Presence::Auto), "", 0)
            .await
            .unwrap();

        assert_eq!(applied.presence, Some(Presence::Auto));
        assert_eq!(server.requests()[1].field("presence"), Some("auto"));
    }

    #[tokio::test]
    async fn apply_with_a_kept_status_skips_the_profile_call() {
        let server = FakeSlack::start(vec![(200, OK)]).await;
        let mut status = mapping(Presence::Away);
        status.keep_status = true;

        let applied = server.api.apply(&token(), &status, "", 0).await.unwrap();

        assert_eq!(
            applied,
            Applied {
                profile: None,
                presence: Some(Presence::Away),
                problem: None
            }
        );
        assert_eq!(server.paths(), vec!["/api/users.setPresence"]);
    }

    #[tokio::test]
    async fn apply_with_kept_presence_skips_the_presence_call() {
        let server = FakeSlack::start(vec![(200, OK)]).await;

        let applied = server
            .api
            .apply(&token(), &mapping(Presence::Keep), "x", 5)
            .await
            .unwrap();

        assert_eq!(
            applied,
            Applied {
                profile: Some((":coffee:".into(), "Break x".into(), 5)),
                presence: None,
                problem: None
            }
        );
        assert_eq!(server.paths(), vec!["/api/users.profile.set"]);
    }

    #[tokio::test]
    async fn a_token_without_the_presence_scope_still_sets_the_status() {
        let server = FakeSlack::start(vec![
            (200, OK),
            (200, r#"{"ok":false,"error":"missing_scope"}"#),
        ])
        .await;

        let applied = server
            .api
            .apply(&token(), &mapping(Presence::Away), "lunch", 0)
            .await
            .unwrap();

        assert_eq!(
            applied,
            Applied {
                profile: Some((":coffee:".into(), "Break lunch".into(), 0)),
                presence: None,
                problem: Some(MISSING_PRESENCE_SCOPE),
            }
        );
    }

    #[tokio::test]
    async fn another_presence_error_fails_the_apply() {
        let server = FakeSlack::start(vec![
            (200, OK),
            (200, r#"{"ok":false,"error":"ratelimited"}"#),
        ])
        .await;

        let err = server
            .api
            .apply(&token(), &mapping(Presence::Away), "", 0)
            .await
            .unwrap_err();

        assert_eq!(
            err,
            SlackError::Api {
                method: "users.setPresence".into(),
                code: "ratelimited".into()
            }
        );
    }

    #[tokio::test]
    async fn a_failing_profile_call_fails_the_apply_before_presence() {
        let server = FakeSlack::start(vec![(200, r#"{"ok":false,"error":"token_revoked"}"#)]).await;

        let err = server
            .api
            .apply(&token(), &mapping(Presence::Away), "", 0)
            .await
            .unwrap_err();

        assert_eq!(
            err,
            SlackError::Api {
                method: "users.profile.set".into(),
                code: "token_revoked".into()
            }
        );
        assert_eq!(server.paths(), vec!["/api/users.profile.set"]);
    }
}
