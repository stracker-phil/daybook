//! Where the pointer counts as at the edge: the trigger zone, and the shown rail's own edge. Pure.

use super::controller::{Controller, Screen};
use super::geometry::{Point, Side, edge_at};
use crate::store::settings::{RailPosition, RailSpan};

impl Controller {
    /// Points from the screen edge that count as the edge.
    pub(super) fn trigger_zone(&self) -> f64 {
        f64::from(self.settings.trigger_width)
    }

    /// The edge the pointer is at, if it shows the rail there: within the trigger zone and, when the
    /// trigger is limited to the rail's height, beside where the rail would appear.
    pub(super) fn trigger_edge(&self, point: Point, screen: &Screen) -> Option<Side> {
        let edge = edge_at(
            point,
            screen.frame,
            self.settings.edges,
            self.trigger_zone(),
        )?;
        if self.settings.trigger_span == RailSpan::Full
            || self.settings.position == RailPosition::Pointer
        {
            return Some(edge);
        }
        let frame = self.frame_on(screen, edge, None, false);
        (point.y >= frame.y && point.y < frame.y + frame.height).then_some(edge)
    }

    /// Whether the pointer is at the shown rail's own edge. Pointing there takes over a rail the
    /// shortcut opened: it now goes once the pointer leaves.
    pub(super) fn handoff_at_own_edge(&mut self, point: Point, screen: Screen) -> bool {
        let at_own_edge = self.visible
            && self.on == Some(screen)
            && edge_at(
                point,
                screen.frame,
                self.side.as_edges(),
                self.trigger_zone(),
            ) == Some(self.side);
        if at_own_edge && self.pinned_by_user {
            self.pinned_by_user = false;
            self.handed_to_pointer = true;
        }
        at_own_edge
    }
}
