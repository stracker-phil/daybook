//! Where the rail goes on a screen. Points with y growing downwards from the top of the main screen;
//! `flip` converts Cocoa's y-up rectangles once per poll. Pure.

use serde::Serialize;
use ts_rs::TS;

use crate::store::settings::{RailEdges, RailPosition, RailShape, RailSize};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "RailPoint")]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "RailRect")]
pub struct Frame {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Frame {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "RailSide")]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Left,
    Right,
}

impl Side {
    /// This side as the only edge, e.g. to ask whether the pointer is at the rail's own edge.
    pub fn as_edges(self) -> RailEdges {
        match self {
            Side::Left => RailEdges::Left,
            Side::Right => RailEdges::Right,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RailLook {
    pub shape: RailShape,
    pub size: RailSize,
}

/// Room around the bar inside the rail window, for its shadow.
const RAIL_PADDING: f64 = 6.0;
/// Off the top and bottom of the work area when the rail sits at one end or at the pointer.
const END_MARGIN: f64 = 8.0;

/// `Math.round`: half a point rounds up, also on negative coordinates.
pub fn round(value: f64) -> f64 {
    (value + 0.5).floor()
}

fn bar_width(size: RailSize) -> f64 {
    match size {
        RailSize::Compact => 32.0,
        RailSize::Regular => 40.0,
        RailSize::Large => 48.0,
    }
}

/// A docked bar touches the screen edge, so its window has no padding on that side.
fn outer_padding(shape: RailShape) -> f64 {
    if shape == RailShape::Docked {
        0.0
    } else {
        RAIL_PADDING
    }
}

fn edge_gap(shape: RailShape) -> f64 {
    if shape == RailShape::Docked { 0.0 } else { 4.0 }
}

fn rail_top(work_area: Frame, height: f64, position: RailPosition, pointer_y: Option<f64>) -> f64 {
    let first = work_area.y + END_MARGIN;
    let last = work_area.y + work_area.height - height - END_MARGIN;
    let clamp = |y: f64| work_area.y.max(first.max(y).min(last));
    let center = work_area.y + (work_area.height - height) / 2.0;
    match position {
        RailPosition::Top => clamp(first),
        RailPosition::Bottom => clamp(last),
        RailPosition::Pointer => pointer_y.map_or(center, |y| clamp(y - height / 2.0)),
        RailPosition::Center => center,
    }
}

/// The rail window at one edge of a screen's work area.
pub fn rail_frame(
    work_area: Frame,
    look: RailLook,
    side: Side,
    height: f64,
    position: RailPosition,
    pointer_y: Option<f64>,
) -> Frame {
    let bar = bar_width(look.size);
    let outer = outer_padding(look.shape);
    let x = match side {
        Side::Right => work_area.x + work_area.width - edge_gap(look.shape) - bar - RAIL_PADDING,
        Side::Left => work_area.x + edge_gap(look.shape) - outer,
    };
    Frame::new(
        round(x),
        round(rail_top(work_area, height, position, pointer_y)),
        bar + RAIL_PADDING + outer,
        height,
    )
}

/// Screen x of the bar's side that faces the screen, where tooltips attach.
pub fn bar_inner_edge(frame: Frame, look: RailLook, side: Side) -> f64 {
    match side {
        Side::Right => frame.x + RAIL_PADDING,
        Side::Left => frame.x + outer_padding(look.shape) + bar_width(look.size),
    }
}

/// The edge of `bounds` (the whole screen) the pointer is at, if it may show the rail there. `zone`
/// points count as the edge, so it's reachable where a neighbouring screen continues past it.
pub fn edge_at(point: Point, bounds: Frame, edges: RailEdges, zone: f64) -> Option<Side> {
    let right = point.x >= bounds.x + bounds.width - zone;
    let left = point.x < bounds.x + zone;
    if right && edges != RailEdges::Left {
        Some(Side::Right)
    } else if left && edges != RailEdges::Right {
        Some(Side::Left)
    } else {
        None
    }
}

/// The side a rail opened without the edge (shortcut, call) takes: the configured one, or the nearer.
pub fn side_for(edges: RailEdges, pointer_x: Option<f64>, bounds: Frame) -> Side {
    match (edges, pointer_x) {
        (RailEdges::Left, _) => Side::Left,
        (RailEdges::Right, _) | (RailEdges::Both, None) => Side::Right,
        (RailEdges::Both, Some(x)) if x < bounds.x + bounds.width / 2.0 => Side::Left,
        (RailEdges::Both, Some(_)) => Side::Right,
    }
}

/// A rectangle between Cocoa (y up from the bottom of the main screen) and y-down coordinates; the same
/// formula works both ways.
pub fn flip(frame: Frame, main_height: f64) -> Frame {
    Frame::new(
        frame.x,
        main_height - frame.y - frame.height,
        frame.width,
        frame.height,
    )
}

/// A Cocoa point in y-down coordinates.
pub fn flip_point(x: f64, y: f64, main_height: f64) -> Point {
    Point {
        x,
        y: main_height - y,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORK_AREA: Frame = Frame::new(0.0, 25.0, 1512.0, 920.0);
    const SCREEN: Frame = Frame::new(0.0, 0.0, 1512.0, 982.0);
    const PILL: RailLook = RailLook {
        shape: RailShape::Pill,
        size: RailSize::Regular,
    };

    fn pill_frame(position: RailPosition, pointer_y: Option<f64>) -> Frame {
        rail_frame(WORK_AREA, PILL, Side::Right, 300.0, position, pointer_y)
    }

    fn at(x: f64, y: f64) -> Point {
        Point { x, y }
    }

    #[test]
    fn a_pill_floats_a_few_points_off_the_right_edge_with_shadow_room_on_both_sides() {
        let frame = pill_frame(RailPosition::Center, None);
        assert_eq!(frame, Frame::new(1462.0, 335.0, 52.0, 300.0));
        assert_eq!(bar_inner_edge(frame, PILL, Side::Right), 1468.0);
    }

    #[test]
    fn a_docked_bar_touches_the_edge_on_either_side() {
        let docked = RailLook {
            shape: RailShape::Docked,
            size: RailSize::Regular,
        };
        let right = rail_frame(
            WORK_AREA,
            docked,
            Side::Right,
            300.0,
            RailPosition::Center,
            None,
        );
        assert_eq!(right.x + right.width, 1512.0);
        assert_eq!(bar_inner_edge(right, docked, Side::Right), 1512.0 - 40.0);

        let left = rail_frame(
            WORK_AREA,
            docked,
            Side::Left,
            300.0,
            RailPosition::Center,
            None,
        );
        assert_eq!(left.x, 0.0);
        assert_eq!(bar_inner_edge(left, docked, Side::Left), 40.0);
    }

    #[test]
    fn sizes_change_the_bars_width() {
        let floating_compact = RailLook {
            shape: RailShape::Floating,
            size: RailSize::Compact,
        };
        let compact = rail_frame(
            WORK_AREA,
            floating_compact,
            Side::Left,
            300.0,
            RailPosition::Center,
            None,
        );
        assert_eq!(compact.width, 32.0 + 12.0);

        let pill_large = RailLook {
            shape: RailShape::Pill,
            size: RailSize::Large,
        };
        let large = rail_frame(
            WORK_AREA,
            pill_large,
            Side::Left,
            300.0,
            RailPosition::Center,
            None,
        );
        assert_eq!(large.width, 48.0 + 12.0);
        assert_eq!(bar_inner_edge(large, PILL, Side::Left), 4.0 + 40.0);
    }

    #[test]
    fn the_rail_sits_at_the_top_the_bottom_or_where_the_pointer_was_always_on_screen() {
        for (position, pointer_y, expected_y) in [
            (RailPosition::Top, None, 33.0),
            (RailPosition::Bottom, None, 25.0 + 920.0 - 300.0 - 8.0),
            (RailPosition::Pointer, Some(500.0), 350.0),
            (RailPosition::Pointer, Some(40.0), 33.0),
            (RailPosition::Pointer, None, 335.0),
        ] {
            assert_eq!(
                pill_frame(position, pointer_y).y,
                expected_y,
                "{position:?} {pointer_y:?}"
            );
        }
    }

    #[test]
    fn the_trigger_zone_widens_the_edge_and_both_edges_can_show_the_rail() {
        for (x, edges, zone, expected) in [
            (1511.0, RailEdges::Right, 2.0, Some(Side::Right)),
            (1500.0, RailEdges::Right, 2.0, None),
            (1500.0, RailEdges::Right, 16.0, Some(Side::Right)),
            (0.0, RailEdges::Right, 2.0, None),
            (0.0, RailEdges::Both, 2.0, Some(Side::Left)),
            (1511.0, RailEdges::Both, 2.0, Some(Side::Right)),
        ] {
            assert_eq!(
                edge_at(at(x, 400.0), SCREEN, edges, zone),
                expected,
                "{x} {edges:?} {zone}"
            );
        }
    }

    #[test]
    fn with_both_edges_a_shortcut_opens_the_rail_on_the_pointers_side() {
        for (edges, pointer_x, expected) in [
            (RailEdges::Both, Some(200.0), Side::Left),
            (RailEdges::Both, Some(1300.0), Side::Right),
            (RailEdges::Left, Some(1300.0), Side::Left),
            (RailEdges::Right, Some(200.0), Side::Right),
            (RailEdges::Both, None, Side::Right),
        ] {
            assert_eq!(
                side_for(edges, pointer_x, SCREEN),
                expected,
                "{edges:?} {pointer_x:?}"
            );
        }
    }

    #[test]
    fn flipping_a_frame_twice_gives_the_original() {
        let cocoa = Frame::new(10.0, 20.0, 52.0, 300.0);
        let flipped = flip(cocoa, 982.0);
        assert_eq!(flipped, Frame::new(10.0, 662.0, 52.0, 300.0));
        assert_eq!(flip(flipped, 982.0), cocoa);
    }

    #[test]
    fn a_cocoa_point_flips_to_y_down() {
        assert_eq!(flip_point(100.0, 982.0, 982.0), at(100.0, 0.0));
        assert_eq!(flip_point(100.0, 0.0, 982.0), at(100.0, 982.0));
    }

    #[test]
    fn half_a_point_rounds_up_also_below_zero() {
        for (value, expected) in [(2.5, 3.0), (2.4, 2.0), (-2.5, -2.0), (-2.6, -3.0)] {
            assert_eq!(round(value), expected, "{value}");
        }
    }
}
