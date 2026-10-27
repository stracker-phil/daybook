//! The verbs' parameters, read and checked before anything reaches a window: the `act` kind, the
//! `window` request, and `type`'s mode, `scroll`'s distance and `wait`'s timeout. Pure.

use std::str::FromStr;

use super::page::{ScrollTo, TypeMode};
use super::protocol::{Bounds, StageResult, parse_bounds, parse_whole, trimmed};
use super::windows::{StagePage, is_section};
use crate::automation::{AutomationError, Params};

const WAIT_DEFAULT_MS: u64 = 2000;
const WAIT_MAX_MS: u64 = 5000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActKind {
    Command,
    Reveal,
    Click,
    Type,
    Keys,
    Scroll,
    Wait,
}

impl FromStr for ActKind {
    type Err = AutomationError;

    fn from_str(kind: &str) -> Result<Self, Self::Err> {
        Ok(match kind.trim() {
            "command" => ActKind::Command,
            "reveal" => ActKind::Reveal,
            "click" => ActKind::Click,
            "type" => ActKind::Type,
            "keys" => ActKind::Keys,
            "scroll" => ActKind::Scroll,
            "wait" => ActKind::Wait,
            _ => {
                return Err(AutomationError::bad(
                    "“kind” must be command, reveal, click, type, keys, scroll or wait.",
                ));
            }
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    Show,
    Hide,
    Focus,
    Place,
}

impl FromStr for WindowAction {
    type Err = AutomationError;

    fn from_str(action: &str) -> Result<Self, Self::Err> {
        Ok(match action {
            "show" => WindowAction::Show,
            "hide" => WindowAction::Hide,
            "focus" => WindowAction::Focus,
            "place" => WindowAction::Place,
            _ => {
                return Err(AutomationError::bad(
                    "“action” must be show, hide, focus or place.",
                ));
            }
        })
    }
}

/// The window named by `window`; 404 for an unknown one.
pub fn page_param(params: &Params) -> StageResult<StagePage> {
    params
        .get("window")
        .map(|w| w.trim())
        .unwrap_or_default()
        .parse()
}

/// A `window` request: without `action`, bounds mean place and none mean show. `section` only goes
/// with showing Settings and is checked before the action.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowRequest {
    pub page: StagePage,
    pub action: WindowAction,
    pub bounds: Bounds,
    pub section: Option<String>,
}

impl WindowRequest {
    pub fn parse(params: &Params) -> StageResult<Self> {
        let page = page_param(params)?;
        let bounds = parse_bounds(params)?;
        let default = if bounds.is_empty() { "show" } else { "place" };
        let action = trimmed(params, "action").unwrap_or(default);
        let section = trimmed(params, "section").map(String::from);
        if let Some(section) = &section {
            if page != StagePage::Settings || action != "show" {
                return Err(AutomationError::bad(
                    "“section” only goes with showing the settings window.",
                ));
            }
            if !is_section(section) {
                return Err(AutomationError::new(
                    404,
                    format!(
                        "Unknown section “{section}”; describe lists the settings window's sections."
                    ),
                ));
            }
        }
        Ok(Self {
            page,
            action: action.parse()?,
            bounds,
            section,
        })
    }
}

/// `mode` of `type`: replace (the default) or append at the caret.
pub fn parse_type_mode(raw: Option<&str>) -> StageResult<TypeMode> {
    match raw.map(str::trim).filter(|m| !m.is_empty()) {
        None | Some("replace") => Ok(TypeMode::Replace),
        Some("append") => Ok(TypeMode::Append),
        Some(_) => Err(AutomationError::bad("“mode” must be replace or append.")),
    }
}

/// `scroll`'s `by` (points, positive downwards) or `to` (top, bottom); never both.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Scroll {
    By(f64),
    To(ScrollTo),
}

pub fn parse_scroll(params: &Params) -> StageResult<Scroll> {
    let by = trimmed(params, "by");
    let to = trimmed(params, "to");
    if by.is_some() && to.is_some() {
        return Err(AutomationError::bad("Give “by” or “to”, not both."));
    }
    if let Some(to) = to {
        return match to {
            "top" => Ok(Scroll::To(ScrollTo::Top)),
            "bottom" => Ok(Scroll::To(ScrollTo::Bottom)),
            _ => Err(AutomationError::bad("“to” must be top or bottom.")),
        };
    }
    let Some(by) = by else {
        return Err(AutomationError::bad(
            "Give “by” (points, positive downwards) or “to” (top or bottom).",
        ));
    };
    by.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && *v != 0.0)
        .map(Scroll::By)
        .ok_or_else(|| AutomationError::bad("“by” must be a number of points other than 0."))
}

