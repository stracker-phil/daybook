//! What Slack shows, as Daybook last set it (`slack-shown.json`; the owner never sets it by hand), for the
//! sidebar's Slack line.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::store::settings::Presence;

pub const FILE_NAME: &str = "slack-shown.json";

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase", default)]
pub struct ShownStatus {
    /// A shortcode, `:coffee:`.
    pub emoji: String,
    pub text: String,
    /// Active or away; `None` until Daybook set it.
    pub presence: Option<Presence>,
    /// Unix milliseconds when Slack clears emoji and text.
    pub expires_at: Option<f64>,
}

impl ShownStatus {
    /// Drops a presence of "no change", which Slack never shows.
    pub fn normalized(mut self) -> Self {
        if self.presence == Some(Presence::Keep) {
            self.presence = None;
        }
        self
    }

    /// After its expiry Slack shows no status (emoji and text); presence stays.
    pub fn at(&self, now_ms: f64) -> Self {
        match self.expires_at {
            Some(expires_at) if now_ms >= expires_at => Self {
                presence: self.presence,
                ..Self::default()
            },
            _ => self.clone(),
        }
    }

    /// Takes what Slack accepted: emoji, text and expiry (Unix seconds, 0: none), and presence.
    pub fn record(&mut self, profile: Option<(String, String, i64)>, presence: Option<Presence>) {
        if let Some((emoji, text, expiration)) = profile {
            self.emoji = emoji;
            self.text = text;
            self.expires_at = (expiration > 0).then_some(expiration as f64 * 1000.0);
        }
        if presence.is_some() {
            self.presence = presence;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown() -> ShownStatus {
        ShownStatus {
            emoji: ":warning:".into(),
            text: "Away later".into(),
            presence: Some(Presence::Auto),
            expires_at: Some(2_000.0),
        }
    }

    #[test]
    fn it_reads_the_camel_case_json_the_import_writes() {
        let parsed: ShownStatus = serde_json::from_str(
            r#"{"emoji":":warning:","text":"Away later","presence":"auto","expiresAt":2000.0}"#,
        )
        .unwrap();
        assert_eq!(parsed, shown());
    }

    #[test]
    fn it_serializes_with_camel_case_field_names() {
        assert_eq!(
            serde_json::to_value(shown()).unwrap(),
            serde_json::json!({
                "emoji": ":warning:",
                "text": "Away later",
                "presence": "auto",
                "expiresAt": 2000.0
            })
        );
    }

    #[test]
    fn missing_fields_default_to_nothing_shown() {
        let parsed: ShownStatus = serde_json::from_str(r#"{"text":"Away later"}"#).unwrap();
        assert_eq!(
            parsed,
            ShownStatus {
                text: "Away later".into(),
                ..ShownStatus::default()
            }
        );
    }

    #[test]
    fn a_wrongly_typed_field_fails_to_parse() {
        for json in [
            r#"{"emoji":1}"#,
            r#"{"presence":"sometimes"}"#,
            r#"{"expiresAt":"soon"}"#,
        ] {
            assert!(serde_json::from_str::<ShownStatus>(json).is_err(), "{json}");
        }
    }

    #[test]
    fn a_presence_of_no_change_is_dropped() {
        let keep = ShownStatus {
            presence: Some(Presence::Keep),
            ..shown()
        };
        assert_eq!(
            keep.normalized(),
            ShownStatus {
                presence: None,
                ..shown()
            }
        );
        assert_eq!(shown().normalized(), shown());
    }

    #[test]
    fn emoji_and_text_clear_at_expiry_while_presence_stays() {
        assert_eq!(shown().at(1_999.0), shown());
        for now in [2_000.0, 9_999.0] {
            assert_eq!(
                shown().at(now),
                ShownStatus {
                    presence: Some(Presence::Auto),
                    ..ShownStatus::default()
                },
                "{now}"
            );
        }
    }

    #[test]
    fn a_status_without_expiry_never_clears() {
        let open = ShownStatus {
            expires_at: None,
            ..shown()
        };
        assert_eq!(open.at(9_999.0), open);
    }

    #[test]
    fn recording_a_profile_takes_emoji_text_and_expiry_in_milliseconds() {
        let mut status = ShownStatus::default();
        status.record(
            Some((":coffee:".into(), "Lunch".into(), 1_800_000_000)),
            Some(Presence::Away),
        );
        assert_eq!(
            status,
            ShownStatus {
                emoji: ":coffee:".into(),
                text: "Lunch".into(),
                presence: Some(Presence::Away),
                expires_at: Some(1_800_000_000_000.0),
            }
        );
    }

    #[test]
    fn an_expiration_of_zero_means_no_expiry() {
        let mut status = shown();
        status.record(Some((":coffee:".into(), "Lunch".into(), 0)), None);
        assert_eq!(status.expires_at, None);
    }

    #[test]
    fn recording_without_presence_keeps_the_old_one_and_without_profile_the_old_text() {
        let mut status = shown();
        status.record(Some((":coffee:".into(), "Lunch".into(), 0)), None);
        assert_eq!(status.presence, Some(Presence::Auto));

        let mut status = shown();
        status.record(None, Some(Presence::Away));
        assert_eq!(
            status,
            ShownStatus {
                presence: Some(Presence::Away),
                ..shown()
            }
        );
    }
}
