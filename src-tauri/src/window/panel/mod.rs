//! The overlay: one non-activating panel window whose page renders the current view. Opening sets
//! the session, waits for the page to render it, shows the window invisible until the page painted,
//! then fades it in. A closed overlay is never on screen.

pub mod commands;
mod handshake;
mod native;
pub mod session;

use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::{
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use tokio::sync::oneshot;

use self::handshake::Handshake;
use self::session::{Closing, FocusLoss, OpenDecision, PanelState};
use super::display::{Rect, top_center};
use super::front_app::{CloseContext, HandBack, hand_back};
use super::native as appkit;
use crate::events::PANEL_SESSION_EVENT;
use crate::lock::MutexExt;

pub const LABEL: &str = "panel";
const URL: &str = "src/windows/panel/index.html";
const WIDTH: f64 = 580.0;
/// Past these the overlay opens anyway: shown before the page reported its height, faded in before
/// it painted.
const READY_TIMEOUT: Duration = Duration::from_millis(400);
const PAINT_TIMEOUT: Duration = Duration::from_millis(120);
/// macOS sometimes doesn't make the panel key at once (e.g. right after unlocking); check again.
const FOCUS_CHECK: Duration = Duration::from_millis(200);
const FOLLOW_FRONT_APP: Duration = Duration::from_millis(500);

#[derive(Default)]
pub struct Panel {
    state: Mutex<PanelState>,
    /// The content height.
    ready: Handshake<f64>,
    painted: Handshake<()>,
    /// The session whose front app is followed.
    following: AtomicU32,
}

fn panel(app: &AppHandle) -> tauri::State<'_, Panel> {
    app.state::<Panel>()
}

/// Creates the hidden overlay window, so the first opening doesn't wait for the page to load.
pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App(URL.into()))
        .title("Daybook")
        .inner_size(WIDTH, panel(app).state.lock_unpoisoned().height())
        .decorations(false)
        .transparent(true)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .focused(false)
        .shadow(true)
        .build()?;
    if let Ok(ns_window) = window.ns_window() {
        // SAFETY: setup runs on the main thread and the window was just created.
        unsafe { native::make_panel(ns_window) };
    }
    let handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::Focused(focused) = event {
            focus_changed(&handle, *focused);
        }
    });
    Ok(())
}

/// Runs `f` with the overlay's `NSWindow` on the main thread.
async fn on_main<R: Send + 'static>(
    app: &AppHandle,
    f: impl FnOnce(*mut c_void) -> R + Send + 'static,
) -> Option<R> {
    let window = app.get_webview_window(LABEL)?;
    let (tx, rx) = oneshot::channel();
    app.run_on_main_thread(move || {
        if let Ok(ns_window) = window.ns_window() {
            let _ = tx.send(f(ns_window));
        }
    })
    .ok()?;
    rx.await.ok()
}

fn send_session(app: &AppHandle) {
    let session = panel(app).state.lock_unpoisoned().session().cloned();
    if let Err(e) = app.emit_to(LABEL, PANEL_SESSION_EVENT, session) {
        log::warn!("Couldn't send the overlay its view: {e}");
    }
}

