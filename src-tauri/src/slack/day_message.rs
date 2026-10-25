//! The day's first Working message in the channel (`day-message.json`): the focus list is edited into it
//! and the end-of-day list replies to it. A new day starts without one.

use serde::{Deserialize, Serialize};

pub const FILE_NAME: &str = "day-message.json";

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
    fn a_post_without_a_timestamp_is_not_remembered() {
        assert_eq!(
            DayMessage::default().remember(TODAY, "C0123", "", "hi"),
            None
        );
    }
}
