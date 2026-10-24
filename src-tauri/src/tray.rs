//! The menu bar item: Open and Quit.

use tauri::AppHandle;
use tauri::image::Image;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;

use crate::window;

const TRAY_ID: &str = "main";
const OPEN_ID: &str = "tray.open";
const QUIT_ID: &str = "tray.quit";

/// A template image, 36 px for the menu bar's 18 pt icon height; macOS tints it for the menu bar's appearance.
const ICON: &[u8] = include_bytes!("../icons/tray/tray.png");

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    let menu = Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, OPEN_ID, "Open Daybook", true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, QUIT_ID, "Quit Daybook", true, None::<&str>)?,
        ],
    )?;
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON)?)
        .icon_as_template(true)
        .tooltip(crate::APP_NAME)
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            OPEN_ID => window::show_main(app),
            QUIT_ID => app.exit(0),
            _ => {}
        })
        .build(app)?;
    Ok(())
}
