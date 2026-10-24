//! The app menu (Daybook, File, Edit, Window, Help) and clicks on any menu item: every item's id is the
//! command it runs. The menu-bar menu's model is in `model`.

pub mod command_shortcuts;
pub mod model;
mod prune;
mod title;

pub use command_shortcuts::MENU_KEYS;

use tauri::menu::{AboutMetadata, Menu, MenuItem, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Manager, Wry};

use self::command_shortcuts::{
    CLOSE, CLOSE_WITH_Q, DAYBOOK, JOURNAL, MenuKey, PALETTE, SEARCH, SETTINGS,
};
use crate::bus::{self, CommandStatus};
use crate::shortcuts::{self, accelerator};
use crate::tray;

/// What one item runs and with which key.
struct Item {
    label: &'static str,
    command: &'static str,
    shortcut: Option<&'static str>,
}

const fn keyed(label: &'static str, key: &MenuKey) -> Item {
    Item {
        label,
        command: key.command,
        shortcut: Some(key.shortcut),
    }
}

const fn plain(label: &'static str, command: &'static str) -> Item {
    Item {
        label,
        command,
        shortcut: None,
    }
}

/// `None` for a command no feature registered.
fn build_item(
    app: &AppHandle,
    commands: &dyn CommandStatus,
    item: &Item,
    keys: bool,
) -> tauri::Result<Option<MenuItem<Wry>>> {
    if !commands.knows(item.command) {
        return Ok(None);
    }
    let mut builder = MenuItemBuilder::with_id(item.command, item.label)
        .enabled(commands.is_enabled(item.command));
    if keys && let Some(accel) = item.shortcut.and_then(accelerator) {
        builder = builder.accelerator(accel);
    }
    builder.build(app).map(Some)
}

/// A submenu of the items whose commands exist, with a separator between non-empty groups.
fn submenu(
    app: &AppHandle,
    commands: &dyn CommandStatus,
    title: &str,
    groups: &[&[Item]],
    keys: bool,
) -> tauri::Result<tauri::menu::Submenu<Wry>> {
    let mut builder = SubmenuBuilder::new(app, title);
    let mut filled = false;
    for group in groups {
        let mut items = Vec::new();
        for item in *group {
            items.extend(build_item(app, commands, item, keys)?);
        }
        if items.is_empty() {
            continue;
        }
        if filled {
            builder = builder.separator();
        }
        for item in &items {
            builder = builder.item(item);
        }
        filled = true;
    }
    builder.build()
}

fn help_items() -> Vec<Item> {
    let mut items = vec![
        plain("Open Log Folder", "app:logs"),
        plain("README", "app:readme"),
    ];
    if cfg!(debug_assertions) {
        items.push(plain("Sample Overlay", "sample:open"));
        items.push(plain("Sample Overlay (Sticky)", "sample:sticky"));
    }
    items
}

/// About, Settings…, the services and hiding, Close Window (⌘Q).
fn daybook_submenu(
    app: &AppHandle,
    commands: &dyn CommandStatus,
    settings: &Item,
) -> tauri::Result<tauri::menu::Submenu<Wry>> {
    let mut daybook = SubmenuBuilder::new(app, crate::APP_NAME)
        .about(Some(AboutMetadata::default()))
        .separator();
    if let Some(item) = build_item(app, commands, settings, true)? {
        daybook = daybook.item(&item).separator();
    }
    daybook = daybook
        .services()
        .separator()
        .hide()
        .hide_others()
        .show_all()
        .separator();
    if let Some(item) = build_item(app, commands, &keyed("Close Window", &CLOSE_WITH_Q), true)? {
        daybook = daybook.item(&item);
    }
    daybook.build()
}

pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let registry = app.state::<bus::Commands>();
    let commands = registry.available(app);
    let settings = keyed("Settings…", &SETTINGS);
    if shortcuts::paused(app) {
        // Native items always carry their keys (⌘H, ⌘C…), so they are left out while a shortcut is
        // recorded.
        let daybook = submenu(
            app,
            &commands,
            crate::APP_NAME,
            &[std::slice::from_ref(&settings)],
            false,
        )?;
        return Menu::with_items(app, &[&daybook]);
    }
    let daybook = daybook_submenu(app, &commands, &settings)?;
    let file = submenu(
        app,
        &commands,
        "File",
        &[
            &[
                keyed("Command Palette…", &PALETTE),
                keyed("Search Vault…", &SEARCH),
                keyed("Journal…", &JOURNAL),
                keyed("Daybook", &DAYBOOK),
            ],
            &[keyed("Close Window", &CLOSE)],
        ],
        true,
    )?;
    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;
    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        .bring_all_to_front()
        .build()?;
    let help = submenu(app, &commands, "Help", &[&help_items()], true)?;
    Menu::with_items(app, &[&daybook, &file, &edit, &window, &help])
}

/// Rebuilds the app menu, e.g. without key equivalents while a shortcut is recorded.
pub fn rebuild(app: &AppHandle) {
    if let Err(e) = build(app).and_then(|menu| app.set_menu(menu)) {
        log::warn!("Couldn't rebuild the menu: {e}");
    }
}

/// A click on any menu item, the app menu's or the menu-bar menu's.
pub fn clicked(app: &AppHandle, id: &str) {
    if !app.state::<bus::Commands>().knows(id) {
        return;
    }
    bus::spawn(app, id);
    // macOS ticked a radio item itself; the menu shows the state once the command changed it.
    tray::refresh(app);
}
