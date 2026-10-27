//! The overlay's Tauri commands: closing it and the page's answers.

use tauri::{AppHandle, State};

use super::Panel;
use super::session::PanelSession;
use crate::lock::MutexExt;

#[tauri::command]
pub fn panel_close(app: AppHandle) {
    super::close(&app, false);
}

/// The session to render, `None` while closed; a page that loads after the session was sent asks.
#[tauri::command]
pub fn panel_session(panel: State<'_, Panel>) -> Option<PanelSession> {
    panel.state.lock_unpoisoned().session().cloned()
}

/// The page rendered `session`; `height` is its content height.
#[tauri::command]
pub fn panel_ready(panel: State<'_, Panel>, session: u32, height: f64) {
    panel.ready.answer(session, height);
}

/// The page painted `session`'s first frame.
#[tauri::command]
pub fn panel_painted(panel: State<'_, Panel>, session: u32) {
    panel.painted.answer(session, ());
}

/// The open overlay stays while focus moves elsewhere (`true`), or closes on focus loss again.
#[tauri::command]
pub async fn panel_keep_open(app: AppHandle, keep: bool) {
    super::keep_open(&app, keep).await;
}

#[tauri::command]
pub async fn panel_set_height(app: AppHandle, height: f64) {
    super::set_height(&app, height).await;
}