/// `timeout` of `wait` in milliseconds: 2000 unless given, at most 5000.
pub fn parse_wait_timeout(raw: Option<&str>) -> StageResult<u64> {
    let Some(text) = raw.map(str::trim).filter(|t| !t.is_empty()) else {
        return Ok(WAIT_DEFAULT_MS);
    };
    parse_whole(text)
        .filter(|v| *v > 0)
        .map(|v| (v as u64).min(WAIT_MAX_MS))
        .ok_or_else(|| {
            AutomationError::bad("“timeout” must be a positive whole number of milliseconds.")
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(items: &[(&str, &str)]) -> Params {
        items
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn window_request(items: &[(&str, &str)]) -> StageResult<WindowRequest> {
        WindowRequest::parse(&params(items))
    }

    #[test]
    fn every_act_kind_is_found_by_its_name() {
        for (name, kind) in [
            ("command", ActKind::Command),
            ("reveal", ActKind::Reveal),
            ("click", ActKind::Click),
            ("type", ActKind::Type),
            ("keys", ActKind::Keys),
            ("scroll", ActKind::Scroll),
            ("wait", ActKind::Wait),
            (" click ", ActKind::Click),
        ] {
            assert_eq!(name.parse::<ActKind>(), Ok(kind), "{name:?}");
        }
    }

    #[test]
    fn an_unknown_act_kind_is_a_bad_request_listing_the_kinds() {
        for name in ["", "tap", "Click"] {
            assert_eq!(
                name.parse::<ActKind>(),
                Err(AutomationError::bad(
                    "“kind” must be command, reveal, click, type, keys, scroll or wait."
                )),
                "{name:?}"
            );
        }
    }

    #[test]
    fn every_window_action_is_found_by_its_name() {
        for (name, action) in [
            ("show", WindowAction::Show),
            ("hide", WindowAction::Hide),
            ("focus", WindowAction::Focus),
            ("place", WindowAction::Place),
        ] {
            assert_eq!(name.parse::<WindowAction>(), Ok(action), "{name}");
        }
        assert_eq!(
            "close".parse::<WindowAction>(),
            Err(AutomationError::bad(
                "“action” must be show, hide, focus or place."
            ))
        );
    }

    #[test]
    fn the_window_param_names_a_page_or_is_not_found() {
        assert_eq!(
            page_param(&params(&[("window", " quick-notes ")])),
            Ok(StagePage::QuickNotes)
        );
        for items in [vec![], vec![("window", "")], vec![("window", "dock")]] {
            assert_eq!(
                page_param(&params(&items)),
                Err(AutomationError::new(
                    404,
                    "“window” must be one of hub, settings, panel, rail, quick-notes."
                )),
                "{items:?}"
            );
        }
    }

    #[test]
    fn a_window_request_without_bounds_or_action_shows() {
        assert_eq!(
            window_request(&[("window", "hub")]),
            Ok(WindowRequest {
                page: StagePage::Hub,
                action: WindowAction::Show,
                bounds: Bounds::default(),
                section: None,
            })
        );
    }

    #[test]
    fn bounds_without_an_action_mean_place() {
        assert_eq!(
            window_request(&[("window", "hub"), ("x", "10"), ("width", "800")]),
            Ok(WindowRequest {
                page: StagePage::Hub,
                action: WindowAction::Place,
                bounds: Bounds {
                    x: Some(10),
                    width: Some(800),
                    ..Bounds::default()
                },
                section: None,
            })
        );
    }

    #[test]
    fn an_explicit_action_wins_over_the_bounds() {
        let request = window_request(&[("window", "hub"), ("action", "hide"), ("y", "5")]).unwrap();

        assert_eq!(request.action, WindowAction::Hide);
        assert_eq!(request.bounds.y, Some(5));
    }

    #[test]
    fn settings_shows_a_known_section() {
        assert_eq!(
            window_request(&[
                ("window", "settings"),
                ("action", "show"),
                ("section", " automation ")
            ]),
            Ok(WindowRequest {
                page: StagePage::Settings,
                action: WindowAction::Show,
                bounds: Bounds::default(),
                section: Some("automation".into()),
            })
        );
        assert_eq!(
            window_request(&[("window", "settings"), ("section", "vault")])
                .unwrap()
                .action,
            WindowAction::Show
        );
    }

    #[test]
    fn a_section_only_goes_with_showing_the_settings_window() {
        for items in [
            vec![("window", "hub"), ("section", "vault")],
            vec![
                ("window", "settings"),
                ("action", "hide"),
                ("section", "vault"),
            ],
            vec![("window", "settings"), ("x", "5"), ("section", "vault")],
            vec![
                ("window", "settings"),
                ("action", "bogus"),
                ("section", "vault"),
            ],
        ] {
            assert_eq!(
                window_request(&items),
                Err(AutomationError::bad(
                    "“section” only goes with showing the settings window."
                )),
                "{items:?}"
            );
        }
    }

    #[test]
    fn an_unknown_section_is_not_found() {
        assert_eq!(
            window_request(&[("window", "settings"), ("section", "nope")]),
            Err(AutomationError::new(
                404,
                "Unknown section “nope”; describe lists the settings window's sections."
            ))
        );
    }

    #[test]
    fn an_unknown_window_action_is_a_bad_request() {
        assert_eq!(
            window_request(&[("window", "hub"), ("action", "minimize")]),
            Err(AutomationError::bad(
                "“action” must be show, hide, focus or place."
            ))
        );
    }

    #[test]
    fn an_unknown_window_is_not_found_before_anything_else_is_checked() {
        let error =
            window_request(&[("window", "dock"), ("width", "0"), ("action", "x")]).unwrap_err();

        assert_eq!(error.status, 404);
    }

    #[test]
    fn bad_bounds_are_reported_before_the_action() {
        assert_eq!(
            window_request(&[("window", "hub"), ("width", "0"), ("action", "x")]),
            Err(AutomationError::bad("“width” must be positive."))
        );
    }

    #[test]
    fn the_type_mode_is_replace_unless_append_is_given() {
        for (raw, mode) in [
            (None, TypeMode::Replace),
            (Some(""), TypeMode::Replace),
            (Some("  "), TypeMode::Replace),
            (Some("replace"), TypeMode::Replace),
            (Some("append"), TypeMode::Append),
            (Some(" append "), TypeMode::Append),
        ] {
            assert_eq!(parse_type_mode(raw), Ok(mode), "{raw:?}");
        }
    }

    #[test]
    fn an_unknown_type_mode_is_a_bad_request() {
        assert_eq!(
            parse_type_mode(Some("insert")),
            Err(AutomationError::bad("“mode” must be replace or append."))
        );
    }

    #[test]
    fn scroll_reads_a_distance_or_an_end() {
        for (items, expected) in [
            (vec![("by", "-240")], Scroll::By(-240.0)),
            (vec![("by", " 12.5 ")], Scroll::By(12.5)),
            (vec![("to", "bottom")], Scroll::To(ScrollTo::Bottom)),
            (vec![("to", "top")], Scroll::To(ScrollTo::Top)),
            (vec![("by", ""), ("to", "top")], Scroll::To(ScrollTo::Top)),
        ] {
            assert_eq!(parse_scroll(&params(&items)), Ok(expected), "{items:?}");
        }
    }

    #[test]
    fn scroll_refuses_missing_conflicting_and_unusable_values() {
        for (items, message) in [
            (
                vec![],
                "Give “by” (points, positive downwards) or “to” (top or bottom).",
            ),
            (
                vec![("by", "10"), ("to", "top")],
                "Give “by” or “to”, not both.",
            ),
            (vec![("to", "middle")], "“to” must be top or bottom."),
            (
                vec![("by", "0")],
                "“by” must be a number of points other than 0.",
            ),
            (
                vec![("by", "abc")],
                "“by” must be a number of points other than 0.",
            ),
            (
                vec![("by", "NaN")],
                "“by” must be a number of points other than 0.",
            ),
            (
                vec![("by", "inf")],
                "“by” must be a number of points other than 0.",
            ),
        ] {
            assert_eq!(
                parse_scroll(&params(&items)),
                Err(AutomationError::bad(message)),
                "{items:?}"
            );
        }
    }

    #[test]
    fn the_wait_timeout_defaults_to_two_seconds_and_caps_at_five() {
        for (raw, expected) in [
            (None, 2000),
            (Some(""), 2000),
            (Some("  "), 2000),
            (Some("800"), 800),
            (Some(" 1 "), 1),
            (Some("5000"), 5000),
            (Some("60000"), 5000),
        ] {
            assert_eq!(parse_wait_timeout(raw), Ok(expected), "{raw:?}");
        }
    }

    #[test]
    fn a_wait_timeout_must_be_a_positive_whole_number() {
        for raw in ["soon", "0", "1.5", "-3", "inf"] {
            assert_eq!(
                parse_wait_timeout(Some(raw)),
                Err(AutomationError::bad(
                    "“timeout” must be a positive whole number of milliseconds."
                )),
                "{raw}"
            );
        }
    }
}
