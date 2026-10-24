//! The overlay window as a non-activating `NSPanel`, like Spotlight: it takes the keyboard without
//! activating Daybook, so the app in front stays in front and closing never switches Spaces. Every
//! function runs on the main thread with a live `NSWindow`.

use std::ffi::{CStr, c_void};
use std::sync::OnceLock;

use objc2::runtime::{AnyClass, AnyObject, Bool, ClassBuilder, Sel};
use objc2::{class, msg_send, sel};

const NONACTIVATING_PANEL: usize = 1 << 7;
const CAN_JOIN_ALL_SPACES: usize = 1 << 0;
const MOVE_TO_ACTIVE_SPACE: usize = 1 << 1;
const IGNORES_CYCLE: usize = 1 << 6;
const FULL_SCREEN_AUXILIARY: usize = 1 << 8;
/// `NSPopUpMenuWindowLevel`: above other apps' windows, also full-screen ones.
const POP_UP_MENU_LEVEL: isize = 101;
/// `NSFloatingWindowLevel`: a sticky view stays above normal windows, below menus.
const FLOATING_LEVEL: isize = 3;
/// The ivar tao's `TaoWindow` adds; the panel class repeats it so the swapped window keeps its layout.
const FOCUSABLE: &CStr = c"focusable";

extern "C-unwind" fn yes(_: &AnyObject, _: Sel) -> Bool {
    Bool::YES
}

extern "C-unwind" fn no(_: &AnyObject, _: Sel) -> Bool {
    Bool::NO
}

/// An `NSPanel` that takes the keyboard although it has no title bar.
fn panel_class() -> &'static AnyClass {
    static CLASS: OnceLock<&'static AnyClass> = OnceLock::new();
    CLASS.get_or_init(|| {
        let mut builder = ClassBuilder::new(c"DaybookPanel", class!(NSPanel))
            .expect("the panel class is declared once");
        builder.add_ivar::<Bool>(FOCUSABLE);
        // SAFETY: both selectors take no arguments and return BOOL.
        unsafe {
            builder.add_method(
                sel!(canBecomeKeyWindow),
                yes as extern "C-unwind" fn(_, _) -> _,
            );
            builder.add_method(
                sel!(canBecomeMainWindow),
                no as extern "C-unwind" fn(_, _) -> _,
            );
        }
        builder.register()
    })
}

/// Where `class` or one of its superclasses keeps the `FOCUSABLE` ivar. A window macOS observes has
/// a key-value-observing subclass of `TaoWindow`, which declares no ivars of its own.
fn focusable_offset(class: &AnyClass) -> Option<isize> {
    std::iter::successors(Some(class), |class| class.superclass())
        .find_map(|class| class.instance_variable(FOCUSABLE))
        .map(|ivar| ivar.offset())
}

/// Whether an object of `from` can become a `to` in place: same size and the same ivar where it was.
fn same_layout(from: &AnyClass, to: &AnyClass) -> bool {
    let Some(focusable) = focusable_offset(from) else {
        return false;
    };
    from.instance_size() == to.instance_size() && focusable_offset(to) == Some(focusable)
}

/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
unsafe fn window<'a>(ns_window: *mut c_void) -> &'a AnyObject {
    // SAFETY: the caller passes a live window.
    unsafe { &*ns_window.cast::<AnyObject>() }
}

/// Turns the window into the panel; false (and nothing changes) when its class has an unexpected
/// layout, so the overlay stays a plain window.
///
/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn make_panel(ns_window: *mut c_void) -> bool {
    // SAFETY: the caller passes a live window.
    let window = unsafe { window(ns_window) };
    let panel = panel_class();
    if window.class() == panel {
        return true;
    }
    if !same_layout(window.class(), panel) {
        log::warn!(
            "The overlay stays a plain window: {:?} has an unexpected layout",
            window.class().name()
        );
        return false;
    }
    // SAFETY: checked above that the panel class matches the window's size and ivar.
    unsafe { AnyObject::set_class(window, panel) };
    // SAFETY: plain NSWindow / NSPanel setters.
    unsafe {
        let mask: usize = msg_send![window, styleMask];
        let _: () = msg_send![window, setStyleMask: mask | NONACTIVATING_PANEL];
        let _: () = msg_send![window, setHidesOnDeactivate: false];
        let _: () = msg_send![window, setAlphaValue: 0.0f64];
    }
    true
}

/// Sticky: on every Space above normal windows. Otherwise it appears on the active Space (also over
/// a full-screen app) and stays there.
///
/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn set_sticky(ns_window: *mut c_void, sticky: bool) {
    let (spaces, level) = if sticky {
        (CAN_JOIN_ALL_SPACES, FLOATING_LEVEL)
    } else {
        (MOVE_TO_ACTIVE_SPACE, POP_UP_MENU_LEVEL)
    };
    // SAFETY: plain NSWindow setters.
    unsafe {
        let window = window(ns_window);
        let behavior = spaces | FULL_SCREEN_AUXILIARY | IGNORES_CYCLE;
        let _: () = msg_send![window, setCollectionBehavior: behavior];
        let _: () = msg_send![window, setLevel: level];
    }
}

/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn set_alpha(ns_window: *mut c_void, alpha: f64) {
    // SAFETY: plain NSWindow setter.
    unsafe {
        let _: () = msg_send![window(ns_window), setAlphaValue: alpha];
    }
}

/// Brings the window to the front and gives it the keyboard without activating the app.
///
/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn show(ns_window: *mut c_void) {
    // SAFETY: plain NSWindow calls.
    unsafe {
        let window = window(ns_window);
        let _: () = msg_send![window, orderFrontRegardless];
        let _: () = msg_send![window, makeKeyWindow];
    }
}

/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn is_key(ns_window: *mut c_void) -> bool {
    // SAFETY: plain NSWindow getter.
    unsafe { msg_send![window(ns_window), isKeyWindow] }
}

/// Invisible first, so whatever macOS does with the window afterwards, a closed overlay isn't seen.
///
/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn hide(ns_window: *mut c_void) {
    let none: Option<&AnyObject> = None;
    // SAFETY: plain NSWindow calls.
    unsafe {
        set_alpha(ns_window, 0.0);
        let _: () = msg_send![window(ns_window), orderOut: none];
    }
}
