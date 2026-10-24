//! The overlay's open and close rules, without any window: every opening is a new session in the
//! requested view, the page answers per session, and a closed overlay ignores everything but closing.

use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use crate::window::front_app::FrontApp;

const MIN_HEIGHT: f64 = 48.0;
const MAX_HEIGHT: f64 = 640.0;
/// Right after opening, macOS may briefly hand the keyboard to another window.
const FOCUS_SETTLE: Duration = Duration::from_millis(400);
/// One key press can arrive twice (a global shortcut and the page); the second is ignored.
const DUPLICATE_TOGGLE: Duration = Duration::from_millis(250);
/// Views that stay open while the owner looks elsewhere, on every Space, until closed: macOS can't
/// tell a Space switch from a click elsewhere, so they ignore both.
const STICKY_VIEWS: &[&str] = &["activity", "sample-sticky"];

/// What the overlay page renders.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct PanelSession {
    /// Counts up with every opening; the page answers with it.
    pub session: u32,
    pub view: String,
    #[ts(type = "unknown")]
    pub args: Value,
}

#[derive(Debug, PartialEq)]
pub enum OpenDecision {
    /// A repeated key press.
    Ignore,
    /// The same view is open already: the request toggles it closed.
    Close,
    /// Ask the page to render this session, then show it.
    Render(PanelSession),
}

#[derive(Debug, PartialEq)]
pub struct Showing {
    /// The overlay was already on screen with another session: no fade-in, no new place.
    pub was_open: bool,
    pub height: f64,
}

#[derive(Debug, PartialEq)]
pub enum Closing {
    /// The window is still hidden, in case macOS showed it anyway.
    WasClosed,
    Closed {
        target: Option<FrontApp>,
        opened_while_active: bool,
    },
}

#[derive(Debug, PartialEq, Eq)]
pub enum FocusLoss {
    Ignore,
    /// The shuffle right after opening: take the keyboard back.
    Refocus,
    /// A sticky view stays and follows the owner.
    StayOpen,
    /// Another app took focus; it stays in front.
    Close,
}

#[derive(Debug)]
pub struct PanelState {
    open: bool,
    session: Option<PanelSession>,
    counter: u32,
    opened_at: Option<Instant>,
    last_request: Option<Instant>,
    height: f64,
    target: Option<FrontApp>,
    opened_while_active: bool,
}

impl Default for PanelState {
    fn default() -> Self {
        Self {
            open: false,
            session: None,
            counter: 0,
            opened_at: None,
            last_request: None,
            height: 120.0,
            target: None,
            opened_while_active: false,
        }
    }
}

fn clamp_height(height: f64) -> f64 {
    height.clamp(MIN_HEIGHT, MAX_HEIGHT).round()
}

impl PanelState {
    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn session(&self) -> Option<&PanelSession> {
        self.session.as_ref()
    }

    pub fn is_current(&self, session: u32) -> bool {
        self.session.as_ref().is_some_and(|s| s.session == session)
    }

    pub fn is_sticky(&self) -> bool {
        self.session
            .as_ref()
            .is_some_and(|s| STICKY_VIEWS.contains(&s.view.as_str()))
    }

    pub fn height(&self) -> f64 {
        self.height
    }

    pub fn request(&mut self, view: &str, args: Value, now: Instant) -> OpenDecision {
        let repeated = self
            .last_request
            .is_some_and(|last| now.duration_since(last) < DUPLICATE_TOGGLE);
        self.last_request = Some(now);
        if repeated {
            return OpenDecision::Ignore;
        }
        if self.open
            && let Some(current) = &self.session
            && current.view == view
            && current.args == args
        {
            return OpenDecision::Close;
        }
        self.counter += 1;
        let session = PanelSession {
            session: self.counter,
            view: view.to_string(),
            args,
        };
        self.session = Some(session.clone());
        OpenDecision::Render(session)
    }

