//! The rail's windows as AppKit sees them: non-activating panels that never become key, on every Space
//! (also over full-screen apps) at the floating level, left alone when Daybook hides. Every function
//! runs on the main thread with a live `NSWindow`.

use std::ffi::c_void;

use objc2::msg_send;
use objc2::runtime::AnyObject;

use crate::window::panel::native::make_inert_panel;

const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
/// Stays put in Mission Control.
const STATIONARY: usize = 1 << 4;
const IGNORES_CYCLE: usize = 1 << 6;
const FULL_SCREEN_AUXILIARY: usize = 1 << 8;
/// `NSFloatingWindowLevel`.
const FLOATING_LEVEL: isize = 3;

/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
unsafe fn window<'a>(ns_window: *mut c_void) -> &'a AnyObject {
    // SAFETY: the caller passes a live window.
    unsafe { &*ns_window.cast::<AnyObject>() }
}

/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn prepare(ns_window: *mut c_void, what: &str) {
    // SAFETY: as the caller promises; plain NSWindow / NSPanel setters.
    unsafe {
        let panel = make_inert_panel(ns_window, what);
        let window = window(ns_window);
        let behavior = CAN_JOIN_ALL_SPACES | STATIONARY | IGNORES_CYCLE | FULL_SCREEN_AUXILIARY;
        let _: () = msg_send![window, setCollectionBehavior: behavior];
        let _: () = msg_send![window, setLevel: FLOATING_LEVEL];
        // `-[NSApp hide:]` (closing the Daybook window) would hide the rail too.
        let _: () = msg_send![window, setCanHide: false];
        if panel {
            let _: () = msg_send![window, setBecomesKeyOnlyIfNeeded: true];
        }
    }
}

/// On screen without activating Daybook or taking the keyboard.
///
/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn order_front(ns_window: *mut c_void) {
    // SAFETY: plain NSWindow call.
    unsafe {
        let _: () = msg_send![window(ns_window), orderFrontRegardless];
    }
}

/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn order_out(ns_window: *mut c_void) {
    let none: Option<&AnyObject> = None;
    // SAFETY: plain NSWindow call.
    unsafe {
        let _: () = msg_send![window(ns_window), orderOut: none];
    }
}
