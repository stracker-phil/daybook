//! The day's first Working message in the channel (`day-message.json`): the focus list is edited into it
//! and the end-of-day list replies to it. A new day starts without one.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const FILE_NAME: &str = "day-message.json";

/// Where the focus list goes in Slack.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "kebab-case")]
pub enum FocusTarget {
    /// Not shared in Slack here.
    None,
    /// No message today yet: the list becomes the day's message.
    Post,
    /// Into the day's message.
    Edit,
    /// The end-of-day list went out; the list stays in the note.
    Sent,
}

/// Slack's answers when a remembered message can't be used any more: deleted, in another workspace,
/// too old to edit, or its channel is gone.
const MESSAGE_GONE: &[&str] = &[
    "message_not_found",
    "thread_not_found",
    "channel_not_found",
    "cant_update_message",
    "edit_window_closed",
    "is_archived",
];

pub fn message_gone(code: &str) -> bool {
    MESSAGE_GONE.contains(&code)
}

/// The parts that aren't empty, a blank line apart.
pub fn paragraphs(parts: &[&str]) -> String {
    parts
        .iter()
        .filter(|part| !part.trim().is_empty())
        .copied()
        .collect::<Vec<_>>()
        .join("\n\n")
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct DayMessage {
    /// `YYYY-MM-DD`
    pub date: String,
    /// Channel id and timestamp as Slack returned them; `None` until a message was posted that day.
    pub channel: Option<String>,
    pub ts: Option<String>,
    /// The message's own text, kept above the list.
    pub greeting: String,
    pub eod_sent: bool,
}

impl DayMessage {
    /// The message of `today`; a remembered one from another day is gone.
    pub fn of(&self, today: &str) -> Self {
        if self.date == today {
            self.clone()
        } else {
            Self {
                date: today.into(),
                ..Self::default()
            }
        }
    }

    /// The message's channel id and timestamp, once it was posted.
    pub fn anchor(&self) -> Option<(&str, &str)> {
        Some((self.channel.as_deref()?, self.ts.as_deref()?))
    }

    /// Where the list goes when it `shares` in Slack.
    pub fn target(&self, shares: bool) -> FocusTarget {
        match (shares, self.eod_sent, &self.ts) {
            (false, _, _) => FocusTarget::None,
            (true, true, _) => FocusTarget::Sent,
            (true, false, Some(_)) => FocusTarget::Edit,
            (true, false, None) => FocusTarget::Post,
        }
    }

    /// Remembers the posted message unless `today` already has one; `None`: nothing changed.
    pub fn remember(&self, today: &str, channel: &str, ts: &str, greeting: &str) -> Option<Self> {
        let day = self.of(today);
        if day.ts.is_some() || ts.is_empty() {
            return None;
        }
        Some(Self {
            channel: Some(channel.into()),
            ts: Some(ts.into()),
            greeting: greeting.trim().into(),
            ..day
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TODAY: &str = "2026-10-23";

    fn posted(date: &str) -> DayMessage {
        DayMessage {
            date: date.into(),
            channel: Some("C0123".into()),
            ts: Some("1700000000.000100".into()),
            greeting: "good morning :wave:".into(),
            eod_sent: false,
        }
    }

    #[test]
    fn it_round_trips_the_camel_case_json() {
        let json = r#"{"date":"2026-10-23","channel":"C0123","ts":"1700000000.000100","greeting":"good morning :wave:","eodSent":false}"#;
        let parsed: DayMessage = serde_json::from_str(json).unwrap();
        assert_eq!(parsed, posted(TODAY));
        assert_eq!(
            serde_json::to_value(&parsed).unwrap(),
            serde_json::from_str::<serde_json::Value>(json).unwrap()
        );
    }

    #[test]
    fn todays_message_is_kept() {
        assert_eq!(posted(TODAY).of(TODAY), posted(TODAY));
    }

    #[test]
    fn another_days_message_becomes_a_new_empty_one_for_today() {
        assert_eq!(
            posted("2026-10-22").of(TODAY),
            DayMessage {
                date: TODAY.into(),
                ..DayMessage::default()
            }
        );
    }

    #[test]
    fn the_first_message_of_the_day_is_remembered_with_a_trimmed_greeting() {
        let remembered = DayMessage::default()
            .remember(
                TODAY,
                "C0123",
                "1700000000.000100",
                "  good morning :wave: \n",
            )
            .unwrap();
        assert_eq!(remembered, posted(TODAY));
    }

    #[test]
    fn a_message_from_yesterday_is_replaced() {
        let remembered = posted("2026-10-22")
            .remember(TODAY, "C0456", "1700000500.000200", "hi")
            .unwrap();
        assert_eq!(
            remembered,
            DayMessage {
                date: TODAY.into(),
                channel: Some("C0456".into()),
                ts: Some("1700000500.000200".into()),
                greeting: "hi".into(),
                eod_sent: false,
            }
        );
    }

    #[test]
    fn only_the_first_message_of_the_day_counts() {
        assert_eq!(
            posted(TODAY).remember(TODAY, "C0456", "1700000500.000200", "later"),
            None
        );
    }

    #[test]
    fn the_list_goes_where_the_days_message_and_sharing_say() {
        let sent = DayMessage {
            eod_sent: true,
            ..posted(TODAY)
        };
        let unposted = DayMessage {
            date: TODAY.into(),
            ..DayMessage::default()
        };
        for (name, message, shares, expected) in [
            ("not shared", posted(TODAY), false, FocusTarget::None),
            ("not shared, sent", sent.clone(), false, FocusTarget::None),
            ("sent", sent, true, FocusTarget::Sent),
            ("posted", posted(TODAY), true, FocusTarget::Edit),
            ("no message yet", unposted, true, FocusTarget::Post),
        ] {
            assert_eq!(message.target(shares), expected, "{name}");
        }
    }

    #[test]
    fn a_target_serializes_in_kebab_case() {
        for (target, expected) in [
            (FocusTarget::None, "none"),
            (FocusTarget::Post, "post"),
            (FocusTarget::Edit, "edit"),
            (FocusTarget::Sent, "sent"),
        ] {
            assert_eq!(
                serde_json::to_value(target).unwrap(),
                serde_json::json!(expected)
            );
        }
    }

    #[test]
    fn slack_codes_for_an_unusable_message_mean_it_is_gone() {
        for code in [
            "message_not_found",
            "thread_not_found",
            "channel_not_found",
            "cant_update_message",
            "edit_window_closed",
            "is_archived",
        ] {
            assert!(message_gone(code), "{code}");
        }
        for code in ["ratelimited", "invalid_auth", "missing_scope", ""] {
            assert!(!message_gone(code), "{code}");
        }
    }

    #[test]
    fn paragraphs_join_the_parts_that_are_not_blank_with_a_blank_line() {
        assert_eq!(
            paragraphs(&["good morning", "", "  \n", "- one\n- two"]),
            "good morning\n\n- one\n- two"
        );
        assert_eq!(paragraphs(&["", " "]), "");
        assert_eq!(paragraphs(&[]), "");
    }

    #[test]
    fn a_post_without_a_timestamp_is_not_remembered() {
        assert_eq!(
            DayMessage::default().remember(TODAY, "C0123", "", "hi"),
            None
        );
    }
}
