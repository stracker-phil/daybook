//! The rail window and its tooltip window: transparent, shadowless pages built hidden at start, placed
//! in y-down points (converted to Cocoa here), shown and hidden without taking focus.

use std::ffi::c_void;

use tauri::{AppHandle, Manager, WebviewUrl, WebviewWindowBuilder};
use tokio::sync::oneshot;

use super::controller::{INITIAL_HEIGHT, Screen};
use super::geometry::{Frame, Point, flip, flip_point};
use super::native;
use crate::window::display::{Rect, Screen as DisplayScreen, screen_at};
use crate::window::native as appkit;

pub const RAIL: &str = "rail";
pub const TIP: &str = "railtip";
const RAIL_URL: &str = "src/windows/rail/index.html";
const TIP_URL: &str = "src/windows/railtip/index.html";
/// Wide enough for the large bar; every placement sets the frame.
const RAIL_WIDTH: f64 = 60.0;
const TIP_WIDTH: f64 = 284.0;
const TIP_HEIGHT: f64 = 600.0;

fn build_one(
    app: &AppHandle,
    label: &str,
    title: &str,
    url: &str,
    (width, height): (f64, f64),
) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(title)
        .inner_size(width, height)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .visible(false)
        .focused(false)
        .focusable(false)
        .accept_first_mouse(true)
        .build()?;
    if let Ok(ns_window) = window.ns_window() {
        // SAFETY: setup runs on the main thread and the window was just created.
        unsafe { native::prepare(ns_window, title) };
    }
    Ok(())
}

/// Builds both windows hidden. The tooltip window lets clicks through and is shown once while still
/// empty: a window's first show restores a remembered frame, and a first hover shouldn't wait for it.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    build_one(
        app,
        RAIL,
        "Daybook Rail",
        RAIL_URL,
        (RAIL_WIDTH, INITIAL_HEIGHT),
    )?;
    build_one(
        app,
        TIP,
        "Daybook Rail Tooltip",
        TIP_URL,
        (TIP_WIDTH, TIP_HEIGHT),
    )?;
    set_clickable(app, false);
    with_window(app, TIP, |ns_window| {
        // SAFETY: on the main thread with the live tooltip window.
        unsafe {
            native::order_front(ns_window);
            native::order_out(ns_window);
        }
    });
    Ok(())
}

/// Runs `f` with the window's `NSWindow` on the main thread, without waiting.
fn with_window(app: &AppHandle, label: &'static str, f: impl FnOnce(*mut c_void) + Send + 'static) {
    let handle = app.clone();
    let ran = app.run_on_main_thread(move || {
        if let Some(ns_window) = handle
            .get_webview_window(label)
            .and_then(|window| window.ns_window().ok())
        {
            f(ns_window);
        }
    });
    if let Err(e) = ran {
        log::warn!("Couldn't reach the {label} window: {e}");
    }
}

fn to_frame(rect: Rect) -> Frame {
    Frame::new(rect.x, rect.y, rect.width, rect.height)
}

fn to_rect(frame: Frame) -> Rect {
    Rect::new(frame.x, frame.y, frame.width, frame.height)
}

/// The main screen's height, where y-down coordinates start.
fn main_height_of(screens: &[DisplayScreen]) -> f64 {
    screens.first().map_or(0.0, |screen| screen.frame.height)
}

/// `frame` in y-down points.
pub fn set_frame(app: &AppHandle, label: &'static str, frame: Frame) {
    with_window(app, label, move |ns_window| {
        let cocoa = to_rect(flip(frame, main_height_of(&appkit::screens())));
        // SAFETY: on the main thread with the live window.
        unsafe { appkit::set_frame(ns_window, cocoa) };
    });
}

pub fn show(app: &AppHandle, label: &'static str) {
    with_window(app, label, |ns_window| {
        // SAFETY: on the main thread with the live window.
        unsafe { native::order_front(ns_window) }
    });
}

pub fn order_out(app: &AppHandle, label: &'static str) {
    with_window(app, label, |ns_window| {
        // SAFETY: on the main thread with the live window.
        unsafe { native::order_out(ns_window) }
    });
}

/// The tooltip window takes clicks only while the pointer is on its choices.
pub fn set_clickable(app: &AppHandle, clickable: bool) {
    if let Some(window) = app.get_webview_window(TIP)
        && let Err(e) = window.set_ignore_cursor_events(!clickable)
    {
        log::warn!("Couldn't make the tooltip clickable: {e}");
    }
}

/// The pointer and its screen, y-down.
pub async fn pointer(app: &AppHandle) -> Option<(Point, Screen)> {
    let (tx, rx) = oneshot::channel();
    app.run_on_main_thread(move || {
        let screens = appkit::screens();
        let (x, y) = appkit::pointer();
        let _ = tx.send(screen_at(&screens, x, y).map(|screen| {
            let height = main_height_of(&screens);
            let screen = Screen {
                frame: flip(to_frame(screen.frame), height),
                visible: flip(to_frame(screen.visible), height),
            };
            (flip_point(x, y, height), screen)
        }));
    })
    .ok()?;
    rx.await.ok().flatten()
}
