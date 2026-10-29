//! Where an issue opens: in Taskboard, or in the browser when Taskboard is missing or fails.

use super::taskboard::{Failure, IssueApp};
use crate::error::{AppError, AppResult};

const NOTHING_TO_OPEN_WITH: &str =
    "Taskboard isn't installed and no Jira URL is set (Settings → Connections → Jira).";

#[derive(Debug, PartialEq)]
pub enum Opened {
    Taskboard,
    /// The caller opens `url`; `taskboard_failure` says why Taskboard didn't show it.
    Browser {
        url: String,
        taskboard_failure: Option<String>,
    },
}

/// Opens `key` in `taskboard`; when that doesn't work, falls back to the browser's `url` if there is one.
pub fn open_route(key: &str, taskboard: &impl IssueApp, url: Option<String>) -> AppResult<Opened> {
    let failure = match taskboard.open(key) {
        Ok(()) => return Ok(Opened::Taskboard),
        Err(failure) => failure,
    };
    match (url, failure) {
        (Some(url), Failure::Missing) => Ok(Opened::Browser {
            url,
            taskboard_failure: None,
        }),
        (Some(url), Failure::Failed(why)) => Ok(Opened::Browser {
            url,
            taskboard_failure: Some(why),
        }),
        (None, Failure::Missing) => Err(AppError::InvalidInput(NOTHING_TO_OPEN_WITH.into())),
        (None, Failure::Failed(why)) => Err(AppError::Io(why)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(Result<(), Failure>);

    impl IssueApp for Fake {
        fn open(&self, _key: &str) -> Result<(), Failure> {
            self.0.clone()
        }
    }

    fn url() -> Option<String> {
        Some("https://example.atlassian.net/browse/QRS-1".into())
    }

    fn boom() -> Failure {
        Failure::Failed("boom".into())
    }

    #[test]
    fn a_working_taskboard_opens_the_issue_even_with_a_browser_url() {
        for url in [url(), None] {
            assert_eq!(
                open_route("QRS-1", &Fake(Ok(())), url.clone()).unwrap(),
                Opened::Taskboard,
                "{url:?}"
            );
        }
    }

    #[test]
    fn a_missing_taskboard_falls_back_to_the_browser_without_a_reason() {
        assert_eq!(
            open_route("QRS-1", &Fake(Err(Failure::Missing)), url()).unwrap(),
            Opened::Browser {
                url: url().unwrap(),
                taskboard_failure: None,
            }
        );
    }

    #[test]
    fn a_failing_taskboard_falls_back_to_the_browser_with_its_reason() {
        assert_eq!(
            open_route("QRS-1", &Fake(Err(boom())), url()).unwrap(),
            Opened::Browser {
                url: url().unwrap(),
                taskboard_failure: Some("boom".into()),
            }
        );
    }

    #[test]
    fn a_missing_taskboard_and_no_url_is_refused_as_invalid_input() {
        let err = open_route("QRS-1", &Fake(Err(Failure::Missing)), None).unwrap_err();
        assert!(matches!(
            err,
            AppError::InvalidInput(ref message) if message
                == "Taskboard isn't installed and no Jira URL is set (Settings → Connections → Jira)."
        ));
    }

    #[test]
    fn a_failing_taskboard_and_no_url_reports_its_reason_as_an_io_error() {
        let err = open_route("QRS-1", &Fake(Err(boom())), None).unwrap_err();
        assert!(matches!(err, AppError::Io(ref message) if message == "boom"));
    }
}
