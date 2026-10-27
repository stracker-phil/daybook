//! The channel message of a status change: only what the owner confirmed in the status overlay, only
//! while the status's workflow is on, never the same message twice in a row.

use std::sync::LazyLock;

use regex::Regex;

use super::decide::fill_template;
use crate::status::{EndOfDayAnswer, StatusChanged};
use crate::store::settings::Settings;

/// `{message}` with the separator and underscores around it (` | _{message}_`).
static MESSAGE_PART: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\s*[|,:–-]?\s*_?\{message\}_?").unwrap());

/// Fills `{message}` (`Tracked: Done | _off for today_`); without a message the part around it goes.
fn end_of_day_message(template: &str, message: &str) -> String {
    if message.is_empty() {
        MESSAGE_PART.replace(template, "").trim().to_string()
    } else {
        fill_template(template, &[("message", message)])
    }
}

/// The confirmed message, wrapped by the end-of-day answer when there is one.
pub fn message_text(settings: &Settings, message: &str, answer: Option<EndOfDayAnswer>) -> String {
    let message = message.trim();
    match answer {
        Some(EndOfDayAnswer::Done) => end_of_day_message(&settings.end_of_day.done, message),
        Some(EndOfDayAnswer::Incomplete) => {
            end_of_day_message(&settings.end_of_day.incomplete, message)
        }
        None => message.to_string(),
    }
}

/// What a status change posts to the channel; `None`: nothing, also when the end-of-day reply took it.
pub fn status_message(change: &StatusChanged, settings: &Settings) -> Option<String> {
    if change.message_sent || !*settings.slack.manual_workflows.get(change.next) {
        return None;
    }
    let text = message_text(settings, &change.message, change.end_of_day);
    (!text.is_empty()).then_some(text)
}

/// The last message posted, lower-cased.
#[derive(Default)]
pub struct LastMessage(String);

impl LastMessage {
    /// Whether `message` may go out: not when it repeats the last one, ignoring case and spaces.
    pub fn admit(&mut self, message: &str) -> bool {
        let key = message.trim().to_lowercase();
        let repeated = !key.is_empty() && key == self.0;
        self.0 = key;
        !repeated
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::Status;

    fn change(next: Status, message: &str, end_of_day: Option<EndOfDayAnswer>) -> StatusChanged {
        StatusChanged {
            previous: Some(Status::Working),
            next,
            first: false,
            since: "17:30".into(),
            message: message.into(),
            end_of_day,
            message_sent: false,
            by_activity: false,
        }
    }

    #[test]
    fn a_message_that_already_went_out_with_the_end_of_day_reply_is_not_posted_again() {
        let settings = Settings::default();
        let mut sent = change(Status::Off, "all shipped", Some(EndOfDayAnswer::Done));
        sent.message_sent = true;
        assert_eq!(status_message(&sent, &settings), None);
    }

    #[test]
    fn the_message_text_is_trimmed_without_an_answer() {
        assert_eq!(
            message_text(&Settings::default(), "  Morning!  ", None),
            "Morning!"
        );
    }

    #[test]
    fn the_message_text_is_wrapped_in_the_answers_template() {
        let settings = Settings::default();
        for (answer, expected) in [
            (EndOfDayAnswer::Done, "Tracked: Done | _off for today_"),
            (
                EndOfDayAnswer::Incomplete,
                "Tracked: Incomplete | _off for today_",
            ),
        ] {
            assert_eq!(
                message_text(&settings, "  off for today ", Some(answer)),
                expected,
                "{answer:?}"
            );
        }
    }

    #[test]
    fn the_end_of_day_message_fills_the_placeholder() {
        assert_eq!(
            end_of_day_message("Tracked: Done | _{message}_", "off for today"),
            "Tracked: Done | _off for today_"
        );
    }

    #[test]
    fn without_a_message_the_part_around_the_placeholder_goes() {
        for (template, expected) in [
            ("Tracked: Done | _{message}_", "Tracked: Done"),
            ("Tracked: Done, {message}", "Tracked: Done"),
            ("Tracked: Done: {message}", "Tracked: Done"),
            ("Tracked: Done – {message}", "Tracked: Done"),
            ("Tracked: Done - {message}", "Tracked: Done"),
            ("Tracked: Done", "Tracked: Done"),
        ] {
            assert_eq!(end_of_day_message(template, ""), expected, "{template}");
        }
    }

    #[test]
    fn a_status_change_posts_the_trimmed_confirmed_message() {
        let settings = Settings::default();
        assert_eq!(
            status_message(&change(Status::Working, "  Morning!  ", None), &settings),
            Some("Morning!".to_string())
        );
    }

    #[test]
    fn an_empty_or_blank_message_posts_nothing() {
        let settings = Settings::default();
        for message in ["", "   ", "\n"] {
            assert_eq!(
                status_message(&change(Status::Working, message, None), &settings),
                None,
                "{message:?}"
            );
        }
    }

    #[test]
    fn nothing_is_posted_when_the_next_status_workflow_is_off() {
        let mut settings = Settings::default();
        settings.slack.manual_workflows.break_ = false;
        assert_eq!(
            status_message(&change(Status::Break, "lunch", None), &settings),
            None
        );
        assert_eq!(
            status_message(&change(Status::Working, "back", None), &settings),
            Some("back".to_string())
        );
    }

    #[test]
    fn going_off_through_the_end_of_day_check_wraps_the_message_in_the_answers_template() {
        let settings = Settings::default();
        assert_eq!(
            status_message(
                &change(Status::Off, "all shipped", Some(EndOfDayAnswer::Done)),
                &settings
            ),
            Some("Tracked: Done | _all shipped_".to_string())
        );
        assert_eq!(
            status_message(
                &change(Status::Off, "one left", Some(EndOfDayAnswer::Incomplete)),
                &settings
            ),
            Some("Tracked: Incomplete | _one left_".to_string())
        );
    }

    #[test]
    fn the_fixed_end_of_day_template_is_posted_even_without_a_message() {
        let settings = Settings::default();
        assert_eq!(
            status_message(
                &change(Status::Off, "", Some(EndOfDayAnswer::Done)),
                &settings
            ),
            Some("Tracked: Done".to_string())
        );
        assert_eq!(
            status_message(
                &change(Status::Off, " ", Some(EndOfDayAnswer::Incomplete)),
                &settings
            ),
            Some("Tracked: Incomplete".to_string())
        );
    }

    #[test]
    fn going_off_without_the_check_posts_the_plain_message() {
        let settings = Settings::default();
        assert_eq!(
            status_message(&change(Status::Off, "bye", None), &settings),
            Some("bye".to_string())
        );
    }

    #[test]
    fn the_same_message_twice_in_a_row_is_refused_ignoring_case_and_spacing() {
        let mut last = LastMessage::default();
        assert!(last.admit("Good morning"));
        assert!(!last.admit("good morning"));
        assert!(!last.admit("  GOOD MORNING "));
    }

    #[test]
    fn a_different_message_in_between_allows_the_first_again() {
        let mut last = LastMessage::default();
        assert!(last.admit("Good morning"));
        assert!(last.admit("Lunch"));
        assert!(last.admit("Good morning"));
    }

    #[test]
    fn an_empty_key_is_never_refused() {
        let mut last = LastMessage::default();
        assert!(last.admit(""));
        assert!(last.admit("  "));
        assert!(last.admit(""));
    }
}
