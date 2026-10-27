//! The quick-notes editor window: small, floating, its own page. Built on first use and hidden on close;
//! unlike the overlay it stays open when it loses focus. Closing ends a presentation and keeps the
//! normal frame for the next start.

use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use super::{native, presentation};
use crate::events::QUICK_NOTE_TARGET_EVENT;
use crate::quick_notes::Frame;
use crate::state::AppState;

pub const LABEL: &str = "quicknotes";
const URL: &str = "src/windows/quicknotes/index.html";
const WIDTH: f64 = 420.0;
const HEIGHT: f64 = 380.0;
const MIN_WIDTH: f64 = 300.0;
const MIN_HEIGHT: f64 = 180.0;

fn build(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App(URL.into()))
        .title("Quick Notes")
        .min_inner_size(MIN_WIDTH, MIN_HEIGHT)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(LogicalPosition::new(14.0, 18.0))
        .always_on_top(true)
        .skip_taskbar(true)
        .minimizable(false)
        .maximizable(false)
        .visible(false);
    let builder = match app.state::<AppState>().quick_notes.frame() {
        Some(frame) => builder
            .inner_size(frame.width, frame.height)
            .position(frame.x, frame.y),
        None => builder.inner_size(WIDTH, HEIGHT),
    };
    builder.build()
}

fn current_frame(window: &WebviewWindow) -> Option<Frame> {
    let scale = window.scale_factor().ok()?;
    let position = window.outer_position().ok()?.to_logical::<f64>(scale);
    let size = window.inner_size().ok()?.to_logical::<f64>(scale);
    Some(Frame {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    })
}

/// The main screen's height in points, which turns Cocoa frames into top-left ones.
fn main_height(window: &WebviewWindow) -> Option<f64> {
    let monitor = window.primary_monitor().ok()??;
    Some(f64::from(monitor.size().height) / monitor.scale_factor())
}

fn remember_frame(app: &AppHandle) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    let Some(current) = current_frame(&window) else {
        return;
    };
    let quick_notes = &app.state::<AppState>().quick_notes;
    let frame = match main_height(&window) {
        Some(height) => quick_notes.frame_to_remember(current, height),
        None if quick_notes.presenting() => return,
        None => current,
    };
    quick_notes.set_frame(frame);
}

fn find_or_build(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(LABEL).or_else(|| {
        build(app)
            .inspect_err(|e| log::warn!("Couldn't open the quick notes editor: {e}"))
            .ok()
    })
}

/// Shows the editor on a newly chosen topic: its page saves the topic before and loads this one.
pub fn open(app: &AppHandle) {
    let Some(window) = find_or_build(app) else {
        return;
    };
    if let Err(e) = window.emit_to(LABEL, QUICK_NOTE_TARGET_EVENT, ()) {
        log::warn!("Couldn't tell the editor its topic: {e}");
    }
    show(app, window);
}

/// Brings the open editor forward on the current Space.
pub fn focus(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        show(app, window);
    }
}

fn show(app: &AppHandle, window: WebviewWindow) {
    let shown = app.run_on_main_thread(move || {
        let ns_window = window.ns_window().ok();
        if let Some(ns_window) = ns_window {
            // SAFETY: run_on_main_thread runs this on the main thread; the window is alive.
            unsafe { native::move_to_active_space(ns_window) };
        }
        // Failing only means the window is gone.
        let _ = window.show();
        if let Some(ns_window) = ns_window {
            // SAFETY: as above.
            unsafe { presentation::reassert(&window, ns_window) };
        }
        super::bring_to_front(&window);
    });
    if let Err(e) = shown {
        log::warn!("Couldn't show the quick notes editor: {e}");
    }
}

pub fn visible(app: &AppHandle) -> bool {
    super::is_visible(app, LABEL)
}

/// `notes:present`: brings the open editor forward, then starts or ends presenting.
pub fn toggle_presentation(app: &AppHandle) {
    if !visible(app) {
        return;
    }
    focus(app);
    presentation::toggle(app);
}

pub fn hide(app: &AppHandle) {
    presentation::end(app);
    remember_frame(app);
    super::hide_regular(app, LABEL);
}
