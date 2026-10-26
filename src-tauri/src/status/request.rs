//! What the status overlay and the offer-off overlay are opened with. Pure.

use serde::Serialize;
use ts_rs::TS;

use super::memory::StatusMemory;
use crate::store::settings::{Context, Feature, Settings, Status, StatusMap};

/// The end-of-day reminder and question shown before going Off.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct EndOfDayCheck {
    pub reminder: String,
    pub question: String,
    /// Opened from the question; empty for none.
    pub url: String,
}

/// The status overlay's session args.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct StatusRequest {
    /// Picked when the overlay opens.
    pub status: Status,
    /// Today's status; `None` before the first one.
    pub current: Option<Status>,
    /// The message each status suggests.
    pub messages: StatusMap<String>,
    /// `None` when going Off asks nothing first.
    pub end_of_day: Option<EndOfDayCheck>,
    /// Going Off asks for the end-of-day list first (the focus list goes to Slack, once a day).
    pub end_of_day_list: bool,
}

pub fn status_request(settings: &Settings, today: &StatusMemory, status: Status) -> StatusRequest {
    let messages = &settings.status_messages;
    let first_working = !today.worked_today && !messages.first_working.is_empty();
    let check = &settings.end_of_day;
    StatusRequest {
        status,
        current: today.status,
        messages: StatusMap {
            working: if first_working {
                messages.first_working.clone()
            } else {
                messages.working.clone()
            },
            break_: messages.break_.clone(),
            off: messages.off.clone(),
        },
        end_of_day: (check.enabled && settings.slack.manual_workflows.off).then(|| EndOfDayCheck {
            reminder: check.reminder.clone(),
            question: check.question.clone(),
            url: check.url.clone(),
        }),
        end_of_day_list: false,
    }
}

/// The status a toggle picks: Break while Working, else Working.
pub fn toggled(current: Option<Status>) -> Status {
    if current == Some(Status::Working) {
        Status::Break
    } else {
        Status::Working
    }
}

/// The offer-off overlay's session args.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct OfferOff {
    /// The context switched to, e.g. `🐈 Personal`.
    pub to: String,
}

