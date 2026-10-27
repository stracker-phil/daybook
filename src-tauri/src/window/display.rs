//! Screens and where windows go on them. The geometry is pure; the screens, the pointer and the
//! window frames come from AppKit (`native`), all in Cocoa coordinates (points, y grows upwards).

/// A rectangle in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Rect {
    pub const fn new(x: f64, y: f64, width: f64, height: f64) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Left and bottom edges inside, right and top edges outside, so neighbouring screens never
    /// both contain a point.
    pub fn contains(&self, x: f64, y: f64) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// A screen's whole frame (to find it) and its visible frame without menu bar and Dock (to place in).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Screen {
    pub frame: Rect,
    pub visible: Rect,
}

/// One axis of `frame_on_display`: the start keeps its relative place in the free room, the size
/// shrinks to fit.
fn axis(start: f64, size: f64, from: (f64, f64), to: (f64, f64)) -> (f64, f64) {
    let next = size.min(to.1);
    let room = from.1 - size;
    let ratio = if room > 0.0 {
        ((start - from.0) / room).clamp(0.0, 1.0)
    } else {
        0.5
    };
    // Not `round`: half a point always rounds up, also on negative coordinates.
    ((to.0 + ratio * (to.1 - next) + 0.5).floor(), next)
}

/// A window frame moved from one screen's visible area to another's: at the same relative place (a
/// window in the top-right corner stays there), shrunk when the other screen is smaller.
pub fn frame_on_display(frame: Rect, from: Rect, to: Rect) -> Rect {
    let (x, width) = axis(frame.x, frame.width, (from.x, from.width), (to.x, to.width));
    let (y, height) = axis(
        frame.y,
        frame.height,
        (from.y, from.height),
        (to.y, to.height),
    );
    Rect::new(x, y, width, height)
}

/// The screen containing the point, else the first (the one with the menu bar).
pub fn screen_at(screens: &[Screen], x: f64, y: f64) -> Option<Screen> {
    screens
        .iter()
        .find(|screen| screen.frame.contains(x, y))
        .or(screens.first())
        .copied()
}

fn overlap(a: Rect, b: Rect) -> f64 {
    let width = (a.x + a.width).min(b.x + b.width) - a.x.max(b.x);
    let height = (a.y + a.height).min(b.y + b.height) - a.y.max(b.y);
    width.max(0.0) * height.max(0.0)
}

/// The screen a window is on, as AppKit's `NSWindow.screen`: the one it overlaps most, else the one at
/// its centre, else the first.
pub fn screen_holding(screens: &[Screen], frame: Rect) -> Option<Screen> {
    let best = screens
        .iter()
        .map(|screen| (screen, overlap(screen.frame, frame)))
        .filter(|(_, area)| *area > 0.0)
        .max_by(|a, b| a.1.total_cmp(&b.1));
    match best {
        Some((screen, _)) => Some(*screen),
        None => screen_at(
            screens,
            frame.x + frame.width / 2.0,
            frame.y + frame.height / 2.0,
        ),
    }
}

/// The quick-notes editor while it presents: the frame to return to and the screen it covers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Presentation {
    pub normal: Rect,
    pub screen: Screen,
}

/// The whole screen, its menu bar strip and Dock included.
pub fn presentation_frame(screen: Screen) -> Rect {
    screen.frame
}

/// `normal` carried from the presented screen to `to`, unchanged when it is the same screen.
fn normal_on(presentation: &Presentation, to: Screen) -> Rect {
    if to == presentation.screen {
        presentation.normal
    } else {
        frame_on_display(presentation.normal, presentation.screen.visible, to.visible)
    }
}

/// The frame to return to: `normal` while its screen still exists, else `normal` moved onto the screen
/// holding it now, shrunk to fit.
pub fn exit_frame(presentation: &Presentation, screens: &[Screen]) -> Rect {
    if screens.contains(&presentation.screen) {
        return presentation.normal;
    }
    match screen_holding(screens, presentation.normal) {
        Some(screen) => normal_on(presentation, screen),
        None => presentation.normal,
    }
}

