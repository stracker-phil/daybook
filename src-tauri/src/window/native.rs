//! AppKit calls the window layer needs and Tauri doesn't offer: screens and the pointer, window frames,
//! the app in front, and activating or hiding apps. Every function runs on the main thread.

use std::ffi::c_void;

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};
use objc2_core_foundation::CFArray;
use objc2_core_graphics::{CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};

use super::display::{Rect, Screen, frame_on_display, screen_at};
use super::front_app::FrontApp;

fn to_rect(r: NSRect) -> Rect {
    Rect::new(r.origin.x, r.origin.y, r.size.width, r.size.height)
}

fn to_ns(r: Rect) -> NSRect {
    NSRect::new(NSPoint::new(r.x, r.y), NSSize::new(r.width, r.height))
}

/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
unsafe fn window<'a>(ns_window: *mut c_void) -> &'a AnyObject {
    // SAFETY: the caller passes a live window.
    unsafe { &*ns_window.cast::<AnyObject>() }
}

pub fn screens() -> Vec<Screen> {
    // SAFETY: `+[NSScreen screens]` returns an array of NSScreen; this runs on the main thread.
    unsafe {
        let Some(screens): Option<Retained<AnyObject>> = msg_send![class!(NSScreen), screens]
        else {
            return Vec::new();
        };
        let count: usize = msg_send![&screens, count];
        (0..count)
            .map(|i| {
                let screen: Retained<AnyObject> = msg_send![&screens, objectAtIndex: i];
                let frame: NSRect = msg_send![&screen, frame];
                let visible: NSRect = msg_send![&screen, visibleFrame];
                Screen {
                    frame: to_rect(frame),
                    visible: to_rect(visible),
                }
            })
            .collect()
    }
}

fn pointer_on(screens: &[Screen]) -> Option<Screen> {
    // SAFETY: `+[NSEvent mouseLocation]` takes no arguments.
    let point: NSPoint = unsafe { msg_send![class!(NSEvent), mouseLocation] };
    screen_at(screens, point.x, point.y)
}

/// The screen with the pointer.
pub fn pointer_screen() -> Option<Screen> {
    pointer_on(&screens())
}

/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn frame(ns_window: *mut c_void) -> Rect {
    // SAFETY: plain NSWindow getter.
    to_rect(unsafe { msg_send![window(ns_window), frame] })
}

/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn set_frame(ns_window: *mut c_void, frame: Rect) {
    // SAFETY: plain NSWindow setter.
    unsafe {
        let _: () = msg_send![window(ns_window), setFrame: to_ns(frame), display: true];
    }
}

/// Moves the window to the pointer's screen, at the same relative place, shrunk to fit.
///
/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn move_to_pointer_screen(ns_window: *mut c_void) {
    let screens = screens();
    let Some(target) = pointer_on(&screens) else {
        return;
    };
    // SAFETY: the caller passes a live window.
    let current = unsafe { frame(ns_window) };
    let (center_x, center_y) = (
        current.x + current.width / 2.0,
        current.y + current.height / 2.0,
    );
    let Some(from) = screen_at(&screens, center_x, center_y) else {
        return;
    };
    if from == target {
        return;
    }
    let moved = frame_on_display(current, from.visible, target.visible);
    // SAFETY: as above.
    unsafe { set_frame(ns_window, moved) };
}

fn shared_application() -> Retained<AnyObject> {
    // SAFETY: `+[NSApplication sharedApplication]` always returns the app object.
    unsafe { msg_send![class!(NSApplication), sharedApplication] }
}

/// Shown on the Space the owner looks at (also over a full-screen app) instead of switching to its own.
///
/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn move_to_active_space(ns_window: *mut c_void) {
    const MOVE_TO_ACTIVE_SPACE: usize = 1 << 1;
    const FULL_SCREEN_AUXILIARY: usize = 1 << 8;
    // SAFETY: plain NSWindow setter.
    unsafe {
        let behavior = MOVE_TO_ACTIVE_SPACE | FULL_SCREEN_AUXILIARY;
        let _: () = msg_send![window(ns_window), setCollectionBehavior: behavior];
    }
}

/// Daybook is the active app.
pub fn app_active() -> bool {
    // SAFETY: plain NSApplication getter.
    unsafe { msg_send![&shared_application(), isActive] }
}

/// Hides Daybook, so macOS picks what comes forward on this Space.
pub fn hide_app() {
    let none: Option<&AnyObject> = None;
    // SAFETY: `-[NSApplication hide:]` takes an optional sender.
    unsafe {
        let _: () = msg_send![&shared_application(), hide: none];
    }
}

/// The frontmost app unless it is Daybook.
pub fn frontmost_app() -> Option<FrontApp> {
    let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
    let pid = app.processIdentifier();
    if pid == std::process::id() as i32 {
        return None;
    }
    let bundle_id = app
        .bundleIdentifier()
        .map(|id| id.to_string())
        .unwrap_or_default();
    Some(FrontApp { pid, bundle_id })
}

/// The app has a normal window on the current Space (the on-screen window list holds only those).
pub fn on_current_space(pid: i32) -> bool {
    let Some(list) = CGWindowListCopyWindowInfo(
        CGWindowListOption::OptionOnScreenOnly | CGWindowListOption::ExcludeDesktopElements,
        kCGNullWindowID,
    ) else {
        return false;
    };
    let list: *const CFArray = &*list;
    // SAFETY: a CFArray of CFDictionary is toll-free bridged to an NSArray of NSDictionary, so it
    // answers NSArray messages.
    let (windows, count): (&AnyObject, usize) = unsafe {
        let windows: &AnyObject = &*list.cast();
        (windows, msg_send![windows, count])
    };
    (0..count).any(|i| {
        // SAFETY: an element of the window list.
        let info: Retained<AnyObject> = unsafe { msg_send![windows, objectAtIndex: i] };
        // Layer 0 holds normal windows; menus, panels and the menu bar sit above it.
        number(&info, "kCGWindowOwnerPID") == Some(f64::from(pid))
            && number(&info, "kCGWindowLayer") == Some(0.0)
    })
}

fn number(dictionary: &AnyObject, key: &str) -> Option<f64> {
    let key = NSString::from_str(key);
    // SAFETY: `dictionary` is an NSDictionary keyed by strings holding NSNumbers.
    unsafe {
        let value: Option<Retained<AnyObject>> = msg_send![dictionary, objectForKey: &*key];
        value.map(|value| msg_send![&value, doubleValue])
    }
}

/// Brings the app forward; false when it quit meanwhile or refused.
pub fn activate(pid: i32) -> bool {
    NSRunningApplication::runningApplicationWithProcessIdentifier(pid).is_some_and(|app| {
        // The cooperative successor is a request macOS may decline; handing focus back must not fail.
        #[allow(deprecated)]
        app.activateWithOptions(NSApplicationActivationOptions::empty())
    })
}
