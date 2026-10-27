//! Where the overlay sits: every fresh open centres it on the pointer's screen; while open, a new view
//! or height keeps the current (maybe dragged) top edge and middle.

use std::ffi::c_void;

use super::native;
use crate::window::display::{Rect, top_center};
use crate::window::native as appkit;

/// Centred on the pointer's screen, a fifth of the way down.
///
/// # Safety
/// On the main thread with the live overlay window.
pub(super) unsafe fn place(ns_window: *mut c_void, width: f64, height: f64) {
    let Some(screen) = appkit::pointer_screen() else {
        return;
    };
    // SAFETY: as the caller promises.
    unsafe {
        appkit::set_frame(ns_window, top_center(screen.visible, width, height));
        native::invalidate_shadow(ns_window);
    }
}

/// The overlay grows and shrinks downwards from a fixed top edge, and stays centred on its middle when
/// another view is wider.
///
/// # Safety
/// On the main thread with the live overlay window.
pub(super) unsafe fn resize_from_top(ns_window: *mut c_void, width: f64, height: f64) {
    // SAFETY: as the caller promises.
    unsafe {
        let frame = appkit::frame(ns_window);
        let top = frame.y + frame.height;
        let x = (frame.x + (frame.width - width) / 2.0).round();
        appkit::set_frame(ns_window, Rect::new(x, top - height, width, height));
        native::invalidate_shadow(ns_window);
    }
}