/// After a screen change: the presentation on the screen now holding the window (`current`), with
/// `normal` carried over. `None` without screens.
pub fn refit(
    presentation: &Presentation,
    screens: &[Screen],
    current: Rect,
) -> Option<Presentation> {
    let screen = screen_holding(screens, current)?;
    Some(Presentation {
        normal: normal_on(presentation, screen),
        screen,
    })
}

/// How far down its screen an overlay's top edge sits, as a share of the screen's height (like
/// Spotlight).
const TOP_SHARE: f64 = 0.2;

/// The frame that centres a window of this size horizontally in `visible`, its top edge `TOP_SHARE`
/// down, kept inside `visible`.
pub fn top_center(visible: Rect, width: f64, height: f64) -> Rect {
    let (left, bottom) = (visible.x, visible.y);
    let (right, top) = (left + visible.width, bottom + visible.height);
    let x = (left + (visible.width - width) / 2.0)
        .min(right - width)
        .max(left);
    let top_edge = (top - visible.height * TOP_SHARE)
        .max(bottom + height)
        .min(top);
    Rect::new(x.round(), (top_edge - height).round(), width, height)
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAPTOP: Rect = Rect::new(0.0, 25.0, 1512.0, 920.0);
    const EXTERNAL: Rect = Rect::new(1512.0, -200.0, 2560.0, 1415.0);

    fn screen(frame: Rect) -> Screen {
        Screen {
            frame,
            visible: frame,
        }
    }

    #[test]
    fn a_centered_window_lands_centered_on_the_other_screen() {
        let frame = Rect::new(136.0, 125.0, 1240.0, 720.0);
        assert_eq!(
            frame_on_display(frame, LAPTOP, EXTERNAL),
            Rect::new(1512.0 + 660.0, -200.0 + 348.0, 1240.0, 720.0)
        );
    }

    #[test]
    fn a_window_in_a_corner_keeps_its_corner() {
        let frame = Rect::new(272.0, 25.0, 1240.0, 720.0);
        let moved = frame_on_display(frame, LAPTOP, EXTERNAL);
        assert_eq!(moved.x + moved.width, EXTERNAL.x + EXTERNAL.width);
        assert_eq!(moved.y, EXTERNAL.y);
    }

    #[test]
    fn a_window_too_large_for_the_other_screen_shrinks_to_fit_it() {
        let frame = Rect::new(1600.0, -100.0, 1800.0, 1100.0);
        assert_eq!(frame_on_display(frame, EXTERNAL, LAPTOP), LAPTOP);
    }

    #[test]
    fn a_window_partly_off_its_screen_stays_on_the_other_one() {
        let frame = Rect::new(-300.0, 900.0, 1240.0, 720.0);
        let moved = frame_on_display(frame, LAPTOP, EXTERNAL);
        assert_eq!(moved.x, EXTERNAL.x);
        assert_eq!(moved.y + moved.height, EXTERNAL.y + EXTERNAL.height);
    }

    #[test]
    fn a_rect_contains_its_left_and_bottom_edges_but_not_its_right_and_top_edges() {
        let rect = Rect::new(10.0, 20.0, 100.0, 50.0);
        for (x, y, expected) in [
            (10.0, 20.0, true),
            (109.9, 69.9, true),
            (110.0, 30.0, false),
            (50.0, 70.0, false),
            (9.9, 30.0, false),
            (50.0, 19.9, false),
        ] {
            assert_eq!(rect.contains(x, y), expected, "({x}, {y})");
        }
    }

    fn screens() -> [Screen; 4] {
        [
            screen(Rect::new(0.0, 0.0, 1440.0, 900.0)),
            screen(Rect::new(1440.0, 0.0, 1920.0, 1080.0)),
            screen(Rect::new(-1280.0, 0.0, 1280.0, 1024.0)),
            screen(Rect::new(0.0, -800.0, 1440.0, 800.0)),
        ]
    }

    #[test]
    fn screen_at_finds_the_screen_under_the_point() {
        let screens = screens();
        for (x, y, index) in [
            (100.0, 100.0, 0),
            (2000.0, 500.0, 1),
            (-500.0, 300.0, 2),
            (700.0, -100.0, 3),
        ] {
            assert_eq!(
                screen_at(&screens, x, y),
                Some(screens[index]),
                "({x}, {y})"
            );
        }
    }

    #[test]
    fn the_far_edge_of_a_screen_belongs_to_the_next_one() {
        let screens = screens();
        assert_eq!(screen_at(&screens, 1440.0, 100.0), Some(screens[1]));
        assert_eq!(screen_at(&screens, 0.0, 100.0), Some(screens[0]));
        assert_eq!(screen_at(&screens, 100.0, 0.0), Some(screens[0]));
    }

    #[test]
    fn a_point_on_no_screen_falls_back_to_the_first() {
        let screens = screens();
        assert_eq!(screen_at(&screens, 9000.0, 9000.0), Some(screens[0]));
    }

    #[test]
    fn no_screens_means_no_screen() {
        assert_eq!(screen_at(&[], 10.0, 10.0), None);
    }

    #[test]
    fn top_center_puts_the_top_edge_a_fifth_down_and_centers_horizontally() {
        let visible = Rect::new(0.0, 0.0, 1440.0, 875.0);
        assert_eq!(
            top_center(visible, 600.0, 400.0),
            Rect::new(420.0, 300.0, 600.0, 400.0)
        );
    }

    #[test]
    fn top_center_offsets_by_the_origin_of_the_visible_area() {
        let visible = Rect::new(100.0, 50.0, 1440.0, 875.0);
        assert_eq!(
            top_center(visible, 600.0, 400.0),
            Rect::new(520.0, 350.0, 600.0, 400.0)
        );
    }

    #[test]
    fn top_center_pushes_a_tall_window_up_so_its_bottom_stays_on_the_screen() {
        let visible = Rect::new(0.0, 0.0, 1440.0, 875.0);
        assert_eq!(
            top_center(visible, 600.0, 800.0),
            Rect::new(420.0, 0.0, 600.0, 800.0)
        );
    }

    const LEFT: Rect = Rect::new(0.0, 0.0, 1000.0, 800.0);
    const RIGHT: Rect = Rect::new(1000.0, 0.0, 1000.0, 800.0);
    const SMALL: Rect = Rect::new(0.0, 0.0, 500.0, 400.0);

    fn presenting_on(on: Rect, normal: Rect) -> Presentation {
        Presentation {
            normal,
            screen: screen(on),
        }
    }

    #[test]
    fn a_window_holds_the_screen_it_overlaps_most() {
        let screens = [screen(LEFT), screen(RIGHT)];
        for (frame, index) in [
            (Rect::new(100.0, 100.0, 300.0, 200.0), 0),
            (Rect::new(1100.0, 100.0, 300.0, 200.0), 1),
            (Rect::new(900.0, 100.0, 300.0, 200.0), 1),
            (Rect::new(700.0, 100.0, 300.0, 200.0), 0),
        ] {
            assert_eq!(
                screen_holding(&screens, frame),
                Some(screens[index]),
                "{frame:?}"
            );
        }
    }

    #[test]
    fn a_window_off_every_screen_falls_back_to_the_first() {
        let screens = [screen(LEFT), screen(RIGHT)];
        assert_eq!(
            screen_holding(&screens, Rect::new(5000.0, 5000.0, 300.0, 200.0)),
            Some(screens[0])
        );
    }

    #[test]
    fn a_window_without_area_is_held_by_the_screen_at_its_centre() {
        let screens = [screen(LEFT), screen(RIGHT)];
        assert_eq!(
            screen_holding(&screens, Rect::new(1500.0, 100.0, 0.0, 0.0)),
            Some(screens[1])
        );
    }

    #[test]
    fn no_screens_hold_no_window() {
        assert_eq!(screen_holding(&[], LEFT), None);
    }

    #[test]
    fn a_presentation_covers_the_whole_frame_not_the_visible_area() {
        let with_menu_bar = Screen {
            frame: Rect::new(0.0, 0.0, 1440.0, 900.0),
            visible: Rect::new(0.0, 0.0, 1440.0, 875.0),
        };
        assert_eq!(presentation_frame(with_menu_bar), with_menu_bar.frame);
    }

    #[test]
    fn leaving_a_presentation_on_a_present_screen_restores_the_normal_frame_exactly() {
        let normal = Rect::new(123.5, 45.25, 601.0, 333.0);
        let presentation = presenting_on(RIGHT, normal);
        let screens = [screen(LEFT), screen(RIGHT)];
        assert_eq!(exit_frame(&presentation, &screens), normal);
    }

    #[test]
    fn leaving_a_presentation_whose_screen_is_gone_moves_the_frame_onto_the_remaining_one() {
        let presentation = presenting_on(RIGHT, Rect::new(1100.0, 100.0, 600.0, 400.0));
        assert_eq!(
            exit_frame(&presentation, &[screen(LEFT)]),
            Rect::new(100.0, 100.0, 600.0, 400.0)
        );
    }

    #[test]
    fn leaving_a_presentation_whose_screen_is_gone_shrinks_the_frame_to_the_remaining_one() {
        let presentation = presenting_on(RIGHT, Rect::new(1100.0, 100.0, 600.0, 400.0));
        assert_eq!(exit_frame(&presentation, &[screen(SMALL)]), SMALL);
    }

    #[test]
    fn leaving_a_presentation_without_screens_keeps_the_normal_frame() {
        let normal = Rect::new(1100.0, 100.0, 600.0, 400.0);
        assert_eq!(exit_frame(&presenting_on(RIGHT, normal), &[]), normal);
    }

    #[test]
    fn a_refit_on_the_same_screen_changes_nothing() {
        let presentation = presenting_on(LEFT, Rect::new(100.0, 100.0, 600.0, 400.0));
        let screens = [screen(LEFT), screen(RIGHT)];
        assert_eq!(refit(&presentation, &screens, LEFT), Some(presentation));
    }

    #[test]
    fn a_larger_resolution_carries_the_normal_frame_to_the_new_screen() {
        let presentation = presenting_on(LEFT, Rect::new(100.0, 100.0, 600.0, 400.0));
        let bigger = Rect::new(0.0, 0.0, 2000.0, 1600.0);
        assert_eq!(
            refit(&presentation, &[screen(bigger)], LEFT),
            Some(Presentation {
                normal: Rect::new(350.0, 300.0, 600.0, 400.0),
                screen: screen(bigger),
            })
        );
    }

    #[test]
    fn a_smaller_resolution_shrinks_the_normal_frame_to_the_new_screen() {
        let presentation = presenting_on(LEFT, Rect::new(100.0, 100.0, 600.0, 400.0));
        assert_eq!(
            refit(&presentation, &[screen(SMALL)], LEFT),
            Some(Presentation {
                normal: SMALL,
                screen: screen(SMALL),
            })
        );
    }

    #[test]
    fn a_refit_after_the_screen_is_gone_moves_to_the_remaining_screen() {
        let presentation = presenting_on(RIGHT, Rect::new(1100.0, 100.0, 600.0, 400.0));
        assert_eq!(
            refit(&presentation, &[screen(LEFT)], RIGHT),
            Some(Presentation {
                normal: Rect::new(100.0, 100.0, 600.0, 400.0),
                screen: screen(LEFT),
            })
        );
    }

    #[test]
    fn a_refit_without_screens_ends_with_nothing() {
        let presentation = presenting_on(LEFT, Rect::new(100.0, 100.0, 600.0, 400.0));
        assert_eq!(refit(&presentation, &[], LEFT), None);
    }

    #[test]
    fn top_center_keeps_the_top_left_of_a_window_larger_than_the_screen() {
        let visible = Rect::new(0.0, 0.0, 1440.0, 875.0);
        assert_eq!(
            top_center(visible, 2000.0, 1000.0),
            Rect::new(0.0, -125.0, 2000.0, 1000.0)
        );
    }
}
