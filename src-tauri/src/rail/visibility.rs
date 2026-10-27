//! When the rail shows and hides, Dock-like: the pointer rests at the screen edge to show it and stays
//! away to hide it. Pure; times in milliseconds.

use super::geometry::{Frame, Point};

pub const DWELL_MS: u64 = 200;
pub const LEAVE_MS: u64 = 700;

pub fn is_inside(point: Point, frame: Frame, slop: f64) -> bool {
    point.x >= frame.x - slop
        && point.x < frame.x + frame.width + slop
        && point.y >= frame.y - slop
        && point.y < frame.y + frame.height + slop
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RailView {
    /// A vault is open and the rail is switched on or pinned.
    pub allowed: bool,
    pub enabled: bool,
    pub pinned: bool,
    pub visible: bool,
    pub at_edge: bool,
    /// The pointer is over the rail or at the edge of its screen, or one of its menus is open.
    pub held: bool,
}

/// 0 means unset.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RailTimers {
    pub edge_since: u64,
    pub outside_since: u64,
}

pub const IDLE: RailTimers = RailTimers {
    edge_since: 0,
    outside_since: 0,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailAction {
    Show,
    Hide,
    Nothing,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RailStep {
    pub action: RailAction,
    pub timers: RailTimers,
}

/// How long the pointer rests at the edge before the rail shows, and stays away before it hides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RailDelays {
    pub show: u64,
    pub hide: u64,
}

impl Default for RailDelays {
    fn default() -> Self {
        Self {
            show: DWELL_MS,
            hide: LEAVE_MS,
        }
    }
}

const fn step(action: RailAction, timers: RailTimers) -> RailStep {
    RailStep { action, timers }
}

pub fn next_rail_step(
    view: RailView,
    timers: RailTimers,
    now: u64,
    delays: RailDelays,
) -> RailStep {
    if !view.allowed {
        return step(RailAction::Hide, IDLE);
    }
    if view.visible {
        while_shown(view, timers, now, delays)
    } else {
        while_hidden(view, timers, now, delays)
    }
}

fn since(start: u64, now: u64) -> u64 {
    if start == 0 { now } else { start }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HIDDEN: RailView = RailView {
        allowed: true,
        enabled: true,
        pinned: false,
        visible: false,
        at_edge: false,
        held: false,
    };
    const SHOWN: RailView = RailView {
        visible: true,
        ..HIDDEN
    };

    fn timers(edge_since: u64, outside_since: u64) -> RailTimers {
        RailTimers {
            edge_since,
            outside_since,
        }
    }

    fn defaults(view: RailView, timers: RailTimers, now: u64) -> RailStep {
        next_rail_step(view, timers, now, RailDelays::default())
    }

    #[test]
    fn it_hides_when_there_is_no_vault_or_the_rail_is_off() {
        let view = RailView {
            allowed: false,
            ..SHOWN
        };
        assert_eq!(
            defaults(view, IDLE, 1000),
            RailStep {
                action: RailAction::Hide,
                timers: IDLE
            }
        );
    }

    #[test]
    fn it_shows_once_the_pointer_has_rested_at_the_edge() {
        let view = RailView {
            at_edge: true,
            ..HIDDEN
        };
        let first = defaults(view, IDLE, 1000);
        assert_eq!(
            first,
            RailStep {
                action: RailAction::Nothing,
                timers: timers(1000, 0)
            }
        );
        assert_eq!(
            defaults(view, first.timers, 1000 + DWELL_MS),
            RailStep {
                action: RailAction::Show,
                timers: IDLE
            }
        );
    }

    #[test]
    fn it_starts_over_when_the_pointer_leaves_the_edge() {
        assert_eq!(
            defaults(HIDDEN, timers(1000, 0), 1100),
            RailStep {
                action: RailAction::Nothing,
                timers: IDLE
            }
        );
    }

    #[test]
    fn it_does_not_show_at_the_edge_while_switched_off_but_does_when_pinned() {
        let off_at_edge = RailView {
            enabled: false,
            at_edge: true,
            ..HIDDEN
        };
        assert_eq!(
            defaults(off_at_edge, timers(1, 0), 5000),
            RailStep {
                action: RailAction::Nothing,
                timers: IDLE
            }
        );
        let off_pinned = RailView {
            enabled: false,
            pinned: true,
            ..HIDDEN
        };
        assert_eq!(
            defaults(off_pinned, IDLE, 1),
            RailStep {
                action: RailAction::Show,
                timers: IDLE
            }
        );
    }

    #[test]
    fn it_hides_after_the_pointer_stayed_away() {
        let first = defaults(SHOWN, IDLE, 1000);
        assert_eq!(
            first,
            RailStep {
                action: RailAction::Nothing,
                timers: timers(0, 1000)
            }
        );
        assert_eq!(
            defaults(SHOWN, first.timers, 1000 + LEAVE_MS),
            RailStep {
                action: RailAction::Hide,
                timers: IDLE
            }
        );
    }

    #[test]
    fn it_stays_while_held_or_pinned() {
        for (name, view) in [
            (
                "held",
                RailView {
                    held: true,
                    ..SHOWN
                },
            ),
            (
                "pinned",
                RailView {
                    pinned: true,
                    ..SHOWN
                },
            ),
        ] {
            assert_eq!(
                defaults(view, timers(0, 1), 9999),
                RailStep {
                    action: RailAction::Nothing,
                    timers: IDLE
                },
                "{name}"
            );
        }
    }

    #[test]
    fn it_waits_as_long_as_the_delays_say() {
        let delays = RailDelays {
            show: 0,
            hide: 1500,
        };
        let at_edge = RailView {
            at_edge: true,
            ..HIDDEN
        };
        assert_eq!(
            next_rail_step(at_edge, IDLE, 1000, delays),
            RailStep {
                action: RailAction::Show,
                timers: IDLE
            }
        );

        let first = next_rail_step(SHOWN, IDLE, 1000, delays);
        assert_eq!(first.timers, timers(0, 1000));
        assert_eq!(
            next_rail_step(SHOWN, first.timers, 1700, delays),
            RailStep {
                action: RailAction::Nothing,
                timers: first.timers
            }
        );
        assert_eq!(
            next_rail_step(SHOWN, first.timers, 2500, delays),
            RailStep {
                action: RailAction::Hide,
                timers: IDLE
            }
        );
    }

    #[test]
    fn frames_are_measured_with_an_optional_slop() {
        let frame = Frame::new(10.0, 10.0, 20.0, 20.0);
        let point = Point { x: 30.0, y: 15.0 };
        assert!(!is_inside(point, frame, 0.0));
        assert!(is_inside(point, frame, 16.0));
    }
}

fn while_hidden(view: RailView, timers: RailTimers, now: u64, delays: RailDelays) -> RailStep {
    if view.pinned {
        return step(RailAction::Show, IDLE);
    }
    if !view.enabled || !view.at_edge {
        return step(RailAction::Nothing, IDLE);
    }
    let edge_since = since(timers.edge_since, now);
    if now.saturating_sub(edge_since) >= delays.show {
        return step(RailAction::Show, IDLE);
    }
    step(
        RailAction::Nothing,
        RailTimers {
            edge_since,
            ..timers
        },
    )
}

fn while_shown(view: RailView, timers: RailTimers, now: u64, delays: RailDelays) -> RailStep {
    if view.pinned || view.held {
        return step(RailAction::Nothing, IDLE);
    }
    let outside_since = since(timers.outside_since, now);
    if now.saturating_sub(outside_since) >= delays.hide {
        return step(RailAction::Hide, IDLE);
    }
    step(
        RailAction::Nothing,
        RailTimers {
            outside_since,
            ..timers
        },
    )
}