/// Opens `view` in a new session; the same view again closes it.
pub async fn open(app: &AppHandle, view: &str, args: Value) {
    let panel = panel(app);
    let decision = panel
        .state
        .lock_unpoisoned()
        .request(view, args, Instant::now());
    let session = match decision {
        OpenDecision::Ignore => return,
        OpenDecision::Close => return close(app, false),
        OpenDecision::Render(session) => session,
    };
    log::info!(
        "Opening the {} overlay (session {})",
        session.view,
        session.session
    );
    if !panel.state.lock_unpoisoned().is_open() {
        remember_front_app(app).await;
    }
    let id = session.session;
    let ready = panel.ready.expect(id);
    let painted = panel.painted.expect(id);
    send_session(app);

    let height = Handshake::wait(ready, READY_TIMEOUT).await;
    if height.is_none() {
        log::warn!("Showed the {view} overlay before its page was ready");
    }
    let (showing, sticky) = {
        let mut panel_state = panel.state.lock_unpoisoned();
        let Some(showing) = panel_state.show(id, height, Instant::now()) else {
            return;
        };
        (showing, panel_state.is_sticky())
    };
    let was_open = showing.was_open;
    on_main(app, move |ns_window| {
        // SAFETY: on the main thread with the live overlay window.
        unsafe {
            native::set_sticky(ns_window, sticky);
            if was_open {
                resize_from_top(ns_window, showing.height);
            } else {
                native::set_alpha(ns_window, 0.0);
                place(ns_window, showing.height);
            }
            native::show(ns_window);
        }
    })
    .await;
    if !was_open && Handshake::wait(painted, PAINT_TIMEOUT).await.is_none() {
        log::warn!("Faded in the {view} overlay before it painted");
    }
    if !is_current(app, id) {
        return;
    }
    // SAFETY: on the main thread with the live overlay window.
    on_main(app, |ns_window| unsafe {
        native::set_alpha(ns_window, 1.0)
    })
    .await;
    let handle = app.clone();
    tauri::async_runtime::spawn(async move { recheck_focus(handle, id).await });
}

/// Only while another app is still in front can we tell which one to hand focus back to.
async fn remember_front_app(app: &AppHandle) {
    let front = on_main(app, |_| (appkit::frontmost_app(), appkit::app_active())).await;
    let (target, active) = front.unwrap_or((None, false));
    panel(app).state.lock_unpoisoned().remember(target, active);
}

/// The open overlay's view.
pub fn view(app: &AppHandle) -> Option<String> {
    let panel = panel(app);
    let state = panel.state.lock_unpoisoned();
    state
        .is_open()
        .then(|| state.session().map(|s| s.view.clone()))
        .flatten()
}

/// The open overlay's session.
pub fn session_id(app: &AppHandle) -> Option<u32> {
    let panel = panel(app);
    let state = panel.state.lock_unpoisoned();
    state
        .is_open()
        .then(|| state.session().map(|s| s.session))
        .flatten()
}

fn is_current(app: &AppHandle, session: u32) -> bool {
    let panel = panel(app);
    let state = panel.state.lock_unpoisoned();
    state.is_open() && state.is_current(session)
}

async fn recheck_focus(app: AppHandle, session: u32) {
    tokio::time::sleep(FOCUS_CHECK).await;
    if !is_current(&app, session) {
        return;
    }
    // SAFETY: on the main thread with the live overlay window.
    on_main(&app, |ns_window| unsafe {
        if !native::is_key(ns_window) {
            native::show(ns_window);
        }
    })
    .await;
}

/// Centred on the pointer's screen, a fifth of the way down.
///
/// # Safety
/// On the main thread with the live overlay window.
unsafe fn place(ns_window: *mut c_void, height: f64) {
    let Some(screen) = appkit::pointer_screen() else {
        return;
    };
    // SAFETY: as the caller promises.
    unsafe { appkit::set_frame(ns_window, top_center(screen.visible, WIDTH, height)) };
}

/// The overlay grows and shrinks downwards from a fixed top edge.
///
/// # Safety
/// On the main thread with the live overlay window.
unsafe fn resize_from_top(ns_window: *mut c_void, height: f64) {
    // SAFETY: as the caller promises.
    unsafe {
        let frame = appkit::frame(ns_window);
        let top = frame.y + frame.height;
        appkit::set_frame(
            ns_window,
            Rect::new(frame.x, top - height, frame.width, height),
        );
    }
}

