//! The app the owner works in, remembered right before an overlay opens, and whether focus goes back
//! to it when the overlay closes. Overlays never navigate: focus only goes back to an app on the
//! Space the owner looks at now, because activating an app switches to its Space.

/// Another app than Daybook.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontApp {
    pub pid: i32,
    pub bundle_id: String,
}

/// What closing an overlay does with the keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandBack {
    /// The app in front keeps it (Daybook isn't active, or something else took focus already).
    Nothing,
    /// Bring the remembered app forward; on failure hide Daybook if nothing else of it is shown.
    Activate,
    /// Hide Daybook, so macOS picks what comes forward on this Space.
    HideApp,
}

/// Read when an overlay closes, for `hand_back`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CloseContext {
    pub daybook_active: bool,
    /// Daybook was already active when the overlay opened; focus then stays with Daybook.
    pub opened_while_active: bool,
    /// Another app took focus while the overlay closed (a click elsewhere, a link opened).
    pub handed_off: bool,
    /// `None` without a remembered app, else whether it has a window on the current Space.
    pub target_on_current_space: Option<bool>,
    /// A regular Daybook window (the Daybook window, Settings) is visible.
    pub other_window_visible: bool,
}

pub fn hand_back(context: CloseContext) -> HandBack {
    if !context.daybook_active || context.opened_while_active || context.handed_off {
        return HandBack::Nothing;
    }
    match context.target_on_current_space {
        Some(true) => HandBack::Activate,
        // Bringing it forward would switch Spaces.
        Some(false) => HandBack::Nothing,
        None if context.other_window_visible => HandBack::Nothing,
        None => HandBack::HideApp,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active(target_on_current_space: Option<bool>) -> CloseContext {
        CloseContext {
            daybook_active: true,
            target_on_current_space,
            ..CloseContext::default()
        }
    }

    #[test]
    fn closing_hands_focus_back_only_when_it_is_safe() {
        let cases = [
            (
                "daybook not active",
                CloseContext {
                    daybook_active: false,
                    ..active(Some(true))
                },
                HandBack::Nothing,
            ),
            (
                "opened while daybook was active",
                CloseContext {
                    opened_while_active: true,
                    ..active(Some(true))
                },
                HandBack::Nothing,
            ),
            (
                "handed off",
                CloseContext {
                    handed_off: true,
                    ..active(Some(true))
                },
                HandBack::Nothing,
            ),
            (
                "remembered app on the current space",
                active(Some(true)),
                HandBack::Activate,
            ),
            (
                "remembered app on another space",
                active(Some(false)),
                HandBack::Nothing,
            ),
            ("no remembered app", active(None), HandBack::HideApp),
            (
                "no remembered app, another daybook window visible",
                CloseContext {
                    other_window_visible: true,
                    ..active(None)
                },
                HandBack::Nothing,
            ),
        ];
        for (name, context, expected) in cases {
            assert_eq!(hand_back(context), expected, "{name}");
        }
    }
}
