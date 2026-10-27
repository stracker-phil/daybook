//! The rail's state: shown or not, on which screen and edge, kept by the shortcut or a call. Every poll
//! and request answers with the window work to do (`Action`). Pure; points are y-down (`geometry`).

use serde::Serialize;
use ts_rs::TS;

use super::geometry::{Frame, Point, RailLook, Side, rail_frame, round, side_for};
use super::visibility::{
    IDLE, RailAction, RailDelays, RailStep, RailTimers, RailView, is_inside, next_rail_step,
};
use crate::store::settings::{Rail, RailCallEdge, RailCallPosition, RailEdges, RailPosition};

/// Until the page reports the bar's height.
pub const INITIAL_HEIGHT: f64 = 280.0;
const MIN_HEIGHT: f64 = 60.0;
const MAX_HEIGHT: f64 = 1200.0;
/// Around the rail the pointer still holds it.
const HOVER_SLOP: f64 = 16.0;
/// Near the rail the poll runs fast, so hover and tooltips follow the pointer.
const NEAR_SLOP: f64 = 48.0;

/// A screen in y-down points: its whole frame (to find it) and its work area (to place in).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub frame: Frame,
    pub visible: Frame,
}

/// Payload of `rail_shown` and the answer of `rail_shown_state`.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RailShown {
    pub visible: bool,
    pub side: Side,
    /// Kept by the shortcut or a call: it may fade back while the pointer is away.
    pub sticky: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// The rail window's frame.
    Place(Frame),
    /// Order the rail window front without activating Daybook.
    Show,
    /// Order the rail window out once the page slid the bar away.
    Hide,
    HideTip,
    Announce(RailShown),
    /// The pointer in window coordinates while it is on the rail, else `None`.
    Pointer(Option<Point>),
}

/// One poll: the pointer and its screen, and what holds the rail besides it.
#[derive(Clone, Copy, Debug)]
pub struct Poll {
    pub point: Point,
    pub screen: Screen,
    pub has_vault: bool,
    /// A call is detected.
    pub call_active: bool,
    /// The pointer keeps a tooltip's menu open.
    pub on_tip_menu: bool,
    /// Milliseconds, never 0.
    pub now: u64,
}

pub struct Controller {
    pub(super) settings: Rail,
    height: f64,
    pub(super) visible: bool,
    pub(super) on: Option<Screen>,
    /// The configured edge, or with both edges the one it opened at.
    pub(super) side: Side,
    /// Where the pointer was when it opened, for the "at the pointer" position.
    pub(super) pointer_y: Option<f64>,
    /// Placed for a call: on the call's edge and place.
    pub(super) call: bool,
    /// Shown by the shortcut: stays until the shortcut hides it, or the pointer takes it over.
    pub(super) pinned_by_user: bool,
    /// The pointer reached the edge of a rail the shortcut opened; it now hides like an edge-shown one.
    pub(super) handed_to_pointer: bool,
    /// The right-click menu is open.
    pub(super) menu_open: bool,
    call_active: bool,
    timers: RailTimers,
    announced: Option<RailShown>,
    pub(super) relayed: Option<Option<Point>>,
    fast: bool,
}

fn call_side(edge: RailCallEdge) -> Side {
    match edge {
        RailCallEdge::Left => Side::Left,
        RailCallEdge::Right => Side::Right,
    }
}

fn call_position(position: RailCallPosition) -> RailPosition {
    match position {
        RailCallPosition::Center => RailPosition::Center,
        RailCallPosition::Top => RailPosition::Top,
        RailCallPosition::Bottom => RailPosition::Bottom,
    }
}

impl Controller {
    pub fn new(settings: Rail) -> Self {
        Self {
            settings,
            height: INITIAL_HEIGHT,
            visible: false,
            on: None,
            side: Side::Right,
            pointer_y: None,
            call: false,
            pinned_by_user: false,
            handed_to_pointer: false,
            menu_open: false,
            call_active: false,
            timers: IDLE,
            announced: None,
            relayed: None,
            fast: false,
        }
    }

