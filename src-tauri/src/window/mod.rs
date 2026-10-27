//! The app's windows: the Daybook window here, Settings in `settings`, the overlay panel in `panel`, the
//! quick-notes editor in `quick_notes` (presenting in `presentation`), and the native window behaviour
//! they share (`display`, `front_app`, `native`).

pub mod display;
pub mod front_app;
pub(crate) mod native;
pub mod panel;
pub mod presentation;
pub mod quick_notes;
pub mod settings;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use crate::events::{SHOW_ACTIVITY_TAB_EVENT, TOGGLE_INSPECTOR_EVENT, WINDOW_OPENED_EVENT};

const MAIN: &str = "main";

/// Skipped with `DAYBOOK_BACKGROUND=1` in debug builds so scripts/dev never steal focus.
fn bring_to_front(window: &WebviewWindow) {
    #[cfg(debug_assertions)]
    if crate::dev::background() {
        return;
    }
    // Failing only means the window is gone; there is nothing left to bring forward.
    let _ = window.unminimize();
    let _ = window.set_focus();
}

/// Shows the hidden or minimized main window and brings it to the front.
pub fn show_main(app: &AppHandle) {
    show_main_in_background(app);
    if let Some(window) = app.get_webview_window(MAIN) {
        bring_to_front(&window);
    }
}

/// Shows the window on the pointer's screen without bringing the app to the front; emits
/// `WINDOW_OPENED_EVENT` when it was hidden.
pub fn show_main_in_background(app: &AppHandle) {
    set_dock_icon(app, true);
    let opening = !main_visible(app);
    if let Some(window) = app.get_webview_window(MAIN) {
        let shown = app.run_on_main_thread(move || {
            if let Ok(ns_window) = window.ns_window() {
                // SAFETY: run_on_main_thread runs this on the main thread; the window is alive.
                unsafe { native::move_to_pointer_screen(ns_window) };
            }
            let _ = window.show();
        });
        if let Err(e) = shown {
            log::warn!("Couldn't show the Daybook window: {e}");
        }
    }
    if opening && let Err(e) = app.emit(WINDOW_OPENED_EVENT, ()) {
        log::warn!("Couldn't tell the page the window opened: {e}");
    }
}

fn is_visible(app: &AppHandle, label: &str) -> bool {
    app.get_webview_window(label)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

pub fn main_visible(app: &AppHandle) -> bool {
    is_visible(app, MAIN)
}

fn set_dock_icon(app: &AppHandle, shown: bool) {
    #[cfg(target_os = "macos")]
    if let Err(e) = app.set_dock_visibility(shown) {
        log::warn!("Couldn't change the Dock icon: {e}");
    }
}

/// Hides a window as closing it does; the app keeps running in the menu bar. The Dock icon goes with
/// the last regular window (the Daybook window, Settings); with no window left, not even the quick-notes
/// editor, an active Daybook hides too, so macOS picks what comes forward on this Space instead of
/// Daybook bringing back an app.
fn hide_regular(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_webview_window(label) {
        let _ = window.hide();
    }
    if main_visible(app) || settings::visible(app) {
        return;
    }
    set_dock_icon(app, false);
    if quick_notes::visible(app) {
        return;
    }
    let hidden = app.run_on_main_thread(|| {
        if native::app_active() {
            native::hide_app();
        }
    });
    if let Err(e) = hidden {
        log::warn!("Couldn't hide Daybook: {e}");
    }
}

pub fn hide_main(app: &AppHandle) {
    hide_regular(app, MAIN);
}

/// Hides the Daybook window, Settings or the quick-notes editor on close and returns true; for other
/// windows it returns false, so their close proceeds.
pub fn close_requested(app: &AppHandle, label: &str) -> bool {
    if label == quick_notes::LABEL {
        quick_notes::hide(app);
        return true;
    }
    let regular = label == MAIN || label == settings::LABEL;
    if regular {
        hide_regular(app, label);
    }
    regular
}

/// Hides the Daybook window when it is in front, else shows it.
pub fn toggle_main(app: &AppHandle) {
    if main_visible(app) && focused(app, MAIN) {
        hide_main(app);
    } else {
        show_main(app);
    }
}

fn focused(app: &AppHandle, label: &str) -> bool {
    app.get_webview_window(label)
        .and_then(|window| window.is_focused().ok())
        .unwrap_or(false)
}

/// ⌘W and ⌘Q: an overlay closes as Esc closes it, a regular window hides.
pub fn close_focused(app: &AppHandle) {
    if focused(app, panel::LABEL) {
        panel::close(app, false);
    } else if focused(app, MAIN) {
        hide_main(app);
    } else if focused(app, settings::LABEL) {
        settings::hide(app);
    } else if focused(app, quick_notes::LABEL) {
        quick_notes::hide(app);
    }
}

/// Opens Settings at `page`, or on the page it showed when `None`.
pub fn open_settings(app: &AppHandle, page: Option<&str>) {
    settings::open(app, page);
}

/// Shows the Daybook window and shows or hides its inspector column.
pub fn toggle_inspector(app: &AppHandle) {
    show_main(app);
    if let Err(e) = app.emit_to(MAIN, TOGGLE_INSPECTOR_EVENT, ()) {
        log::warn!("Couldn't toggle the inspector: {e}");
    }
}

/// Shows the Daybook window with its inspector open on the Activity tab.
pub fn show_activity_tab(app: &AppHandle) {
    show_main(app);
    if let Err(e) = app.emit_to(MAIN, SHOW_ACTIVITY_TAB_EVENT, ()) {
        log::warn!("Couldn't show the Activity tab: {e}");
    }
}
