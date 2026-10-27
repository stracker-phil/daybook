//! The tooltip beside the rail: one card in a click-through column window as tall as the screen, so the
//! card glides from button to button. A tool with choices lists them in the card; the card then takes
//! clicks while the pointer is on it. Pure; points are y-down.

use serde::Serialize;
use ts_rs::TS;

use super::geometry::{Frame, Point, Side, round};
use super::visibility::is_inside;
use crate::store::settings::{RailHighlight, RailTheme, ToolId};
use crate::tools::{Choice, choice_commands};

/// The card's widest; a plain tooltip is as wide as its text, one with choices always this wide.
const CARD_WIDTH: f64 = 264.0;
/// Between the bar and the card.
const GAP: f64 = 6.0;
/// Room for the card's shadow on the side away from the rail.
const OUTER: f64 = 14.0;
/// Around the card the pointer still counts as on it (it has to cross the gap from the bar).
const HOLD_SLOP: f64 = 10.0;
/// How long the pointer may be off both the button and the card before the card closes.
const LEAVE_MS: u64 = 350;
/// Picks that keep the card open, updated; any other pick opens something, so the card goes.
const STAYS_OPEN: [&str; 2] = ["mode:", "music:"];

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct RailTip {
    pub label: String,
    pub detail: Option<String>,
    /// In Settings notation (`ctrl+alt+cmd+f`).
    pub shortcut: Option<String>,
}

/// Payload of `rail_tip_content`.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct RailTipContent {
    pub tip: RailTip,
    pub side: Side,
    pub theme: RailTheme,
    pub highlight: RailHighlight,
    /// Reported back with `rail_tip_rendered`; matches `rail_tip_placed` and `rail_tip_hidden`.
    pub sequence: u32,
    /// Empty for a plain tooltip.
    pub choices: Vec<Choice>,
    /// The hovered button's centre, from the window's top.
    pub anchor: f64,
    /// Between the card and the window's rail side.
    pub inset: f64,
    pub max_width: f64,
    /// The window's height.
    pub height: f64,
}

/// Where the card goes: beside the bar's inner edge, level with the hovered button.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    /// The bar's inner edge.
    pub x: f64,
    /// The hovered button's centre.
    pub y: f64,
    pub side: Side,
    pub work_area: Frame,
    pub theme: RailTheme,
    pub highlight: RailHighlight,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TipAction {
    /// The window's frame.
    Column(Frame),
    /// Order the window front without activating Daybook, then set its frame again: a window's first
    /// show can restore an older frame.
    Show(Frame),
    /// Order the window out once the page faded the card.
    HideLater,
    /// The window takes clicks, or lets them through.
    Clickable(bool),
    Content(RailTipContent),
    Placed(u32),
    Hidden(u32),
    /// The pointer in window coordinates while it is on the card's choices, else `None`.
    Pointer(Option<Point>),
}

#[derive(Clone, Debug)]
struct Target {
    placement: Placement,
    tip: RailTip,
    menu: Option<ToolId>,
    sequence: u32,
    with_choices: bool,
}

#[derive(Default)]
pub struct TipState {
    column: Option<Frame>,
    visible: bool,
    sequence: u32,
    target: Option<Target>,
    /// Where the card with choices is; `None` for a plain tooltip.
    menu_frame: Option<Frame>,
    /// The rail's button lost the pointer; the card goes once the pointer is off it too.
    leaving: bool,
    outside_since: u64,
    relayed: Option<Option<Point>>,
    clickable: bool,
    /// The commands of the choices on show; nothing else runs from the card.
    offered: Vec<String>,
}

/// The column touches the bar's inner edge and spans the screen's work area.
pub fn column_for(placement: &Placement) -> Frame {
    let width = CARD_WIDTH + GAP + OUTER;
    let x = match placement.side {
        Side::Right => placement.x - width,
        Side::Left => placement.x,
    };
    Frame::new(
        round(x),
        round(placement.work_area.y),
        width,
        round(placement.work_area.height),
    )
}

pub fn stays_open(command: &str) -> bool {
    STAYS_OPEN.iter().any(|prefix| command.starts_with(prefix))
}

impl TipState {
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// The tool whose choices the card shows or is about to show.
    pub fn menu(&self) -> Option<ToolId> {
        self.target.as_ref().and_then(|target| target.menu)
    }