    /// Who gets the keyboard back on close; only taken while the overlay isn't open yet.
    pub fn remember(&mut self, target: Option<FrontApp>, daybook_active: bool) {
        if !self.open {
            self.target = target;
            self.opened_while_active = daybook_active;
        }
    }

    /// The page rendered `session` (at `height`, `None` when it didn't answer in time). `None` when a
    /// later request replaced the session meanwhile.
    pub fn show(&mut self, session: u32, height: Option<f64>, now: Instant) -> Option<Showing> {
        if !self.is_current(session) {
            return None;
        }
        if let Some(height) = height {
            self.height = clamp_height(height);
        }
        let was_open = self.open;
        self.open = true;
        self.opened_at = Some(now);
        Some(Showing {
            was_open,
            height: self.height,
        })
    }

    pub fn close(&mut self) -> Closing {
        let was_open = self.open;
        self.open = false;
        self.session = None;
        self.opened_at = None;
        if !was_open {
            return Closing::WasClosed;
        }
        Closing::Closed {
            target: self.target.take(),
            opened_while_active: std::mem::take(&mut self.opened_while_active),
        }
    }

    /// The page's new content height; `Some` when the open window has to change. A page still
    /// loading reports its height after closing: resizing then would put the window back on screen.
    pub fn set_height(&mut self, height: f64) -> Option<f64> {
        let height = clamp_height(height);
        if !self.open || height == self.height {
            return None;
        }
        self.height = height;
        Some(height)
    }

    /// While a sticky view waits unfocused, the app in front now replaces the remembered one, so
    /// closing hands focus to where the owner is.
    pub fn follow(&mut self, front: Option<FrontApp>) {
        if self.open && front.is_some() {
            self.target = front;
        }
    }

