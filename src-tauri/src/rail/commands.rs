//! The rail pages' commands and `rail:toggle` on the bus.

use tauri::AppHandle;

use super::controller::RailShown;
use super::geometry::Frame;
use super::model::RailModel;
use super::tip_window;
use crate::bus::{Command, CommandBus};
use crate::store::settings::ToolId;

/// The buttons to draw; later changes come as `rail_model`.
#[tauri::command]
pub async fn rail_model(app: AppHandle) -> RailModel {
    super::current_model(&app)
}

/// Whether the rail is shown, on which side, kept; later changes come as `rail_shown`.
#[tauri::command]
pub async fn rail_shown_state(app: AppHandle) -> RailShown {
    super::shown(&app)
}

/// The bar's measured height in points.
#[tauri::command]
pub async fn rail_set_height(app: AppHandle, height: f64) {
    super::set_height(&app, height);
}

/// Button `tool` is hovered; `center` is its middle, in points below the rail window's top.
#[tauri::command]
pub async fn rail_show_tip(app: AppHandle, tool: ToolId, center: f64) {
    tip_window::show_tip(&app, tool, center);
}

/// No button is hovered any more.
#[tauri::command]
pub async fn rail_hide_tip(app: AppHandle) {
    tip_window::leave_tip(&app);
}

/// The tooltip page drew card `sequence` at `card` (window coordinates).
#[tauri::command]
pub async fn rail_tip_rendered(app: AppHandle, sequence: u32, card: Frame) {
    tip_window::tip_rendered(&app, sequence, card);
}

/// A choice picked in the tooltip; only the choices on show run.
#[tauri::command]
pub async fn rail_tip_run(app: AppHandle, command: String) {
    tip_window::pick(&app, &command).await;
}

/// A right-click: the menu-bar menu at the pointer. The rail stays while it is open.
#[tauri::command]
pub async fn rail_context_menu(app: AppHandle) {
    super::context_menu(&app).await;
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("rail:toggle", "Show or Hide the Rail")
            .help("Show or hide the rail")
            .needs_vault()
            .run(|app| async move {
                super::toggle(&app).await;
                Ok(())
            }),
    );
}
