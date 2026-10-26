//! The quick-notes editor window: small, floating, its own page. Built on first use and hidden on close;
//! unlike the overlay it stays open when it loses focus. Closing keeps its frame for the next start.

use tauri::{
    AppHandle, Emitter, LogicalPosition, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};

use super::native;
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

fn remember_frame(app: &AppHandle) {
    if let Some(frame) = app
        .get_webview_window(LABEL)
        .as_ref()
        .and_then(current_frame)
    {
        app.state::<AppState>().quick_notes.set_frame(frame);
    }
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
        if let Ok(ns_window) = window.ns_window() {
            // SAFETY: run_on_main_thread runs this on the main thread; the window is alive.
            unsafe { native::move_to_active_space(ns_window) };
        }
        // Failing only means the window is gone.
        let _ = window.show();
        super::bring_to_front(&window);
    });
    if let Err(e) = shown {
        log::warn!("Couldn't show the quick notes editor: {e}");
    }
}

pub fn visible(app: &AppHandle) -> bool {
    super::is_visible(app, LABEL)
}

pub fn hide(app: &AppHandle) {
    remember_frame(app);
    super::hide_regular(app, LABEL);
}
