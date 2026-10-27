//! The rail at the screen edge (docs/rail.md): the poll, the window work and the buttons' model.

pub mod commands;
pub mod controller;
mod edge;
mod follow;
pub mod geometry;
pub mod model;
mod native;
pub mod tip;
mod tip_window;
pub mod visibility;
mod window;

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use chrono::Local;
use tauri::{AppHandle, Emitter, Manager};

pub use commands::register;

use self::controller::{Action, Controller, Poll, RailShown};
use self::model::{RailInput, RailModel, rail_model};
use self::tip::TipState;
use self::tip_window::{apply_tip, hide_tip, send_content};
use crate::events::{RAIL_MODEL_EVENT, RAIL_POINTER_EVENT, RAIL_SHOWN_EVENT};
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::store::settings::Settings;
use crate::tools::{self, ToolState};
use crate::{bus, calls, music, shortcuts, tray};

const POLL: Duration = Duration::from_millis(150);
const FAST_POLL: Duration = Duration::from_millis(50);
/// The page slides the bar out first; the window hides once it's gone.
const SLIDE_OUT: Duration = Duration::from_millis(180);
/// A shown rail's captions (break minutes) stay current.
const MODEL_REFRESH: Duration = Duration::from_secs(30);

pub struct RailState {
    controller: Mutex<Controller>,
    tip: Mutex<TipState>,
    model: Mutex<Option<RailModel>>,
    /// The tools' state the model was built from, for the tooltip's choices.
    tools: Mutex<Option<ToolState>>,
    model_built: Mutex<Option<Instant>>,
    has_vault: AtomicBool,
    started: Instant,
}

impl Default for RailState {
    fn default() -> Self {
        Self {
            controller: Mutex::new(Controller::new(Default::default())),
            tip: Mutex::default(),
            model: Mutex::default(),
            tools: Mutex::default(),
            model_built: Mutex::default(),
            has_vault: AtomicBool::new(false),
            started: Instant::now(),
        }
    }
}

fn rail(app: &AppHandle) -> &RailState {
    app.state::<RailState>().inner()
}

impl RailState {
    /// Milliseconds since start, never 0 (0 means unset in the timers).
    fn now(&self) -> u64 {
        u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX) + 1
    }
}

fn emit<S: serde::Serialize + Clone>(app: &AppHandle, label: &str, event: &str, payload: S) {
    if let Err(e) = app.emit_to(label, event, payload) {
        log::warn!("Couldn't send {event}: {e}");
    }
}

/// Builds the windows and starts the poll.
pub fn install(app: &AppHandle) {
    settings_changed(app, &app.state::<AppState>().settings.get());
    if let Err(e) = window::build(app) {
        log::warn!("Couldn't build the rail: {e}");
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let fast = tick(&app).await;
            tokio::time::sleep(if fast { FAST_POLL } else { POLL }).await;
        }
    });
}

/// One poll; true when the next should come soon.
async fn tick(app: &AppHandle) -> bool {
    let rail = rail(app);
    let has_vault = rail.has_vault.load(Ordering::SeqCst);
    if !rail.controller.lock_unpoisoned().allowed(has_vault) {
        let actions = rail.controller.lock_unpoisoned().hide();
        apply(app, actions);
        return false;
    }
    let Some((point, screen)) = window::pointer(app).await else {
        return false;
    };
    let now = rail.now();
    let (on_tip_menu, tip_actions) = rail.tip.lock_unpoisoned().holds(point, now);
    apply_tip(app, tip_actions);
    let poll = Poll {
        point,
        screen,
        has_vault,
        call_active: calls::active(app).is_some(),
        on_tip_menu,
        now,
    };
    let (actions, visible, fast) = {
        let mut controller = rail.controller.lock_unpoisoned();
        let actions = controller.poll(poll);
        (actions, controller.visible(), controller.fast())
    };
    apply(app, actions);
    if visible && model_stale(rail) {
        refresh(app);
    }
    fast
}

fn model_stale(rail: &RailState) -> bool {
    rail.model_built
        .lock_unpoisoned()
        .is_none_or(|built| built.elapsed() >= MODEL_REFRESH)
}

fn apply(app: &AppHandle, actions: Vec<Action>) {
    for action in actions {
        match action {
            Action::Place(frame) => window::set_frame(app, window::RAIL, frame),
            Action::Show => {
                window::show(app, window::RAIL);
                refresh(app);
                // The Music button shows what plays now, not what the last regular look found.
                let handle = app.clone();
                tauri::async_runtime::spawn(async move { music::refresh(&handle).await });
            }
            Action::Hide => {
                let handle = app.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(SLIDE_OUT).await;
                    if !rail(&handle).controller.lock_unpoisoned().visible() {
                        window::order_out(&handle, window::RAIL);
                    }
                });
            }
            Action::HideTip => hide_tip(app),
            Action::Announce(shown) => emit(app, window::RAIL, RAIL_SHOWN_EVENT, shown),
            Action::Pointer(point) => emit(app, window::RAIL, RAIL_POINTER_EVENT, point),
        }
    }
}