    pub fn settings(&self) -> &Rail {
        &self.settings
    }

    pub fn visible(&self) -> bool {
        self.visible
    }

    /// The next poll comes soon: the pointer is near the rail, at the edge or on a tooltip's menu.
    pub fn fast(&self) -> bool {
        self.fast
    }

    pub fn side(&self) -> Side {
        self.side
    }

    /// The screen the rail is on while it is shown.
    pub fn screen(&self) -> Option<Screen> {
        self.on.filter(|_| self.visible)
    }

    pub fn look(&self) -> RailLook {
        RailLook {
            shape: self.settings.shape,
            size: self.settings.size,
        }
    }

    pub fn shown(&self) -> RailShown {
        RailShown {
            visible: self.visible,
            side: self.side,
            sticky: self.is_pinned(),
        }
    }

    pub fn set_menu_open(&mut self, open: bool) {
        self.menu_open = open;
    }

    /// A vault is open and the rail is switched on or kept by the shortcut.
    pub fn allowed(&self, has_vault: bool) -> bool {
        has_vault && (self.settings.enabled || self.pinned_by_user || self.handed_to_pointer)
    }

    pub(super) fn in_call(&self) -> bool {
        self.settings.show_during_meetings && self.call_active
    }

    pub(super) fn is_pinned(&self) -> bool {
        self.pinned_by_user || self.in_call()
    }

    pub(super) fn frame_on(
        &self,
        screen: &Screen,
        side: Side,
        pointer_y: Option<f64>,
        call: bool,
    ) -> Frame {
        let position = if call {
            call_position(self.settings.call_position)
        } else {
            self.settings.position
        };
        rail_frame(
            screen.visible,
            self.look(),
            side,
            self.height,
            position,
            pointer_y,
        )
    }

    fn placed(&self) -> Option<Frame> {
        self.on
            .map(|on| self.frame_on(&on, self.side, self.pointer_y, self.call))
    }

    /// The rail window's frame while it is shown.
    pub fn frame(&self) -> Option<Frame> {
        self.placed().filter(|_| self.visible)
    }

    fn announce(&mut self, out: &mut Vec<Action>) {
        let shown = self.shown();
        if self.announced != Some(shown) {
            self.announced = Some(shown);
            out.push(Action::Announce(shown));
        }
    }

    pub fn hide(&mut self) -> Vec<Action> {
        self.pinned_by_user = false;
        self.handed_to_pointer = false;
        self.timers = IDLE;
        if !self.visible {
            return Vec::new();
        }
        self.visible = false;
        let mut out = vec![Action::HideTip];
        self.announce(&mut out);
        out.push(Action::Hide);
        out
    }

    fn show(&mut self, screen: Screen, side: Side, pointer_y: Option<f64>, out: &mut Vec<Action>) {
        if self.visible {
            return;
        }
        self.visible = true;
        self.on = Some(screen);
        self.call = self.in_call();
        self.side = if self.call {
            call_side(self.settings.call_edge)
        } else {
            side
        };
        self.pointer_y = pointer_y;
        self.timers = IDLE;
        out.extend(self.placed().map(Action::Place));
        out.push(Action::Show);
        self.announced = None;
        self.announce(out);
    }

    /// An open rail goes along to another screen or edge; during a call it keeps the call's edge.
    pub(super) fn move_to(
        &mut self,
        screen: Screen,
        side: Side,
        call: bool,
        out: &mut Vec<Action>,
    ) {
        self.on = Some(screen);
        self.call = call;
        self.side = if call {
            call_side(self.settings.call_edge)
        } else {
            side
        };
        out.push(Action::HideTip);
        out.extend(self.placed().map(Action::Place));
        self.announce(out);
    }

    /// Shows the rail on the pointer's screen and keeps it there.
    pub fn pin(&mut self, point: Point, screen: Screen, call_active: bool) -> Vec<Action> {
        self.call_active = call_active;
        self.pinned_by_user = true;
        self.handed_to_pointer = false;
        let mut out = Vec::new();
        if self.visible {
            // Kept now: the page may fade it back while the pointer is away.
            self.announce(&mut out);
        } else {
            let side = side_for(self.settings.edges, Some(point.x), screen.frame);
            self.show(screen, side, Some(point.y), &mut out);
        }
        out
    }