    pub fn lost_focus(&self, now: Instant) -> FocusLoss {
        if !self.open {
            return FocusLoss::Ignore;
        }
        if self.is_sticky() {
            return FocusLoss::StayOpen;
        }
        let settling = self
            .opened_at
            .is_some_and(|at| now.duration_since(at) < FOCUS_SETTLE);
        if settling {
            FocusLoss::Refocus
        } else {
            FocusLoss::Close
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn ms(base: Instant, millis: u64) -> Instant {
        base + Duration::from_millis(millis)
    }

    fn app(pid: i32) -> FrontApp {
        FrontApp {
            pid,
            bundle_id: format!("com.example.app{pid}"),
        }
    }

    fn session_id(decision: OpenDecision) -> u32 {
        match decision {
            OpenDecision::Render(session) => session.session,
            other => panic!("expected Render, got {other:?}"),
        }
    }

    /// An open panel showing `view` since `now`.
    fn open_panel(view: &str, now: Instant) -> PanelState {
        let mut state = PanelState::default();
        let id = session_id(state.request(view, Value::Null, now));
        state.show(id, Some(100.0), now).unwrap();
        state
    }

    #[test]
    fn requests_render_sessions_that_count_up() {
        let now = Instant::now();
        let mut state = PanelState::default();
        assert_eq!(
            state.request("sample", json!({ "a": 1 }), now),
            OpenDecision::Render(PanelSession {
                session: 1,
                view: "sample".to_string(),
                args: json!({ "a": 1 }),
            })
        );
        assert_eq!(state.session().map(|s| s.session), Some(1));
        assert_eq!(
            session_id(state.request("sample", Value::Null, ms(now, 1000))),
            2
        );
    }

    #[test]
    fn a_session_serializes_with_camel_case_fields() {
        let session = PanelSession {
            session: 3,
            view: "sample".to_string(),
            args: json!({ "k": "v" }),
        };
        assert_eq!(
            serde_json::to_value(&session).unwrap(),
            json!({ "session": 3, "view": "sample", "args": { "k": "v" } })
        );
    }

    #[test]
    fn a_second_request_within_250_ms_is_ignored() {
        let now = Instant::now();
        let mut state = PanelState::default();
        session_id(state.request("sample", Value::Null, now));
        assert_eq!(
            state.request("sample", Value::Null, ms(now, 249)),
            OpenDecision::Ignore
        );
        assert!(matches!(
            state.request("sample", Value::Null, ms(now, 249 + 250)),
            OpenDecision::Render(_)
        ));
    }

    #[test]
    fn the_same_view_and_args_while_open_close_the_panel() {
        let now = Instant::now();
        let mut state = open_panel("sample", now);
        assert_eq!(
            state.request("sample", Value::Null, ms(now, 1000)),
            OpenDecision::Close
        );
    }

    #[test]
    fn the_same_view_with_other_args_is_rendered_again() {
        let now = Instant::now();
        let mut state = open_panel("sample", now);
        assert_eq!(
            session_id(state.request("sample", json!({ "x": 1 }), ms(now, 1000))),
            2
        );
    }

    #[test]
    fn the_same_view_on_a_closed_panel_opens_it() {
        let now = Instant::now();
        let mut state = open_panel("sample", now);
        state.close();
        assert!(matches!(
            state.request("sample", Value::Null, ms(now, 1000)),
            OpenDecision::Render(_)
        ));
    }

    #[test]
    fn a_different_view_while_open_replaces_the_session() {
        let now = Instant::now();
        let mut state = open_panel("sample", now);
        let id = session_id(state.request("other", Value::Null, ms(now, 1000)));
        assert_eq!(id, 2);
        assert_eq!(
            state.show(id, None, ms(now, 1100)),
            Some(Showing {
                was_open: true,
                height: 100.0
            })
        );
    }

    #[test]
    fn showing_the_first_session_reports_it_was_not_open() {
        let now = Instant::now();
        let mut state = PanelState::default();
        let id = session_id(state.request("sample", Value::Null, now));
        assert_eq!(
            state.show(id, Some(200.0), now),
            Some(Showing {
                was_open: false,
                height: 200.0
            })
        );
        assert!(state.is_open());
    }

    #[test]
    fn showing_a_superseded_session_does_nothing() {
        let now = Instant::now();
        let mut state = PanelState::default();
        let first = session_id(state.request("sample", Value::Null, now));
        session_id(state.request("other", Value::Null, ms(now, 1000)));
        assert_eq!(state.show(first, Some(200.0), ms(now, 1100)), None);
        assert!(!state.is_open());
    }

    #[test]
    fn shown_heights_are_clamped_and_rounded() {
        let now = Instant::now();
        for (height, expected) in [
            (10.0, 48.0),
            (48.0, 48.0),
            (200.4, 200.0),
            (200.6, 201.0),
            (640.0, 640.0),
            (9000.0, 640.0),
        ] {
            let mut state = PanelState::default();
            let id = session_id(state.request("sample", Value::Null, now));
            assert_eq!(
                state.show(id, Some(height), now).map(|s| s.height),
                Some(expected),
                "{height}"
            );
        }
    }

    #[test]
    fn showing_without_a_height_keeps_the_last_one() {
        let now = Instant::now();
        let mut state = open_panel("sample", now);
        assert_eq!(state.height(), 100.0);
        let id = session_id(state.request("other", Value::Null, ms(now, 1000)));
        assert_eq!(
            state.show(id, None, ms(now, 1100)).map(|s| s.height),
            Some(100.0)
        );
    }

    #[test]
    fn closing_a_closed_panel_reports_it_was_closed() {
        let mut state = PanelState::default();
        assert_eq!(state.close(), Closing::WasClosed);
    }

    #[test]
    fn closing_an_open_panel_returns_the_remembered_target_and_resets() {
        let now = Instant::now();
        let mut state = PanelState::default();
        state.remember(Some(app(7)), true);
        let id = session_id(state.request("sample", Value::Null, now));
        state.show(id, None, now).unwrap();
        assert_eq!(
            state.close(),
            Closing::Closed {
                target: Some(app(7)),
                opened_while_active: true
            }
        );
        assert!(!state.is_open());
        assert!(state.session().is_none());
        assert_eq!(state.close(), Closing::WasClosed);
    }

    #[test]
    fn the_target_is_forgotten_after_closing() {
        let now = Instant::now();
        let mut state = PanelState::default();
        state.remember(Some(app(7)), true);
        let id = session_id(state.request("sample", Value::Null, now));
        state.show(id, None, now).unwrap();
        state.close();
        let id = session_id(state.request("sample", Value::Null, ms(now, 1000)));
        state.show(id, None, ms(now, 1000)).unwrap();
        assert_eq!(
            state.close(),
            Closing::Closed {
                target: None,
                opened_while_active: false
            }
        );
    }

    #[test]
    fn a_height_reported_after_closing_is_ignored() {
        let now = Instant::now();
        let mut state = open_panel("sample", now);
        state.close();
        assert_eq!(state.set_height(300.0), None);
    }

    #[test]
    fn set_height_reports_only_changes_while_open() {
        let now = Instant::now();
        let mut state = open_panel("sample", now);
        assert_eq!(state.set_height(100.0), None);
        assert_eq!(state.set_height(100.4), None);
        assert_eq!(state.set_height(300.4), Some(300.0));
        assert_eq!(state.set_height(5000.0), Some(640.0));
        assert_eq!(state.height(), 640.0);
    }

    #[test]
    fn remembering_is_ignored_while_open() {
        let now = Instant::now();
        let mut state = PanelState::default();
        state.remember(Some(app(1)), false);
        let id = session_id(state.request("sample", Value::Null, now));
        state.show(id, None, now).unwrap();
        state.remember(Some(app(2)), true);
        assert_eq!(
            state.close(),
            Closing::Closed {
                target: Some(app(1)),
                opened_while_active: false
            }
        );
    }

    #[test]
    fn following_replaces_the_target_only_while_open_and_only_with_an_app() {
        let now = Instant::now();
        let mut state = PanelState::default();
        state.remember(Some(app(1)), false);
        state.follow(Some(app(2)));
        let id = session_id(state.request("sample", Value::Null, now));
        state.show(id, None, now).unwrap();
        state.follow(None);
        assert_eq!(
            state.close(),
            Closing::Closed {
                target: Some(app(1)),
                opened_while_active: false
            }
        );

        let mut state = PanelState::default();
        state.remember(Some(app(1)), false);
        let id = session_id(state.request("sample", Value::Null, now));
        state.show(id, None, now).unwrap();
        state.follow(Some(app(2)));
        assert_eq!(
            state.close(),
            Closing::Closed {
                target: Some(app(2)),
                opened_while_active: false
            }
        );
    }

    #[test]
    fn losing_focus_on_a_closed_panel_is_ignored() {
        let state = PanelState::default();
        assert_eq!(state.lost_focus(Instant::now()), FocusLoss::Ignore);
    }

    #[test]
    fn losing_focus_right_after_showing_refocuses_and_later_closes() {
        let now = Instant::now();
        let state = open_panel("sample", now);
        for (millis, expected) in [
            (0, FocusLoss::Refocus),
            (399, FocusLoss::Refocus),
            (400, FocusLoss::Close),
            (5000, FocusLoss::Close),
        ] {
            assert_eq!(state.lost_focus(ms(now, millis)), expected, "{millis} ms");
        }
    }

    #[test]
    fn sticky_views_stay_open_when_focus_is_lost() {
        let now = Instant::now();
        for view in ["sample-sticky", "activity"] {
            let state = open_panel(view, now);
            assert!(state.is_sticky(), "{view}");
            for millis in [0, 5000] {
                assert_eq!(
                    state.lost_focus(ms(now, millis)),
                    FocusLoss::StayOpen,
                    "{view} at {millis} ms"
                );
            }
        }
    }

    #[test]
    fn an_ordinary_view_is_not_sticky() {
        let now = Instant::now();
        assert!(!open_panel("sample", now).is_sticky());
        assert!(!PanelState::default().is_sticky());
    }
}
