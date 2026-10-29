//! A running call's suggestion in TimeTracker: sent once the call passes the checks, grown while it
//! runs, and settled when the call is final (docs/timetracker.md#calls-become-suggestions). Pure.

use super::call_suggestion::{CallSuggestion, Skip};

/// What a final call does with its suggestion.
#[derive(Clone, Debug, PartialEq)]
pub enum Settle {
    /// Never sent while it ran: `record` it now.
    Record(CallSuggestion),
    /// Sent while it ran: its final span, note and project.
    Update {
        id: String,
        suggestion: CallSuggestion,
    },
    /// Sent while it ran, but the final call doesn't pass (the status changed): dismiss it.
    Dismiss { id: String, skip: Skip },
    /// Never sent and doesn't pass.
    Skip(Skip),
}

/// `sent` is the id of the suggestion sent while the call ran, if any.
pub fn settle(sent: Option<String>, suggestion: Result<CallSuggestion, Skip>) -> Settle {
    match (sent, suggestion) {
        (None, Ok(suggestion)) => Settle::Record(suggestion),
        (Some(id), Ok(suggestion)) => Settle::Update { id, suggestion },
        (Some(id), Err(skip)) => Settle::Dismiss { id, skip },
        (None, Err(skip)) => Settle::Skip(skip),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn suggestion() -> CallSuggestion {
        let day = NaiveDate::from_ymd_opt(2026, 10, 27).unwrap();
        CallSuggestion {
            project_id: Some("p1".into()),
            project: Some("Apollo".into()),
            start: day.and_hms_opt(10, 0, 0).unwrap(),
            end: day.and_hms_opt(10, 45, 0).unwrap(),
            note: "Zoom - Weekly".into(),
        }
    }

    #[test]
    fn a_call_never_sent_that_passes_is_recorded() {
        assert_eq!(settle(None, Ok(suggestion())), Settle::Record(suggestion()));
    }

    #[test]
    fn a_call_sent_that_passes_updates_its_suggestion() {
        assert_eq!(
            settle(Some("s-1".into()), Ok(suggestion())),
            Settle::Update {
                id: "s-1".into(),
                suggestion: suggestion()
            }
        );
    }

    #[test]
    fn a_call_sent_that_fails_dismisses_its_suggestion() {
        assert_eq!(
            settle(Some("s-1".into()), Err(Skip::NotWorking)),
            Settle::Dismiss {
                id: "s-1".into(),
                skip: Skip::NotWorking
            }
        );
    }

    #[test]
    fn a_call_never_sent_that_fails_is_skipped() {
        assert_eq!(
            settle(None, Err(Skip::TooShort)),
            Settle::Skip(Skip::TooShort)
        );
    }
}