    /// The tool whose choices are on screen, to follow state changes.
    pub fn open_menu(&self) -> Option<ToolId> {
        self.menu().filter(|_| self.visible)
    }

    fn set_clickable(&mut self, clickable: bool, out: &mut Vec<TipAction>) {
        if self.clickable != clickable {
            self.clickable = clickable;
            out.push(TipAction::Clickable(clickable));
        }
    }

    /// A new card; `content` follows with its choices. The window is revealed once the page placed it.
    pub fn show(
        &mut self,
        tip: RailTip,
        menu: Option<ToolId>,
        placement: Placement,
    ) -> Vec<TipAction> {
        self.sequence += 1;
        self.target = Some(Target {
            placement,
            tip,
            menu,
            sequence: self.sequence,
            with_choices: false,
        });
        self.leaving = false;
        self.outside_since = 0;
        let column = column_for(&placement);
        if self.column == Some(column) {
            return Vec::new();
        }
        self.column = Some(column);
        vec![TipAction::Column(column)]
    }

    /// What the page renders for the current card, with `choices` of its tool.
    pub fn content(&mut self, choices: Vec<Choice>) -> Option<TipAction> {
        let column = self.column?;
        let target = self.target.as_mut()?;
        self.offered = choice_commands(&choices);
        target.with_choices = !choices.is_empty();
        Some(TipAction::Content(RailTipContent {
            tip: target.tip.clone(),
            side: target.placement.side,
            theme: target.placement.theme,
            highlight: target.placement.highlight,
            sequence: target.sequence,
            choices,
            anchor: target.placement.y - column.y,
            inset: GAP,
            max_width: CARD_WIDTH,
            height: column.height,
        }))
    }

    /// The page drew card `sequence` at `card` (window coordinates); reveals a hidden window.
    pub fn rendered(&mut self, sequence: u32, card: Frame) -> Vec<TipAction> {
        let (Some(column), Some(target)) = (self.column, self.target.as_ref()) else {
            return Vec::new();
        };
        if target.sequence != sequence {
            return Vec::new();
        }
        let frame = Frame::new(
            column.x + card.x,
            column.y + card.y,
            card.width,
            card.height,
        );
        self.menu_frame = target.with_choices.then_some(frame);
        let mut out = Vec::new();
        if !self.visible {
            self.visible = true;
            out.push(TipAction::Show(column));
        }
        out.push(TipAction::Placed(sequence));
        out
    }

    pub fn hide(&mut self) -> Vec<TipAction> {
        self.sequence += 1;
        self.target = None;
        self.menu_frame = None;
        self.leaving = false;
        self.offered.clear();
        let mut out = Vec::new();
        self.set_clickable(false, &mut out);
        if self.visible {
            self.visible = false;
            out.push(TipAction::Hidden(self.sequence));
            out.push(TipAction::HideLater);
        }
        out
    }

    /// The rail's button lost the pointer: a plain tooltip goes at once, a card with choices waits for
    /// the pointer to leave it too.
    pub fn leave(&mut self) -> Vec<TipAction> {
        if self.visible && self.menu_frame.is_some() {
            self.leaving = true;
            Vec::new()
        } else {
            self.hide()
        }
    }

    /// Every poll: true while the pointer keeps the card's choices open (the rail stays with them).
    pub fn holds(&mut self, point: Point, now: u64) -> (bool, Vec<TipAction>) {
        let mut out = Vec::new();
        let (Some(menu), Some(column)) = (self.menu_frame.filter(|_| self.visible), self.column)
        else {
            self.set_clickable(false, &mut out);
            return (false, out);
        };
        let over = is_inside(point, menu, 0.0);
        self.set_clickable(over, &mut out);
        let relayed = over.then(|| Point {
            x: round(point.x - column.x),
            y: round(point.y - column.y),
        });
        if self.relayed != Some(relayed) {
            self.relayed = Some(relayed);
            out.push(TipAction::Pointer(relayed));
        }
        let inside = is_inside(point, menu, HOLD_SLOP);
        if inside || !self.leaving {
            self.outside_since = 0;
            return (inside, out);
        }
        if self.outside_since == 0 {
            self.outside_since = now;
        }
        if now.saturating_sub(self.outside_since) < LEAVE_MS {
            return (true, out);
        }
        out.extend(self.hide());
        (false, out)
    }