    /// The shortcut: hides a rail it keeps, else keeps the rail (also one the edge showed).
    pub fn toggle(&mut self, point: Point, screen: Screen, call_active: bool) -> Vec<Action> {
        if self.visible && self.pinned_by_user {
            self.hide()
        } else {
            self.pin(point, screen, call_active)
        }
    }

    pub fn poll(&mut self, poll: Poll) -> Vec<Action> {
        self.fast = false;
        self.call_active = poll.call_active;
        if !self.allowed(poll.has_vault) {
            return self.hide();
        }
        let Poll { point, screen, .. } = poll;
        let mut out = Vec::new();
        self.follow_call(point, screen, &mut out);
        self.follow_screen(point, screen, &mut out);
        self.relay(point, &mut out);
        let edge = self.trigger_edge(point, &screen);
        let at_own_edge = self.handoff_at_own_edge(point, screen);
        let frame = self.frame();
        self.fast = poll.on_tip_menu
            || edge.is_some()
            || frame.is_some_and(|frame| is_inside(point, frame, NEAR_SLOP));
        let near_rail = frame.is_some_and(|frame| is_inside(point, frame, HOVER_SLOP));
        let view = self.view(edge.is_some(), near_rail || at_own_edge || poll.on_tip_menu);
        let delays = RailDelays {
            show: u64::from(self.settings.show_delay),
            hide: u64::from(self.settings.hide_delay),
        };
        let step = next_rail_step(view, self.timers, poll.now, delays);
        self.apply_step(step, edge, point, screen, &mut out);
        self.announce(&mut out);
        out
    }

    /// `held`: the pointer is on or beside the rail or on a tooltip's menu; the right-click menu holds too.
    fn view(&self, at_edge: bool, held: bool) -> RailView {
        RailView {
            allowed: true,
            enabled: self.settings.enabled,
            pinned: self.is_pinned(),
            visible: self.visible,
            at_edge,
            held: held || self.menu_open,
        }
    }

    fn apply_step(
        &mut self,
        step: RailStep,
        edge: Option<Side>,
        point: Point,
        screen: Screen,
        out: &mut Vec<Action>,
    ) {
        self.timers = step.timers;
        match step.action {
            RailAction::Show => {
                let side = edge
                    .unwrap_or_else(|| side_for(self.settings.edges, Some(point.x), screen.frame));
                self.show(screen, side, Some(point.y), out);
            }
            RailAction::Hide => out.extend(self.hide()),
            RailAction::Nothing => {}
        }
    }

    /// The bar's height as the page measured it, rounded into 60 to 1200 points.
    pub fn set_height(&mut self, height: f64) -> Vec<Action> {
        if !height.is_finite() {
            return Vec::new();
        }
        let next = round(height.clamp(MIN_HEIGHT, MAX_HEIGHT));
        if next == self.height {
            return Vec::new();
        }
        self.height = next;
        self.placed().map(Action::Place).into_iter().collect()
    }

