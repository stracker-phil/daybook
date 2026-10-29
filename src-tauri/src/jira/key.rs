//! Jira issue keys from what was typed, the issue's browser link, and the Jira settings' checks. Pure.

use std::sync::LazyLock;

use regex::Regex;

use crate::error::{AppError, AppResult};
use crate::store::settings::Jira;

/// A key: the project, a dash, the number.
const KEY_SHAPE: &str = r"([A-Za-z][A-Za-z0-9]*)-([0-9]+)";

static KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("^{KEY_SHAPE}$")).expect("valid key pattern"));
static KEY_IN_LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(KEY_SHAPE).expect("valid key pattern"));
/// The scheme and host of a link.
static ORIGIN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^https?://[^/\s]+").expect("valid origin pattern"));
static PREFIX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Z][A-Z0-9]*$").expect("valid prefix pattern"));

fn key_of(captures: &regex::Captures) -> String {
    format!("{}-{}", captures[1].to_uppercase(), &captures[2])
}

/// `6805` with the default prefix, a key in any case (`qrs-6805`) or a link holding one; `None` otherwise.
pub fn parse_issue_key(input: &str, default_prefix: &str) -> Option<String> {
    let value = input.trim();
    if !value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()) {
        let prefix = default_prefix.trim();
        return (!prefix.is_empty()).then(|| format!("{}-{value}", prefix.to_uppercase()));
    }
    if let Some(origin) = ORIGIN.find(value) {
        let path = &value[origin.end()..];
        return KEY_IN_LINK.captures(path).map(|c| key_of(&c));
    }
    KEY.captures(value).map(|c| key_of(&c))
}

/// The key `input` names; refused when it names no issue.
pub fn issue_key(jira: &Jira, input: &str) -> AppResult<String> {
    parse_issue_key(input, &jira.default_prefix).ok_or_else(|| {
        AppError::InvalidInput(format!(
            "“{}” isn't a Jira issue number, key or link.",
            input.trim()
        ))
    })
}

/// The issue's link in the browser; `None` without a Jira URL in the settings.
pub fn browser_url(jira: &Jira, key: &str) -> Option<String> {
    let base = jira.base_url.trim();
    if base.is_empty() {
        return None;
    }
    let slash = if base.ends_with('/') { "" } else { "/" };
    Some(format!("{base}{slash}{key}"))
}