/// Leaving a context with a status for one without, while Working, offers to go Off.
pub fn offers_off(previous: Option<&Context>, next: &Context, status: Option<Status>) -> bool {
    let has_status = |context: &Context| context.features.contains(&Feature::Status);
    status == Some(Status::Working) && previous.is_some_and(has_status) && !has_status(next)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn memory(status: Option<Status>, worked_today: bool) -> StatusMemory {
        StatusMemory {
            date: "2026-10-24".into(),
            status,
            since: None,
            worked_today,
        }
    }

    fn settings() -> Settings {
        let mut settings = Settings::default();
        settings.status_messages.working = "re".into();
        settings.status_messages.first_working = "morning".into();
        settings.status_messages.break_ = "afk".into();
        settings.status_messages.off = "bye".into();
        settings.end_of_day.enabled = true;
        settings.end_of_day.reminder = "Post the EOD".into();
        settings.end_of_day.question = "Tracked?".into();
        settings.end_of_day.url = "https://example.com/time".into();
        settings.slack.manual_workflows.off = true;
        settings
    }

    #[test]
    fn the_request_carries_the_picked_and_the_current_status() {
        let request = status_request(
            &settings(),
            &memory(Some(Status::Break), true),
            Status::Working,
        );
        assert_eq!(request.status, Status::Working);
        assert_eq!(request.current, Some(Status::Break));
        let before_any = status_request(&settings(), &memory(None, false), Status::Break);
        assert_eq!(before_any.status, Status::Break);
        assert_eq!(before_any.current, None);
    }

    #[test]
    fn the_request_leaves_the_end_of_day_list_to_the_caller() {
        let request = status_request(&settings(), &memory(None, false), Status::Off);
        assert!(!request.end_of_day_list);
    }

    #[test]
    fn each_status_suggests_its_message() {
        let request = status_request(
            &settings(),
            &memory(Some(Status::Working), true),
            Status::Off,
        );
        assert_eq!(
            request.messages,
            StatusMap {
                working: "re".into(),
                break_: "afk".into(),
                off: "bye".into(),
            }
        );
    }

    #[test]
    fn the_days_first_working_suggests_the_first_working_message() {
        let request = status_request(&settings(), &memory(None, false), Status::Working);
        assert_eq!(request.messages.working, "morning");
    }

    #[test]
    fn an_empty_first_working_message_falls_back_to_the_working_message() {
        let mut settings = settings();
        settings.status_messages.first_working = String::new();
        let request = status_request(&settings, &memory(None, false), Status::Working);
        assert_eq!(request.messages.working, "re");
    }

    #[test]
    fn once_worked_today_working_suggests_the_working_message() {
        let request = status_request(
            &settings(),
            &memory(Some(Status::Break), true),
            Status::Working,
        );
        assert_eq!(request.messages.working, "re");
    }

    #[test]
    fn going_off_asks_the_end_of_day_check_when_it_and_the_off_workflow_are_on() {
        let request = status_request(&settings(), &memory(None, false), Status::Off);
        assert_eq!(
            request.end_of_day,
            Some(EndOfDayCheck {
                reminder: "Post the EOD".into(),
                question: "Tracked?".into(),
                url: "https://example.com/time".into(),
            })
        );
    }

    #[test]
    fn going_off_asks_nothing_when_the_check_or_the_off_workflow_is_off() {
        let mut check_off = settings();
        check_off.end_of_day.enabled = false;
        let mut workflow_off = settings();
        workflow_off.slack.manual_workflows.off = false;
        for (name, settings) in [("check", check_off), ("workflow", workflow_off)] {
            let request = status_request(&settings, &memory(None, false), Status::Off);
            assert_eq!(request.end_of_day, None, "{name}");
        }
    }

    #[test]
    fn a_request_serializes_with_camel_case_fields() {
        let request = status_request(
            &settings(),
            &memory(Some(Status::Working), true),
            Status::Break,
        );
        assert_eq!(
            serde_json::to_value(&request).unwrap(),
            json!({
                "status": "break",
                "current": "working",
                "messages": { "working": "re", "break": "afk", "off": "bye" },
                "endOfDay": {
                    "reminder": "Post the EOD",
                    "question": "Tracked?",
                    "url": "https://example.com/time"
                },
                "endOfDayList": false
            })
        );
    }

    #[test]
    fn toggling_breaks_while_working_and_works_otherwise() {
        for (current, expected) in [
            (Some(Status::Working), Status::Break),
            (Some(Status::Break), Status::Working),
            (Some(Status::Off), Status::Working),
            (None, Status::Working),
        ] {
            assert_eq!(toggled(current), expected, "{current:?}");
        }
    }

    fn contexts() -> Vec<Context> {
        Context::read_all(&[
            json!({ "name": "Work", "id": "work", "features": ["status", "jira"] }),
            json!({ "name": "Other Work", "id": "other", "features": ["status"] }),
            json!({ "name": "Personal", "id": "personal", "features": [] }),
            json!({ "name": "Private", "id": "private", "features": ["jira"] }),
        ])
    }

    #[test]
    fn leaving_a_status_context_for_one_without_offers_off_while_working() {
        let contexts = contexts();
        for next in [&contexts[2], &contexts[3]] {
            assert!(
                offers_off(Some(&contexts[0]), next, Some(Status::Working)),
                "{}",
                next.name
            );
        }
    }

    #[test]
    fn nothing_is_offered_unless_working() {
        let contexts = contexts();
        for status in [Some(Status::Break), Some(Status::Off), None] {
            assert!(
                !offers_off(Some(&contexts[0]), &contexts[2], status),
                "{status:?}"
            );
        }
    }

    #[test]
    fn nothing_is_offered_without_a_previous_context() {
        let contexts = contexts();
        assert!(!offers_off(None, &contexts[2], Some(Status::Working)));
    }

    #[test]
    fn nothing_is_offered_when_the_next_context_has_a_status_too() {
        let contexts = contexts();
        assert!(!offers_off(
            Some(&contexts[0]),
            &contexts[1],
            Some(Status::Working)
        ));
    }

    #[test]
    fn nothing_is_offered_when_the_previous_context_had_no_status() {
        let contexts = contexts();
        for previous in [&contexts[2], &contexts[3]] {
            assert!(
                !offers_off(Some(previous), &contexts[2], Some(Status::Working)),
                "{}",
                previous.name
            );
        }
    }

    #[test]
    fn the_offer_serializes_the_context_switched_to() {
        let offer = OfferOff {
            to: "🐈 Personal".into(),
        };
        assert_eq!(
            serde_json::to_value(&offer).unwrap(),
            json!({ "to": "🐈 Personal" })
        );
    }
}