    pub fn apply_settings(&mut self, next: Rail) -> Vec<Action> {
        let previous = std::mem::replace(&mut self.settings, next);
        let next = &self.settings;
        let off_edge =
            !self.call && next.edges != RailEdges::Both && next.edges != self.side.as_edges();
        if (!next.enabled && !self.pinned_by_user) || off_edge {
            return self.hide();
        }
        let moved = previous.shape != next.shape
            || previous.size != next.size
            || previous.position != next.position
            || previous.call_edge != next.call_edge
            || previous.call_position != next.call_position;
        let mut out = Vec::new();
        if moved && let Some(on) = self.on {
            self.move_to(on, self.side, self.call, &mut out);
        }
        self.announce(&mut out);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::{RailShape, RailSpan};

    const A: Screen = Screen {
        frame: Frame::new(0.0, 0.0, 1512.0, 982.0),
        visible: Frame::new(0.0, 25.0, 1512.0, 920.0),
    };
    const B: Screen = Screen {
        frame: Frame::new(1512.0, 0.0, 1512.0, 982.0),
        visible: Frame::new(1512.0, 25.0, 1512.0, 920.0),
    };
    const RIGHT_FRAME: Frame = Frame::new(1462.0, 345.0, 52.0, 280.0);
    const CALL_FRAME: Frame = Frame::new(-2.0, 33.0, 52.0, 280.0);

    fn rail(change: impl FnOnce(&mut Rail)) -> Rail {
        let mut settings = Rail::default();
        change(&mut settings);
        settings
    }

    fn call_settings() -> Rail {
        rail(|r| {
            r.call_edge = RailCallEdge::Left;
            r.call_position = RailCallPosition::Top;
        })
    }

    fn point(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    fn at(x: f64, y: f64, screen: Screen, now: u64) -> Poll {
        Poll {
            point: point(x, y),
            screen,
            has_vault: true,
            call_active: false,
            on_tip_menu: false,
            now,
        }
    }

    fn shown(visible: bool, side: Side, sticky: bool) -> RailShown {
        RailShown {
            visible,
            side,
            sticky,
        }
    }

    /// Shown by resting the pointer at the right edge of `A`.
    fn edge_shown(settings: Rail) -> Controller {
        let mut controller = Controller::new(settings);
        controller.poll(at(1511.0, 400.0, A, 1000));
        controller.poll(at(1511.0, 400.0, A, 1200));
        assert!(controller.visible());
        controller
    }

    /// Shown by the shortcut with the pointer in the middle of `A`.
    fn pinned(settings: Rail) -> Controller {
        let mut controller = Controller::new(settings);
        controller.toggle(point(700.0, 400.0), A, false);
        assert!(controller.visible());
        controller
    }

    #[test]
    fn the_shortcut_shows_the_rail_kept_on_the_pointers_screen() {
        let mut controller = Controller::new(Rail::default());
        assert_eq!(
            controller.toggle(point(700.0, 400.0), A, false),
            vec![
                Action::Place(RIGHT_FRAME),
                Action::Show,
                Action::Announce(shown(true, Side::Right, true)),
            ]
        );
        assert_eq!(controller.screen(), Some(A));
        assert_eq!(controller.frame(), Some(RIGHT_FRAME));
    }

    #[test]
    fn the_shortcut_hides_a_rail_it_kept() {
        let mut controller = pinned(Rail::default());
        assert_eq!(
            controller.toggle(point(700.0, 400.0), A, false),
            vec![
                Action::HideTip,
                Action::Announce(shown(false, Side::Right, false)),
                Action::Hide,
            ]
        );
        assert!(!controller.visible());
    }

    #[test]
    fn the_shortcut_keeps_a_rail_the_edge_showed_instead_of_hiding_it() {
        let mut controller = edge_shown(Rail::default());
        assert_eq!(
            controller.toggle(point(700.0, 400.0), A, false),
            vec![Action::Announce(shown(true, Side::Right, true))]
        );
        assert!(controller.visible());
    }

    #[test]
    fn keeping_an_already_kept_rail_says_nothing_new() {
        let mut controller = pinned(Rail::default());
        assert_eq!(controller.pin(point(700.0, 400.0), A, false), vec![]);
    }

    #[test]
    fn with_both_edges_the_shortcut_opens_on_the_pointers_side() {
        let settings = rail(|r| r.edges = RailEdges::Both);
        for (x, side) in [(200.0, Side::Left), (1300.0, Side::Right)] {
            let mut controller = Controller::new(settings.clone());
            controller.toggle(point(x, 400.0), A, false);
            assert_eq!(controller.side(), side, "{x}");
        }
    }

    #[test]
    fn a_kept_rail_stays_while_the_pointer_is_away() {
        let mut controller = pinned(Rail::default());
        controller.poll(at(700.0, 400.0, A, 1000));
        controller.poll(at(700.0, 400.0, A, 9000));
        assert!(controller.visible());
    }

    #[test]
    fn pointing_at_its_edge_hands_a_kept_rail_to_the_pointer() {
        let mut controller = pinned(Rail::default());
        assert_eq!(
            controller.poll(at(1511.0, 400.0, A, 1000)),
            vec![
                Action::Pointer(Some(point(49.0, 55.0))),
                Action::Announce(shown(true, Side::Right, false)),
            ]
        );
        controller.poll(at(700.0, 400.0, A, 2000));
        controller.poll(at(700.0, 400.0, A, 2699));
        assert!(controller.visible());
        assert_eq!(
            controller.poll(at(700.0, 400.0, A, 2700)),
            vec![
                Action::HideTip,
                Action::Announce(shown(false, Side::Right, false)),
                Action::Hide,
            ]
        );
    }

    #[test]
    fn a_kept_rail_stays_with_the_edge_switched_off_until_the_pointer_takes_it_over() {
        let mut controller = pinned(rail(|r| r.enabled = false));
        controller.poll(at(700.0, 400.0, A, 1000));
        controller.poll(at(700.0, 400.0, A, 9000));
        assert!(controller.visible());
        controller.poll(at(1511.0, 400.0, A, 9100));
        assert!(controller.visible());
        controller.poll(at(700.0, 400.0, A, 10000));
        controller.poll(at(700.0, 400.0, A, 10700));
        assert!(!controller.visible());
    }

    #[test]
    fn resting_at_the_edge_shows_the_rail_after_the_show_delay() {
        let mut controller = Controller::new(Rail::default());
        controller.poll(at(1511.0, 400.0, A, 1000));
        controller.poll(at(1511.0, 400.0, A, 1199));
        assert!(!controller.visible());
        assert_eq!(
            controller.poll(at(1511.0, 400.0, A, 1200)),
            vec![
                Action::Place(RIGHT_FRAME),
                Action::Show,
                Action::Announce(shown(true, Side::Right, false)),
            ]
        );
    }

    #[test]
    fn leaving_the_edge_starts_the_show_delay_over() {
        let mut controller = Controller::new(Rail::default());
        controller.poll(at(1511.0, 400.0, A, 1000));
        controller.poll(at(700.0, 400.0, A, 1100));
        controller.poll(at(1511.0, 400.0, A, 1200));
        controller.poll(at(1511.0, 400.0, A, 1300));
        assert!(!controller.visible());
        controller.poll(at(1511.0, 400.0, A, 1400));
        assert!(controller.visible());
    }

    #[test]
    fn the_edge_shows_the_rail_per_edges_trigger_width_and_switch() {
        let cases = [
            (Rail::default(), (1511.0, 400.0), Some(Side::Right)),
            (Rail::default(), (1500.0, 400.0), None),
            (
                rail(|r| r.trigger_width = 16),
                (1500.0, 400.0),
                Some(Side::Right),
            ),
            (rail(|r| r.edges = RailEdges::Left), (1511.0, 400.0), None),
            (
                rail(|r| r.edges = RailEdges::Left),
                (0.0, 400.0),
                Some(Side::Left),
            ),
            (
                rail(|r| r.edges = RailEdges::Both),
                (0.0, 400.0),
                Some(Side::Left),
            ),
            (
                rail(|r| r.edges = RailEdges::Both),
                (1511.0, 400.0),
                Some(Side::Right),
            ),
            (rail(|r| r.enabled = false), (1511.0, 400.0), None),
        ];
        for (settings, (x, y), expected) in cases {
            let label = format!("{settings:?} at {x},{y}");
            let mut controller = Controller::new(settings);
            controller.poll(at(x, y, A, 1000));
            controller.poll(at(x, y, A, 1200));
            let side = controller.visible().then(|| controller.side());
            assert_eq!(side, expected, "{label}");
        }
    }

    #[test]
    fn no_vault_keeps_the_rail_away_and_hides_an_open_one() {
        let mut hidden = Controller::new(Rail::default());
        let no_vault = |poll: Poll| Poll {
            has_vault: false,
            ..poll
        };
        assert_eq!(hidden.poll(no_vault(at(1511.0, 400.0, A, 1000))), vec![]);

        let mut open = edge_shown(Rail::default());
        assert_eq!(
            open.poll(no_vault(at(1511.0, 400.0, A, 2000))),
            vec![
                Action::HideTip,
                Action::Announce(shown(false, Side::Right, false)),
                Action::Hide,
            ]
        );
    }

    #[test]
    fn an_edge_shown_rail_hides_once_the_pointer_stayed_away_for_the_hide_delay() {
        let mut controller = edge_shown(Rail::default());
        controller.poll(at(700.0, 400.0, A, 2000));
        controller.poll(at(700.0, 400.0, A, 2699));
        assert!(controller.visible());
        assert_eq!(
            controller.poll(at(700.0, 400.0, A, 2700)),
            vec![
                Action::HideTip,
                Action::Announce(shown(false, Side::Right, false)),
                Action::Hide,
            ]
        );
    }

    #[test]
    fn the_pointer_near_the_rail_holds_it_open() {
        let mut controller = edge_shown(Rail::default());
        controller.poll(at(1450.0, 400.0, A, 2000));
        controller.poll(at(1450.0, 400.0, A, 9000));
        assert!(controller.visible());
    }

    #[test]
    fn an_open_context_menu_holds_the_rail_open() {
        let mut controller = edge_shown(Rail::default());
        controller.set_menu_open(true);
        controller.poll(at(700.0, 400.0, A, 2000));
        controller.poll(at(700.0, 400.0, A, 9000));
        assert!(controller.visible());
        controller.set_menu_open(false);
        controller.poll(at(700.0, 400.0, A, 9500));
        controller.poll(at(700.0, 400.0, A, 10200));
        assert!(!controller.visible());
    }

    #[test]
    fn a_tooltips_menu_under_the_pointer_holds_the_rail_open() {
        let mut controller = edge_shown(Rail::default());
        for now in [2000, 9000] {
            controller.poll(Poll {
                on_tip_menu: true,
                ..at(700.0, 400.0, A, now)
            });
        }
        assert!(controller.visible());
        assert!(controller.fast());
    }

    #[test]
    fn the_poll_is_fast_only_near_the_open_rail() {
        let mut controller = edge_shown(Rail::default());
        controller.poll(at(1420.0, 400.0, A, 2000));
        assert!(controller.fast());
        controller.poll(at(1000.0, 400.0, A, 2100));
        assert!(!controller.fast());
    }

    #[test]
    fn the_pointer_on_the_rail_is_relayed_in_rail_coordinates_when_it_changes() {
        let mut controller = edge_shown(Rail::default());
        assert_eq!(
            controller.poll(at(1480.0, 400.0, A, 3000)),
            vec![Action::Pointer(Some(point(18.0, 55.0)))]
        );
        assert_eq!(controller.poll(at(1480.0, 400.0, A, 3100)), vec![]);
        assert_eq!(
            controller.poll(at(1481.0, 400.0, A, 3200)),
            vec![Action::Pointer(Some(point(19.0, 55.0)))]
        );
        assert_eq!(
            controller.poll(at(1440.0, 400.0, A, 3300)),
            vec![Action::Pointer(None)]
        );
    }

    #[test]
    fn a_trigger_limited_to_the_rail_ignores_the_rest_of_the_edge() {
        let mut controller = Controller::new(rail(|r| r.trigger_span = RailSpan::Rail));
        controller.poll(at(1511.0, 50.0, A, 1000));
        controller.poll(at(1511.0, 50.0, A, 1200));
        assert!(!controller.visible());
        controller.poll(at(1511.0, 400.0, A, 1300));
        controller.poll(at(1511.0, 400.0, A, 1500));
        assert!(controller.visible());
    }

    #[test]
    fn a_rail_placed_at_the_pointer_is_triggered_along_the_whole_edge() {
        let mut controller = Controller::new(rail(|r| {
            r.trigger_span = RailSpan::Rail;
            r.position = RailPosition::Pointer;
        }));
        controller.poll(at(1511.0, 50.0, A, 1000));
        controller.poll(at(1511.0, 50.0, A, 1200));
        assert_eq!(
            controller.frame(),
            Some(Frame::new(1462.0, 33.0, 52.0, 280.0))
        );
    }

    #[test]
    fn a_call_shows_the_rail_by_itself_on_the_calls_edge_and_place() {
        let mut controller = Controller::new(call_settings());
        assert_eq!(
            controller.poll(Poll {
                call_active: true,
                ..at(700.0, 400.0, A, 1000)
            }),
            vec![
                Action::Pointer(None),
                Action::Place(CALL_FRAME),
                Action::Show,
                Action::Announce(shown(true, Side::Left, true)),
            ]
        );
    }

    #[test]
    fn a_call_is_ignored_when_the_rail_isnt_kept_during_meetings() {
        let mut controller = Controller::new(rail(|r| r.show_during_meetings = false));
        controller.poll(Poll {
            call_active: true,
            ..at(700.0, 400.0, A, 1000)
        });
        assert!(!controller.visible());
    }

    #[test]
    fn the_shortcut_during_a_call_places_the_rail_for_the_call() {
        let mut controller = Controller::new(call_settings());
        assert_eq!(
            controller.toggle(point(700.0, 400.0), A, true),
            vec![
                Action::Place(CALL_FRAME),
                Action::Show,
                Action::Announce(shown(true, Side::Left, true)),
            ]
        );
    }

    #[test]
    fn a_call_starting_takes_an_open_rail_to_the_calls_place() {
        let mut controller = edge_shown(call_settings());
        assert_eq!(
            controller.poll(Poll {
                call_active: true,
                ..at(1511.0, 400.0, A, 2000)
            }),
            vec![
                Action::HideTip,
                Action::Place(CALL_FRAME),
                Action::Announce(shown(true, Side::Left, true)),
            ]
        );
    }

    #[test]
    fn after_a_call_a_kept_rail_returns_to_the_pointer() {
        let mut controller = pinned(call_settings());
        controller.poll(Poll {
            call_active: true,
            ..at(700.0, 400.0, A, 1000)
        });
        assert_eq!(controller.side(), Side::Left);
        assert_eq!(
            controller.poll(at(1000.0, 500.0, A, 1100)),
            vec![
                Action::HideTip,
                Action::Place(RIGHT_FRAME),
                Action::Announce(shown(true, Side::Right, true)),
            ]
        );
    }

    #[test]
    fn a_kept_rail_follows_the_pointer_to_another_screen() {
        let mut controller = pinned(Rail::default());
        controller.poll(at(700.0, 400.0, A, 1000));
        assert_eq!(
            controller.poll(at(2000.0, 400.0, B, 1100)),
            vec![
                Action::HideTip,
                Action::Place(Frame::new(2974.0, 345.0, 52.0, 280.0)),
            ]
        );
        assert_eq!(controller.screen(), Some(B));
    }

    #[test]
    fn with_both_edges_a_followed_rail_takes_the_side_the_pointer_is_on() {
        let mut controller = pinned_at(rail(|r| r.edges = RailEdges::Both), 1400.0);
        assert_eq!(controller.side(), Side::Right);
        controller.poll(at(1400.0, 400.0, A, 1000));
        assert_eq!(
            controller.poll(at(1600.0, 400.0, B, 1100)),
            vec![
                Action::HideTip,
                Action::Place(Frame::new(1510.0, 345.0, 52.0, 280.0)),
                Action::Announce(shown(true, Side::Left, true)),
            ]
        );
    }

    fn pinned_at(settings: Rail, x: f64) -> Controller {
        let mut controller = Controller::new(settings);
        controller.toggle(point(x, 400.0), A, false);
        controller
    }

    #[test]
    fn an_edge_shown_rail_stays_on_its_screen() {
        let mut controller = edge_shown(Rail::default());
        controller.poll(at(2000.0, 400.0, B, 2000));
        assert_eq!(controller.screen(), Some(A));
    }

    #[test]
    fn the_bars_height_is_rounded_and_kept_within_60_to_1200_points() {
        for (height, frame) in [
            (400.4, Frame::new(1462.0, 285.0, 52.0, 400.0)),
            (300.5, Frame::new(1462.0, 335.0, 52.0, 301.0)),
            (10.0, Frame::new(1462.0, 455.0, 52.0, 60.0)),
            (5000.0, Frame::new(1462.0, -115.0, 52.0, 1200.0)),
        ] {
            let mut controller = pinned(Rail::default());
            assert_eq!(
                controller.set_height(height),
                vec![Action::Place(frame)],
                "{height}"
            );
        }
    }

    #[test]
    fn an_unchanged_or_unreadable_height_places_nothing() {
        let mut controller = pinned(Rail::default());
        assert_eq!(controller.set_height(280.0), vec![]);
        assert_eq!(controller.set_height(f64::NAN), vec![]);
        assert_eq!(controller.set_height(f64::INFINITY), vec![]);
        controller.set_height(400.0);
        assert_eq!(controller.set_height(400.4), vec![]);
    }

    #[test]
    fn a_height_before_the_rail_was_ever_shown_places_nothing() {
        assert_eq!(Controller::new(Rail::default()).set_height(400.0), vec![]);
    }

    #[test]
    fn moving_the_rail_off_the_shown_edge_in_the_settings_hides_it() {
        let hidden = vec![
            Action::HideTip,
            Action::Announce(shown(false, Side::Right, false)),
            Action::Hide,
        ];
        let mut edge = edge_shown(Rail::default());
        assert_eq!(
            edge.apply_settings(rail(|r| r.edges = RailEdges::Left)),
            hidden
        );
        let mut kept = pinned(Rail::default());
        assert_eq!(
            kept.apply_settings(rail(|r| r.edges = RailEdges::Left)),
            hidden
        );
    }

    #[test]
    fn both_edges_keep_a_shown_rail_where_it_is() {
        let mut controller = edge_shown(Rail::default());
        assert_eq!(
            controller.apply_settings(rail(|r| r.edges = RailEdges::Both)),
            vec![]
        );
    }

    #[test]
    fn switching_the_edge_off_hides_a_rail_the_edge_showed_but_not_a_kept_one() {
        let mut edge = edge_shown(Rail::default());
        edge.apply_settings(rail(|r| r.enabled = false));
        assert!(!edge.visible());

        let mut kept = pinned(Rail::default());
        assert_eq!(kept.apply_settings(rail(|r| r.enabled = false)), vec![]);
        assert!(kept.visible());
    }

    #[test]
    fn a_new_look_or_place_moves_the_open_rail() {
        let mut controller = edge_shown(Rail::default());
        assert_eq!(
            controller.apply_settings(rail(|r| r.shape = RailShape::Docked)),
            vec![
                Action::HideTip,
                Action::Place(Frame::new(1466.0, 345.0, 46.0, 280.0)),
            ]
        );
    }

    #[test]
    fn settings_that_dont_affect_the_place_leave_the_open_rail_alone() {
        let mut controller = edge_shown(Rail::default());
        assert_eq!(
            controller.apply_settings(rail(|r| {
                r.captions = false;
                r.hide_delay = 1500;
            })),
            vec![]
        );
    }

    #[test]
    fn a_rail_placed_for_a_call_ignores_the_edges_setting_and_follows_the_calls_place() {
        let mut controller = Controller::new(call_settings());
        controller.toggle(point(700.0, 400.0), A, true);
        let mut next = call_settings();
        next.call_position = RailCallPosition::Bottom;
        assert_eq!(
            controller.apply_settings(next),
            vec![
                Action::HideTip,
                Action::Place(Frame::new(-2.0, 657.0, 52.0, 280.0)),
            ]
        );
    }
}