/// The Jira settings as saved: trimmed, the prefix in capitals; a URL or prefix that can't work is refused.
pub fn checked_settings(jira: Jira) -> AppResult<Jira> {
    let base_url = jira.base_url.trim().to_string();
    if !base_url.is_empty() && !ORIGIN.is_match(&base_url) {
        return Err(AppError::InvalidInput(
            "The Jira URL must start with https://, e.g. https://example.atlassian.net/browse/"
                .into(),
        ));
    }
    let default_prefix = jira.default_prefix.trim().to_uppercase();
    if !default_prefix.is_empty() && !PREFIX.is_match(&default_prefix) {
        return Err(AppError::InvalidInput(
            "The default prefix must be a project key like QRS.".into(),
        ));
    }
    Ok(Jira {
        base_url,
        default_prefix,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jira(base_url: &str, default_prefix: &str) -> Jira {
        Jira {
            base_url: base_url.into(),
            default_prefix: default_prefix.into(),
        }
    }

    fn invalid_input_message(result: AppResult<impl std::fmt::Debug>) -> String {
        match result {
            Err(AppError::InvalidInput(message)) => message,
            other => panic!("expected InvalidInput, got {other:?}"),
        }
    }

    #[test]
    fn a_number_gets_the_default_project_a_key_any_case_and_a_link_its_key() {
        for (input, prefix, expected) in [
            ("6805", "qrs", Some("QRS-6805")),
            ("6805", "", None),
            (" qrs-6805 ", "", Some("QRS-6805")),
            (
                "https://x.atlassian.net/browse/QRS-6805?focus=1",
                "",
                Some("QRS-6805"),
            ),
            ("https://x-1.example.com/jira/qrs-7", "", Some("QRS-7")),
            ("fix the bug", "QRS", None),
        ] {
            assert_eq!(
                parse_issue_key(input, prefix).as_deref(),
                expected,
                "{input:?} with {prefix:?}"
            );
        }
    }

    #[test]
    fn the_host_of_a_link_is_not_read_as_a_key() {
        assert_eq!(parse_issue_key("https://abc-123.example.com/", ""), None);
        assert_eq!(
            parse_issue_key("HTTPS://x.com/browse/ab-1", ""),
            Some("AB-1".into())
        );
    }

    #[test]
    fn text_around_a_key_is_not_a_key() {
        for input in ["", "   ", "QRS-", "-6805", "QRS-68x", "see QRS-6805"] {
            assert_eq!(parse_issue_key(input, "QRS"), None, "{input:?}");
        }
    }

    #[test]
    fn the_browser_url_joins_the_base_and_the_key() {
        for base in [
            "https://example.atlassian.net/browse/",
            "https://example.atlassian.net/browse",
            "  https://example.atlassian.net/browse/  ",
        ] {
            assert_eq!(
                browser_url(&jira(base, "QRS"), "QRS-6805").as_deref(),
                Some("https://example.atlassian.net/browse/QRS-6805"),
                "{base:?}"
            );
        }
    }

    #[test]
    fn there_is_no_browser_url_without_a_jira_url_in_settings() {
        for base in ["", "  "] {
            assert_eq!(
                browser_url(&jira(base, "QRS"), "QRS-6805"),
                None,
                "{base:?}"
            );
        }
    }

    #[test]
    fn an_issue_key_comes_from_a_number_a_key_or_a_link() {
        let settings = jira("https://example.atlassian.net/browse/", "qrs");
        for input in [
            "6805",
            " qrs-6805 ",
            "https://x.atlassian.net/browse/QRS-6805",
        ] {
            assert_eq!(
                issue_key(&settings, input).unwrap(),
                "QRS-6805",
                "{input:?}"
            );
        }
    }

    #[test]
    fn an_issue_key_needs_no_jira_url() {
        assert_eq!(issue_key(&jira("", "QRS"), "6805").unwrap(), "QRS-6805");
    }

    #[test]
    fn a_bare_number_without_a_default_prefix_is_refused() {
        assert_eq!(
            invalid_input_message(issue_key(&jira("", ""), "6805")),
            "“6805” isn't a Jira issue number, key or link."
        );
    }

    #[test]
    fn an_input_that_is_no_issue_is_refused_with_what_was_typed() {
        let settings = jira("https://example.atlassian.net/browse/", "QRS");
        assert_eq!(
            invalid_input_message(issue_key(&settings, "  fix the bug ")),
            "“fix the bug” isn't a Jira issue number, key or link."
        );
    }

    #[test]
    fn saved_settings_are_trimmed_with_the_prefix_in_capitals() {
        let saved = checked_settings(jira("  https://example.atlassian.net/browse/ ", " qrs "));
        assert_eq!(
            saved.unwrap(),
            jira("https://example.atlassian.net/browse/", "QRS")
        );
    }

    #[test]
    fn empty_and_http_settings_are_accepted() {
        assert_eq!(checked_settings(jira("", "")).unwrap(), jira("", ""));
        assert_eq!(
            checked_settings(jira("http://jira.local/browse/", "ab1")).unwrap(),
            jira("http://jira.local/browse/", "AB1")
        );
    }

    #[test]
    fn a_url_that_doesnt_start_with_a_web_scheme_is_refused() {
        for base in [
            "example.atlassian.net/browse/",
            "ftp://example.com/",
            "https://",
            "https:// x",
        ] {
            assert_eq!(
                invalid_input_message(checked_settings(jira(base, "QRS"))),
                "The Jira URL must start with https://, e.g. https://example.atlassian.net/browse/",
                "{base:?}"
            );
        }
    }

    #[test]
    fn a_prefix_that_is_no_project_key_is_refused() {
        for prefix in ["1AB", "P-C", "P C", "ÄB"] {
            assert_eq!(
                invalid_input_message(checked_settings(jira("", prefix))),
                "The default prefix must be a project key like QRS.",
                "{prefix:?}"
            );
        }
    }
}
