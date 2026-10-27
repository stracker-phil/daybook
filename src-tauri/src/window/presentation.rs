//! The quick-notes editor's presentation mode: the window covers the screen holding it, above that
//! screen's menu bar and Dock, without window buttons or resizing, and returns to its frame after. Every
//! AppKit call runs on the main thread.

use std::ffi::c_void;

use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

use super::display::{Presentation, Rect, exit_frame, presentation_frame, refit, screen_holding};
use super::native;
use super::quick_notes::LABEL;
use crate::events::QUICK_NOTE_PRESENTATION_EVENT;
use crate::quick_notes::QuickNotes;
use crate::quick_notes::presentation::QuickNotePresentation;
use crate::state::AppState;

fn quick_notes(app: &AppHandle) -> &QuickNotes {
    &app.state::<AppState>().inner().quick_notes
}

/// The editor's live `NSWindow`; only built on the main thread.
struct EditorWindow(*mut c_void);

impl EditorWindow {
    fn frame(&self) -> Rect {
        // SAFETY: the type's invariant.
        unsafe { native::frame(self.0) }
    }

    fn set_frame(&self, frame: Rect) {
        // SAFETY: the type's invariant.
        unsafe { native::set_frame(self.0, frame) };
    }

    fn set_look(&self, presenting: bool) {
        // SAFETY: the type's invariant.
        unsafe {
            native::set_resizable(self.0, !presenting);
            native::set_window_buttons_hidden(self.0, presenting);
            native::set_above_menu_bar(self.0, presenting);
        }
    }
}

/// Runs `apply` on the main thread with the editor's window, when it exists.
fn on_main(
    app: &AppHandle,
    apply: impl FnOnce(&AppHandle, &WebviewWindow, &EditorWindow) + Send + 'static,
) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    let handle = app.clone();
    let ran = app.run_on_main_thread(move || {
        if let Ok(ns_window) = window.ns_window() {
            // SAFETY: run_on_main_thread runs this on the main thread; `window` keeps it alive.
            apply(&handle, &window, &EditorWindow(ns_window));
        }
    });
    if let Err(e) = ran {
        log::warn!("Couldn't change the quick note presentation: {e}");
    }
}

/// Starts presenting, or ends it.
pub fn toggle(app: &AppHandle) {
    on_main(app, |app, window, editor| {
        if quick_notes(app).presenting() {
            leave(app, window, editor);
        } else {
            enter(app, window, editor);
        }
    });
}

/// Ends presenting; does nothing when not presenting.
pub fn end(app: &AppHandle) {
    if !quick_notes(app).presenting() {
        return;
    }
    on_main(app, leave);
}

fn enter(app: &AppHandle, window: &WebviewWindow, editor: &EditorWindow) {
    let normal = editor.frame();
    let Some(screen) = screen_holding(&native::screens(), normal) else {
        return;
    };
    quick_notes(app).set_presentation(Presentation { normal, screen });
    editor.set_look(true);
    editor.set_frame(presentation_frame(screen));
    announce(window, true);
}

fn leave(app: &AppHandle, window: &WebviewWindow, editor: &EditorWindow) {
    let Some(presentation) = quick_notes(app).take_presentation() else {
        return;
    };
    editor.set_look(false);
    editor.set_frame(exit_frame(&presentation, &native::screens()));
    announce(window, false);
}

fn announce(window: &WebviewWindow, presenting: bool) {
    if let Err(e) = window.emit_to(
        LABEL,
        QUICK_NOTE_PRESENTATION_EVENT,
        QuickNotePresentation { presenting },
    ) {
        log::warn!("Couldn't tell the editor about its presentation: {e}");
    }
}

/// Puts a presenting window back over its screen; showing it may have moved it below the menu bar.
///
/// # Safety
/// On the main thread; `ns_window` is `window`'s live `NSWindow`.
pub unsafe fn reassert(window: &WebviewWindow, ns_window: *mut c_void) {
    if let Some(presentation) = quick_notes(window.app_handle()).presentation() {
        EditorWindow(ns_window).set_frame(presentation_frame(presentation.screen));
    }
}

/// After a screen change, a presenting window covers the screen holding it now. Call on the main thread
/// at start.
pub fn install(app: &AppHandle) {
    let app = app.clone();
    native::on_screen_change(move || {
        if quick_notes(&app).presenting() {
            on_main(&app, |app, _, editor| refit_to_screens(app, editor));
        }
    });
}

fn refit_to_screens(app: &AppHandle, editor: &EditorWindow) {
    let state = quick_notes(app);
    let Some(presentation) = state.presentation() else {
        return;
    };
    let Some(next) = refit(&presentation, &native::screens(), editor.frame()) else {
        return;
    };
    state.set_presentation(next);
    editor.set_frame(presentation_frame(next.screen));
}
