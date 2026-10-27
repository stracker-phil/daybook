//! How an open rail follows: a call takes it to the call's place, a kept rail goes along to the
//! pointer's screen, and its page gets the polled pointer. Pure.

use super::controller::{Action, Controller, Screen};
use super::geometry::{Point, round, side_for};
use super::visibility::is_inside;
use crate::store::settings::RailEdges;

impl Controller {
    /// A call starting takes an open rail to the call's place. After the call, one the shortcut keeps
    /// goes back to where the pointer is; any other hides as usual once the pointer leaves it.
    pub(super) fn follow_call(&mut self, point: Point, screen: Screen, out: &mut Vec<Action>) {
        let Some(on) = self.on.filter(|_| self.visible && !self.menu_open) else {
            return;
        };
        if self.in_call() && !self.call {
            self.move_to(on, self.side, true, out);
        } else if !self.in_call() && self.call && self.pinned_by_user {
            self.pointer_y = Some(point.y);
            let side = side_for(self.settings.edges, Some(point.x), screen.frame);
            self.move_to(screen, side, false, out);
        }
    }

    /// Only a rail that stays open follows the pointer to another screen.
    pub(super) fn follow_screen(&mut self, point: Point, screen: Screen, out: &mut Vec<Action>) {
        let elsewhere = self.on.is_some_and(|on| on != screen);
        if !(self.visible && self.is_pinned() && !self.menu_open && elsewhere) {
            return;
        }
        let side = if self.settings.edges == RailEdges::Both {
            side_for(RailEdges::Both, Some(point.x), screen.frame)
        } else {
            self.side
        };
        self.move_to(screen, side, self.call, out);
    }

    /// The rail never becomes key, so its page gets no hover events; it gets the polled pointer instead.
    pub(super) fn relay(&mut self, point: Point, out: &mut Vec<Action>) {
        let relayed = self
            .frame()
            .filter(|frame| is_inside(point, *frame, 0.0))
            .map(|frame| Point {
                x: round(point.x - frame.x),
                y: round(point.y - frame.y),
            });
        if self.relayed != Some(relayed) {
            self.relayed = Some(relayed);
            out.push(Action::Pointer(relayed));
        }
    }
}
