//! The app's windows: the Daybook window here, the overlay panel in `panel`, and the native window
//! behaviour they share (`display`, `front_app`, `native`).

pub mod display;
pub mod front_app;
mod native;
pub mod panel;

use tauri::{AppHandle, Emitter, Manager};

use crate::events::{OPEN_SETTINGS_EVENT, WINDOW_OPENED_EVENT};

const MAIN: &str = "main";

/// Skipped with `DAYBOOK_BACKGROUND=1` in debug builds so scripts/dev never steal focus.
fn bring_to_front(app: &AppHandle) {
    #[cfg(debug_assertions)]
    if crate::dev::background() {
        return;
    }
    // Failing only means the window is gone; there is nothing left to bring forward.
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

/// Shows the hidden or minimized main window and brings it to the front.
pub fn show_main(app: &AppHandle) {
    show_main_in_background(app);
    bring_to_front(app);
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

pub fn main_visible(app: &AppHandle) -> bool {
    app.get_webview_window(MAIN)
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

fn set_dock_icon(app: &AppHandle, shown: bool) {
    #[cfg(target_os = "macos")]
    if let Err(e) = app.set_dock_visibility(shown) {
        log::warn!("Couldn't change the Dock icon: {e}");
    }
}

/// Hides the window and the Dock icon, as closing it does; the app keeps running in the menu bar. An active Daybook hides
/// too, so macOS picks what comes forward on this Space instead of Daybook bringing back an app.
pub fn hide_main(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.hide();
    }
    set_dock_icon(app, false);
    let hidden = app.run_on_main_thread(|| {
        if native::app_active() {
            native::hide_app();
        }
    });
    if let Err(e) = hidden {
        log::warn!("Couldn't hide Daybook: {e}");
    }
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

/// ⌘W and ⌘Q: an overlay closes as Esc closes it, the Daybook window hides.
pub fn close_focused(app: &AppHandle) {
    if focused(app, panel::LABEL) {
        panel::close(app, false);
    } else if focused(app, MAIN) {
        hide_main(app);
    }
}

/// Settings at `page` (the first page when unknown), in the Daybook window.
pub fn open_settings(app: &AppHandle, page: Option<&str>) {
    show_main(app);
    if let Err(e) = app.emit_to(MAIN, OPEN_SETTINGS_EVENT, page) {
        log::warn!("Couldn't open Settings: {e}");
    }
}