/// Closes the overlay. With `handed_off` another app has just taken focus, so it keeps it.
pub fn close(app: &AppHandle, handed_off: bool) {
    let closing = panel(app).state.lock_unpoisoned().close();
    if let Closing::Closed { .. } = &closing {
        log::info!(
            "Closing the overlay{}",
            if handed_off { " (focus moved on)" } else { "" }
        );
        send_session(app);
    }
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    let handle = app.clone();
    let hid = app.run_on_main_thread(move || after_close(&handle, &window, closing, handed_off));
    if let Err(e) = hid {
        log::warn!("Couldn't hide the overlay: {e}");
    }
}

fn after_close(app: &AppHandle, window: &WebviewWindow, closing: Closing, handed_off: bool) {
    let Ok(ns_window) = window.ns_window() else {
        return;
    };
    // SAFETY: run_on_main_thread runs this on the main thread; the window is alive.
    unsafe { native::hide(ns_window) };
    let Closing::Closed {
        target,
        opened_while_active,
    } = closing
    else {
        return;
    };
    let other_window_visible = super::main_visible(app);
    let context = CloseContext {
        daybook_active: appkit::app_active(),
        opened_while_active,
        handed_off,
        target_on_current_space: target.as_ref().map(|t| appkit::on_current_space(t.pid)),
        other_window_visible,
    };
    match hand_back(context) {
        HandBack::Nothing => {}
        HandBack::Activate => {
            let activated = target.is_some_and(|t| appkit::activate(t.pid));
            if !activated && !other_window_visible {
                appkit::hide_app();
            }
        }
        HandBack::HideApp => appkit::hide_app(),
    }
}

/// Runs on the main thread (window events do).
fn focus_changed(app: &AppHandle, focused: bool) {
    let panel = panel(app);
    if focused {
        if !panel.state.lock_unpoisoned().is_open() {
            log::warn!("The closed overlay got focus; hiding it again");
            close(app, false);
        }
        return;
    }
    let loss = panel.state.lock_unpoisoned().lost_focus(Instant::now());
    match loss {
        FocusLoss::Ignore => {}
        FocusLoss::Refocus => take_focus_back(app),
        FocusLoss::StayOpen => start_following(app, &panel),
        FocusLoss::Close => close(app, true),
    }
}

/// On the main thread.
fn take_focus_back(app: &AppHandle) {
    if let Some(ns_window) = app
        .get_webview_window(LABEL)
        .and_then(|w| w.ns_window().ok())
    {
        // SAFETY: the caller runs on the main thread; the window is alive.
        unsafe { native::show(ns_window) };
    }
}

/// One follow loop per session.
fn start_following(app: &AppHandle, panel: &Panel) {
    let session = panel.state.lock_unpoisoned().session().map(|s| s.session);
    if let Some(session) = session
        && panel.following.swap(session, Ordering::SeqCst) != session
    {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move { follow_front_app(handle, session).await });
    }
}

/// While a sticky view waits unfocused, keeps the app in front now as the one to hand focus back to.
async fn follow_front_app(app: AppHandle, session: u32) {
    while is_current(&app, session) {
        let front = on_main(&app, |_| appkit::frontmost_app()).await.flatten();
        panel(&app).state.lock_unpoisoned().follow(front);
        tokio::time::sleep(FOLLOW_FRONT_APP).await;
    }
}

/// Keeps the open overlay on screen while it loses focus (or lets it close again), as a sticky view.
pub async fn keep_open(app: &AppHandle, keep: bool) {
    let changed = panel(app).state.lock_unpoisoned().keep_open(keep);
    if changed {
        // SAFETY: on the main thread with the live overlay window.
        on_main(app, move |ns_window| unsafe {
            native::set_sticky(ns_window, keep)
        })
        .await;
    }
}

pub async fn set_height(app: &AppHandle, height: f64) {
    let changed = panel(app).state.lock_unpoisoned().set_height(height);
    if let Some(height) = changed {
        // SAFETY: on the main thread with the live overlay window.
        on_main(app, move |ns_window| unsafe {
            resize_from_top(ns_window, height)
        })
        .await;
    }
}