    /// A pick from the card: `None` when the card doesn't offer it; else the card hides first unless
    /// the pick keeps it open.
    pub fn pick(&mut self, command: &str) -> Option<Vec<TipAction>> {
        if !self.offered.iter().any(|offered| offered == command) {
            return None;
        }
        Some(if stays_open(command) {
            Vec::new()
        } else {
            self.hide()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COLUMN: Frame = Frame::new(1184.0, 25.0, 284.0, 920.0);
    const FAR: Point = Point { x: 0.0, y: 0.0 };

    fn placement() -> Placement {
        Placement {
            x: 1468.0,
            y: 400.0,
            side: Side::Right,
            work_area: Frame::new(0.0, 25.0, 1512.0, 920.0),
            theme: RailTheme::System,
            highlight: RailHighlight::Tinted,
        }
    }

    fn tip() -> RailTip {
        RailTip {
            label: "Music".into(),
            detail: Some("Playing: Cell".into()),
            shortcut: Some("ctrl+alt+cmd+m".into()),
        }
    }

    fn pick(label: &str, command: &str) -> Choice {
        Choice::Item {
            label: label.into(),
            command: command.into(),
            detail: None,
            checked: None,
        }
    }

    fn choices() -> Vec<Choice> {
        vec![
            pick("Pause", "music:toggle"),
            pick("Cell", "music:play:cell"),
            pick("Work", "mode:work"),
            pick("Journal", "journal:open"),
        ]
    }

    /// The card spans x 1194..1458 and y 125..325 on screen.
    fn card() -> Frame {
        Frame::new(10.0, 100.0, 264.0, 200.0)
    }

    fn over() -> Point {
        Point {
            x: 1300.0,
            y: 200.0,
        }
    }

    fn open_card() -> TipState {
        let mut state = TipState::default();
        state.show(tip(), Some(ToolId::Music), placement());
        state.content(choices());
        state.rendered(1, card());
        assert!(state.visible());
        state
    }

    fn open_plain_tip() -> TipState {
        let mut state = TipState::default();
        state.show(tip(), None, placement());
        state.content(Vec::new());
        state.rendered(1, card());
        assert!(state.visible());
        state
    }

    #[test]
    fn the_column_touches_the_bars_inner_edge_and_spans_the_work_area() {
        let left = Placement {
            x: 44.0,
            side: Side::Left,
            ..placement()
        };
        let fractional = Placement {
            x: 1468.5,
            work_area: Frame::new(0.0, 24.6, 1512.0, 919.5),
            ..placement()
        };
        for (placement, frame) in [
            (placement(), COLUMN),
            (left, Frame::new(44.0, 25.0, 284.0, 920.0)),
            (fractional, Frame::new(1185.0, 25.0, 284.0, 920.0)),
        ] {
            assert_eq!(column_for(&placement), frame, "{placement:?}");
        }
    }

    #[test]
    fn only_context_and_music_picks_keep_the_card_open() {
        for (command, stays) in [
            ("mode:work", true),
            ("music:toggle", true),
            ("music:play:cell", true),
            ("music:volume:50", true),
            ("status:break", false),
            ("journal:open:Dawn", false),
            ("link:https%3A%2F%2Fwiki", false),
            ("musical", false),
            ("", false),
        ] {
            assert_eq!(stays_open(command), stays, "{command}");
        }
    }

    #[test]
    fn showing_a_tip_sets_the_column_once_per_place() {
        let mut state = TipState::default();
        assert_eq!(
            state.show(tip(), None, placement()),
            vec![TipAction::Column(COLUMN)]
        );
        assert_eq!(state.show(tip(), None, placement()), vec![]);
        let left = Placement {
            x: 44.0,
            side: Side::Left,
            ..placement()
        };
        assert_eq!(
            state.show(tip(), None, left),
            vec![TipAction::Column(Frame::new(44.0, 25.0, 284.0, 920.0))]
        );
    }

    #[test]
    fn content_waits_for_a_tip_to_show() {
        assert_eq!(TipState::default().content(Vec::new()), None);
    }

    #[test]
    fn the_content_carries_the_tip_its_look_and_where_the_button_is() {
        let mut state = TipState::default();
        state.show(tip(), Some(ToolId::Music), placement());
        assert_eq!(
            state.content(choices()),
            Some(TipAction::Content(RailTipContent {
                tip: tip(),
                side: Side::Right,
                theme: RailTheme::System,
                highlight: RailHighlight::Tinted,
                sequence: 1,
                choices: choices(),
                anchor: 375.0,
                inset: 6.0,
                max_width: 264.0,
                height: 920.0,
            }))
        );
    }

    #[test]
    fn every_new_tip_gets_the_next_sequence() {
        let mut state = TipState::default();
        for expected in [1, 2, 3] {
            state.show(tip(), None, placement());
            let Some(TipAction::Content(content)) = state.content(Vec::new()) else {
                panic!("no content");
            };
            assert_eq!(content.sequence, expected);
        }
    }

    #[test]
    fn the_content_serializes_with_camel_case_names() {
        let mut state = TipState::default();
        state.show(tip(), None, placement());
        let Some(TipAction::Content(content)) = state.content(Vec::new()) else {
            panic!("no content");
        };
        assert_eq!(
            serde_json::to_value(&content).expect("serializes"),
            serde_json::json!({
                "tip": {
                    "label": "Music",
                    "detail": "Playing: Cell",
                    "shortcut": "ctrl+alt+cmd+m",
                },
                "side": "right",
                "theme": "system",
                "highlight": "tinted",
                "sequence": 1,
                "choices": [],
                "anchor": 375.0,
                "inset": 6.0,
                "maxWidth": 264.0,
                "height": 920.0,
            })
        );
    }

    #[test]
    fn the_first_render_reveals_the_window_and_later_ones_only_place_the_card() {
        let mut state = TipState::default();
        state.show(tip(), None, placement());
        state.content(Vec::new());
        assert_eq!(
            state.rendered(1, card()),
            vec![TipAction::Show(COLUMN), TipAction::Placed(1)]
        );
        state.show(tip(), None, placement());
        state.content(Vec::new());
        assert_eq!(state.rendered(2, card()), vec![TipAction::Placed(2)]);
    }

    #[test]
    fn a_render_of_a_stale_sequence_is_ignored() {
        let mut state = TipState::default();
        state.show(tip(), None, placement());
        state.show(tip(), None, placement());
        state.content(Vec::new());
        assert_eq!(state.rendered(1, card()), vec![]);
        assert!(!state.visible());
        assert_eq!(
            state.rendered(2, card()),
            vec![TipAction::Show(COLUMN), TipAction::Placed(2)]
        );
    }

    #[test]
    fn a_render_without_a_tip_is_ignored() {
        assert_eq!(TipState::default().rendered(1, card()), vec![]);
    }

    #[test]
    fn the_menu_is_known_before_it_is_on_screen() {
        let mut state = TipState::default();
        state.show(tip(), Some(ToolId::Music), placement());
        assert_eq!(state.menu(), Some(ToolId::Music));
        assert_eq!(state.open_menu(), None);
        state.content(choices());
        state.rendered(1, card());
        assert_eq!(state.open_menu(), Some(ToolId::Music));
        state.hide();
        assert_eq!(state.menu(), None);
        assert_eq!(state.open_menu(), None);
    }

    #[test]
    fn the_card_takes_clicks_only_while_the_pointer_is_on_it() {
        let mut state = open_card();
        assert_eq!(
            state.holds(over(), 1000),
            (
                true,
                vec![
                    TipAction::Clickable(true),
                    TipAction::Pointer(Some(Point { x: 116.0, y: 175.0 })),
                ]
            )
        );
        assert_eq!(state.holds(over(), 1100), (true, vec![]));
        assert_eq!(
            state.holds(FAR, 1200),
            (
                false,
                vec![TipAction::Clickable(false), TipAction::Pointer(None)]
            )
        );
    }

    #[test]
    fn the_card_holds_within_ten_points_around_it() {
        for (x, y, holds) in [
            (1190.0, 120.0, true),
            (1184.0, 115.0, true),
            (1467.0, 334.0, true),
            (1183.0, 200.0, false),
            (1300.0, 114.0, false),
            (1468.0, 200.0, false),
            (1300.0, 335.0, false),
        ] {
            let mut state = open_card();
            let (held, _) = state.holds(Point { x, y }, 1000);
            assert_eq!(held, holds, "{x},{y}");
        }
    }

    #[test]
    fn a_plain_tooltip_never_holds_or_takes_clicks() {
        let mut state = open_plain_tip();
        assert_eq!(state.holds(over(), 1000), (false, vec![]));
    }

    #[test]
    fn without_a_tooltip_nothing_is_held() {
        assert_eq!(TipState::default().holds(over(), 1000), (false, vec![]));
    }

    #[test]
    fn the_card_closes_350_ms_after_the_pointer_left_both_button_and_card() {
        let mut state = open_card();
        assert_eq!(state.leave(), vec![]);
        assert_eq!(
            state.holds(FAR, 1000),
            (true, vec![TipAction::Pointer(None)])
        );
        assert_eq!(state.holds(FAR, 1349), (true, vec![]));
        assert_eq!(
            state.holds(FAR, 1350),
            (false, vec![TipAction::Hidden(2), TipAction::HideLater])
        );
        assert!(!state.visible());
    }

    #[test]
    fn returning_to_the_card_starts_the_leave_wait_over() {
        let mut state = open_card();
        state.leave();
        state.holds(FAR, 1000);
        state.holds(over(), 1200);
        state.holds(FAR, 1300);
        assert_eq!(state.holds(FAR, 1649), (true, vec![]));
        let (held, actions) = state.holds(FAR, 1650);
        assert!(!held);
        assert_eq!(actions, vec![TipAction::Hidden(2), TipAction::HideLater]);
    }

    #[test]
    fn the_card_stays_while_the_pointer_is_still_on_the_button() {
        let mut state = open_card();
        state.holds(FAR, 1000);
        assert_eq!(state.holds(FAR, 5000), (false, vec![]));
        assert!(state.visible());
    }

    #[test]
    fn a_new_tip_cancels_a_pending_leave() {
        let mut state = open_card();
        state.leave();
        state.show(tip(), Some(ToolId::Music), placement());
        state.content(choices());
        state.rendered(2, card());
        state.holds(FAR, 1000);
        state.holds(FAR, 5000);
        assert!(state.visible());
    }

    #[test]
    fn leaving_hides_a_plain_tooltip_at_once() {
        let mut state = open_plain_tip();
        assert_eq!(
            state.leave(),
            vec![TipAction::Hidden(2), TipAction::HideLater]
        );
        assert!(!state.visible());
    }

    #[test]
    fn leaving_before_the_tooltip_appeared_has_nothing_to_hide() {
        let mut state = TipState::default();
        state.show(tip(), None, placement());
        assert_eq!(state.leave(), vec![]);
    }

    #[test]
    fn hiding_while_the_card_takes_clicks_lets_them_through_again() {
        let mut state = open_card();
        state.holds(over(), 1000);
        assert_eq!(
            state.hide(),
            vec![
                TipAction::Clickable(false),
                TipAction::Hidden(2),
                TipAction::HideLater,
            ]
        );
    }

    #[test]
    fn hiding_an_unseen_tooltip_announces_nothing() {
        let mut state = TipState::default();
        state.show(tip(), None, placement());
        assert_eq!(state.hide(), vec![]);
    }

    #[test]
    fn a_pick_of_something_offered_hides_the_card_first() {
        let mut state = open_card();
        assert_eq!(
            state.pick("journal:open"),
            Some(vec![TipAction::Hidden(2), TipAction::HideLater])
        );
        assert!(!state.visible());
    }

    #[test]
    fn picking_a_context_or_music_keeps_the_card_open() {
        for command in ["music:toggle", "music:play:cell", "mode:work"] {
            let mut state = open_card();
            assert_eq!(state.pick(command), Some(vec![]), "{command}");
            assert!(state.visible(), "{command}");
        }
    }

    #[test]
    fn a_pick_the_card_doesnt_offer_is_refused() {
        let mut state = open_card();
        for command in ["status:off", "music:volume:50", "mode:", ""] {
            assert_eq!(state.pick(command), None, "{command}");
        }
        assert!(state.visible());
    }

    #[test]
    fn a_late_pick_after_the_card_hid_is_refused() {
        let mut state = open_card();
        state.hide();
        assert_eq!(state.pick("music:toggle"), None);
        assert_eq!(state.pick("journal:open"), None);
    }

    #[test]
    fn a_new_card_offers_only_its_own_choices() {
        let mut state = open_card();
        state.show(tip(), Some(ToolId::Journal), placement());
        state.content(vec![pick("Dawn", "journal:open:Dawn")]);
        assert_eq!(state.pick("music:toggle"), None);
        assert!(state.pick("journal:open:Dawn").is_some());
    }
}