/// Follows saved settings: the rail's own and whether a vault is open.
pub fn settings_changed(app: &AppHandle, saved: &Settings) {
    let rail = rail(app);
    rail.has_vault
        .store(saved.vault.path.is_some(), Ordering::SeqCst);
    let actions = rail
        .controller
        .lock_unpoisoned()
        .apply_settings(saved.rail.clone());
    apply(app, actions);
}

fn build_model(app: &AppHandle, state: &ToolState) -> RailModel {
    let settings = app.state::<AppState>().settings.get();
    let hotkeys = shortcuts::shown(app);
    let commands = app.state::<bus::Commands>();
    let call = calls::active(app);
    rail_model(&RailInput {
        tools: &settings.tools,
        state,
        hotkeys: &hotkeys,
        commands: &commands.available(app),
        music: &music::current(app),
        call_title: call.as_ref().map(|call| call.title.as_str()),
        now: Local::now().time(),
        settings: &settings.rail,
    })
}

/// What the tools show changed: the rail redraws, an open tooltip's choices follow.
pub fn state_changed(app: &AppHandle, state: &ToolState) {
    update(app, state);
}

fn update(app: &AppHandle, state: &ToolState) -> RailModel {
    let rail = rail(app);
    let model = build_model(app, state);
    *rail.tools.lock_unpoisoned() = Some(state.clone());
    *rail.model_built.lock_unpoisoned() = Some(Instant::now());
    let changed = {
        let mut current = rail.model.lock_unpoisoned();
        let changed = current.as_ref() != Some(&model);
        *current = Some(model.clone());
        changed
    };
    if changed {
        emit(app, window::RAIL, RAIL_MODEL_EVENT, model.clone());
    }
    let open_menu = rail.tip.lock_unpoisoned().open_menu();
    if open_menu.is_some() {
        send_content(app);
    }
    model
}

fn refresh(app: &AppHandle) {
    update(app, &tools::snapshot(app));
}

/// The buttons as last built, built now for a page that loads first.
pub fn current_model(app: &AppHandle) -> RailModel {
    let built = rail(app).model.lock_unpoisoned().clone();
    built.unwrap_or_else(|| update(app, &tools::snapshot(app)))
}

pub fn shown(app: &AppHandle) -> RailShown {
    rail(app).controller.lock_unpoisoned().shown()
}

/// Shows the rail on the pointer's screen and keeps it there (the shortcut, scripts).
pub async fn show(app: &AppHandle) {
    pinned(app, Controller::pin).await;
}

/// The shortcut: shows and keeps the rail, or hides the rail it keeps.
pub async fn toggle(app: &AppHandle) {
    pinned(app, Controller::toggle).await;
}

async fn pinned(
    app: &AppHandle,
    change: fn(&mut Controller, geometry::Point, controller::Screen, bool) -> Vec<Action>,
) {
    let Some((point, screen)) = window::pointer(app).await else {
        return;
    };
    let call_active = calls::active(app).is_some();
    let actions = change(
        &mut rail(app).controller.lock_unpoisoned(),
        point,
        screen,
        call_active,
    );
    apply(app, actions);
}

pub fn hide(app: &AppHandle) {
    let actions = rail(app).controller.lock_unpoisoned().hide();
    apply(app, actions);
}

pub fn set_height(app: &AppHandle, height: f64) {
    let actions = rail(app).controller.lock_unpoisoned().set_height(height);
    apply(app, actions);
}

/// A right-click: the menu-bar menu at the pointer. The rail stays while it is open.
pub async fn context_menu(app: &AppHandle) {
    let Some(rail_window) = app.get_webview_window(window::RAIL) else {
        return;
    };
    let menu = match tray::context_menu(app) {
        Ok(menu) => menu,
        Err(e) => return log::warn!("Couldn't build the rail's menu: {e}"),
    };
    rail(app).controller.lock_unpoisoned().set_menu_open(true);
    // Returns once the menu closed.
    let shown = tauri::async_runtime::spawn_blocking(move || rail_window.popup_menu(&menu)).await;
    rail(app).controller.lock_unpoisoned().set_menu_open(false);
    if let Ok(Err(e)) = shown {
        log::warn!("Couldn't show the rail's menu: {e}");
    }
}
