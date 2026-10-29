//! The app the owner works in, remembered right before an overlay opens, and whether focus goes back
//! to it when the overlay closes. Overlays never navigate: focus only goes back to an app on the
//! Space the owner looks at now, because activating an app switches to its Space.

/// Another app than Daybook.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FrontApp {
    pub pid: i32,
    pub bundle_id: String,
}

/// A window in the on-screen window list, which runs front to back over every screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShownWindow {
    pub pid: i32,
    pub layer: i64,
    pub alpha: f64,
}

impl ShownWindow {
    /// A normal window one can see; menus, panels and the menu bar sit above layer 0.
    fn is_normal(&self) -> bool {
        self.layer == 0 && self.alpha > 0.0
    }
}

/// The app with the frontmost normal window other than Daybook's. Minimized and hidden windows aren't
/// on screen, so their apps never come back this way; a full-screen app on another screen does.
pub fn topmost_app(windows: &[ShownWindow], daybook_pid: i32) -> Option<i32> {
    windows
        .iter()
        .find(|window| window.is_normal() && window.pid != daybook_pid)
        .map(|window| window.pid)
}

/// The app has a normal window on the current Space of any screen.
pub fn has_shown_window(windows: &[ShownWindow], pid: i32) -> bool {
    windows
        .iter()
        .any(|window| window.is_normal() && window.pid == pid)
}

/// What closing an overlay does with the keyboard focus.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HandBack {
    /// The app in front keeps it (Daybook isn't active, or something else took focus already).
    Nothing,
    /// Bring the remembered app forward; on failure focus the topmost app if no Daybook window is shown.
    Activate,
    /// Hand focus to the app with the topmost visible window (`topmost_app`).
    FocusTopmost,
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
        None => HandBack::FocusTopmost,
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
            ("no remembered app", active(None), HandBack::FocusTopmost),
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

    const DAYBOOK: i32 = 100;

    fn window(pid: i32, layer: i64, alpha: f64) -> ShownWindow {
        ShownWindow { pid, layer, alpha }
    }

    #[test]
    fn the_topmost_app_is_the_first_normal_window_other_than_daybooks() {
        let cases = [
            ("empty list", vec![], None),
            (
                "list order decides",
                vec![window(1, 0, 1.0), window(2, 0, 1.0)],
                Some(1),
            ),
            (
                "daybook in front is skipped",
                vec![window(DAYBOOK, 0, 1.0), window(2, 0, 1.0)],
                Some(2),
            ),
            (
                "menus and panels above layer 0 are skipped",
                vec![window(5, 25, 1.0), window(6, 24, 1.0), window(3, 0, 1.0)],
                Some(3),
            ),
            (
                "transparent windows are skipped",
                vec![window(4, 0, 0.0), window(3, 0, 0.5)],
                Some(3),
            ),
            (
                "only daybook and skipped windows",
                vec![
                    window(DAYBOOK, 0, 1.0),
                    window(5, 25, 1.0),
                    window(4, 0, 0.0),
                ],
                None,
            ),
        ];
        for (name, windows, expected) in cases {
            assert_eq!(topmost_app(&windows, DAYBOOK), expected, "{name}");
        }
    }

    #[test]
    fn an_app_has_a_shown_window_only_when_it_has_a_normal_one() {
        let cases = [
            (
                "normal window anywhere in the list",
                vec![window(9, 0, 1.0), window(1, 0, 1.0)],
                true,
            ),
            (
                "only a window above layer 0",
                vec![window(1, 25, 1.0), window(9, 0, 1.0)],
                false,
            ),
            ("only a transparent window", vec![window(1, 0, 0.0)], false),
            ("no windows", vec![window(9, 0, 1.0)], false),
            ("empty list", vec![], false),
        ];
        for (name, windows, expected) in cases {
            assert_eq!(has_shown_window(&windows, 1), expected, "{name}");
        }
    }
}
