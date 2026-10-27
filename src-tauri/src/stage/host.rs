//! The app as the hub sees it: Daybook's five windows through Tauri (in logical points, which are
//! macOS points and CSS pixels), the bus's commands, the shortcuts and the automation state.

use std::future::Future;

use serde_json::{Map, Value};
use tauri::{AppHandle, Emitter, EventTarget, Manager, WebviewWindow};

use super::catalog::{self, CommandStatus, StageCommand};
use super::hub::StageHost;
use super::protocol::ScreenRect;
use super::state::stage_state;
use super::windows::{Frame, StagePage};
use crate::automation::{self, AutomationError};
use crate::bus::{self, Host};
use crate::state::AppState;
use crate::store::settings::Hotkeys;
use crate::{rail, window};

pub struct TauriHost(pub AppHandle);

fn logical(
    position: tauri::PhysicalPosition<i32>,
    size: tauri::PhysicalSize<u32>,
    scale: f64,
) -> ScreenRect {
    let p = position.to_logical::<f64>(scale);
    let s = size.to_logical::<f64>(scale);
    ScreenRect {
        x: p.x.round() as i64,
        y: p.y.round() as i64,
        width: s.width.round() as i64,
        height: s.height.round() as i64,
    }
}

fn frame_of(w: &WebviewWindow) -> tauri::Result<Frame> {
    let scale = w.scale_factor()?;
    Ok(Frame {
        title: w.title().unwrap_or_default(),
        focused: w.is_focused().unwrap_or(false),
        bounds: logical(w.outer_position()?, w.outer_size()?, scale),
        content: logical(w.inner_position()?, w.inner_size()?, scale),
    })
}

impl TauriHost {
    fn window(&self, page: StagePage) -> Option<WebviewWindow> {
        self.0.get_webview_window(page.label())
    }
}

impl StageHost for TauriHost {
    fn frame(&self, page: StagePage) -> Option<Frame> {
        frame_of(&self.window(page)?)
            .inspect_err(|e| log::warn!("Stage can't measure the {} window: {e}", page.as_str()))
            .ok()
    }

    fn is_shown(&self, page: StagePage) -> bool {
        match page {
            StagePage::Panel => window::panel::view(&self.0).is_some(),
            StagePage::Rail => rail::shown(&self.0).visible,
            _ => self.window(page).is_some_and(|w| {
                w.is_visible().unwrap_or(false) && !w.is_minimized().unwrap_or(false)
            }),
        }
    }

    fn panel_view(&self) -> Option<String> {
        window::panel::view(&self.0)
    }

    fn show(&self, page: StagePage, section: Option<&str>) {
        let app = &self.0;
        match page {
            StagePage::Hub => window::show_main(app),
            StagePage::Settings => window::open_settings(app, section),
            // The overlay opens with a command; `window` refuses it before.
            StagePage::Panel => {}
            StagePage::Rail => {
                let app = app.clone();
                tauri::async_runtime::spawn(async move { rail::show(&app).await });
            }
            StagePage::QuickNotes => match self.window(page) {
                Some(_) => window::quick_notes::focus(app),
                None => window::quick_notes::open(app),
            },
        }
    }

    fn hide(&self, page: StagePage) {
        let app = &self.0;
        match page {
            StagePage::Hub => window::hide_main(app),
            StagePage::Settings => window::settings::hide(app),
            StagePage::Panel => window::panel::close(app, true),
            StagePage::Rail => rail::hide(app),
            StagePage::QuickNotes => window::quick_notes::hide(app),
        }
    }

    fn focus(&self, page: StagePage) {
        if page == StagePage::Panel {
            let app = self.0.clone();
            tauri::async_runtime::spawn(async move { window::panel::refocus(&app).await });
            return;
        }
        if let Some(w) = self.window(page) {
            let _ = w.unminimize();
            let _ = w.show();
            let _ = w.set_focus();
        }
    }

    fn place(&self, page: StagePage, rect: ScreenRect) {
        if let Some(w) = self.window(page) {
            let _ = w.set_position(tauri::LogicalPosition::new(rect.x as f64, rect.y as f64));
            let _ = w.set_size(tauri::LogicalSize::new(
                rect.width as f64,
                rect.height as f64,
            ));
        }
    }

    fn emit(&self, pages: &[StagePage], event: &str, payload: Value) {
        for page in pages {
            // A window not built yet has no page to answer.
            if self.window(*page).is_some()
                && let Err(e) = self.0.emit_to(
                    EventTarget::webview_window(page.label()),
                    event,
                    payload.clone(),
                )
            {
                log::warn!("Couldn't send {event} to the {} window: {e}", page.as_str());
            }
        }
    }

    fn commands(&self) -> Vec<StageCommand> {
        let commands = self.0.state::<bus::Commands>();
        catalog::stage_commands(&commands, &self.0, &self.hotkeys())
    }

    fn command_status(&self, id: &str) -> CommandStatus {
        let commands = self.0.state::<bus::Commands>();
        catalog::command_status(&commands, &self.0, id)
    }

    fn run_command(&self, id: &str) -> impl Future<Output = Result<(), AutomationError>> + Send {
        let (app, id) = (self.0.clone(), id.to_string());
        async move { automation::command(&app, &id).await }
    }

    fn hotkeys(&self) -> Hotkeys {
        self.0.state::<AppState>().settings.get().hotkeys
    }

    fn state(&self) -> impl Future<Output = Map<String, Value>> + Send {
        let app = self.0.clone();
        async move {
            let state = automation::state::now(&app).await;
            stage_state(&state, app.has_vault())
        }
    }
}
