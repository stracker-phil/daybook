//! The menu-bar item: its title and the menu from `menu::model`, rebuilt whenever what it shows changed.
//! Item ids are command ids, so clicks go through `menu::clicked`.

use std::sync::Mutex;

use tauri::image::Image;
use tauri::menu::{
    CheckMenuItemBuilder, IsMenuItem, Menu, MenuItemBuilder, MenuItemKind, PredefinedMenuItem,
    SubmenuBuilder,
};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Manager, Wry};

use crate::bus;
use crate::lock::MutexExt;
use crate::menu::model::{MenuEntry, MenuInput, build_menu, menu_bar_title, prune};
use crate::shortcuts::{self, accelerator};
use crate::state::AppState;
use crate::tools::{self, ToolState};

const TRAY_ID: &str = "main";

/// A template image, 36 px for the menu bar's 18 pt icon height; macOS tints it for the menu bar's appearance.
const ICON: &[u8] = include_bytes!("../icons/tray/tray.png");

/// What the item shows now, so `refresh` only rebuilds when something changed.
#[derive(Default)]
pub struct Shown(Mutex<Option<Showing>>);

#[derive(Clone, PartialEq)]
struct Showing {
    title: String,
    entries: Vec<MenuEntry>,
}

pub fn install(app: &AppHandle) -> tauri::Result<()> {
    TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(ICON)?)
        .icon_as_template(true)
        .tooltip(crate::APP_NAME)
        .menu(&Menu::new(app)?)
        .build(app)?;
    refresh(app);
    Ok(())
}

fn entries(app: &AppHandle, state: &ToolState) -> Vec<MenuEntry> {
    let settings = app.state::<AppState>().settings.get();
    let hotkeys = shortcuts::shown(app);
    let menu = build_menu(&MenuInput {
        state,
        hotkeys: &hotkeys,
        tools: &settings.tools,
    });
    let commands = app.state::<bus::Commands>();
    prune(menu, &commands.available(app))
}

/// Rebuilds the title and menu from the current state.
pub fn refresh(app: &AppHandle) {
    let state = tools::snapshot(app);
    let next = Showing {
        title: menu_bar_title(&state),
        entries: entries(app, &state),
    };
    if !changed(app, &next) {
        return;
    }
    if let Err(e) = apply(app, next) {
        log::warn!("Couldn't update the menu bar item: {e}");
    }
}

/// Remembers `next`; false when the item already shows it.
fn changed(app: &AppHandle, next: &Showing) -> bool {
    let shown = app.state::<Shown>();
    let mut shown = shown.0.lock_unpoisoned();
    if shown.as_ref() == Some(next) {
        return false;
    }
    *shown = Some(next.clone());
    true
}

fn apply(app: &AppHandle, showing: Showing) -> tauri::Result<()> {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return Ok(());
    };
    let items = showing
        .entries
        .iter()
        .map(|entry| build_entry(app, entry))
        .collect::<tauri::Result<Vec<_>>>()?;
    let refs: Vec<&dyn IsMenuItem<Wry>> = items.iter().map(|item| item as _).collect();
    tray.set_menu(Some(Menu::with_items(app, &refs)?))?;
    tray.set_title(Some(showing.title))
}

fn build_entry(app: &AppHandle, entry: &MenuEntry) -> tauri::Result<MenuItemKind<Wry>> {
    Ok(match entry {
        MenuEntry::Separator => MenuItemKind::Predefined(PredefinedMenuItem::separator(app)?),
        MenuEntry::Info(label) => {
            MenuItemKind::MenuItem(MenuItemBuilder::new(label).enabled(false).build(app)?)
        }
        MenuEntry::Command {
            label,
            command,
            shortcut,
            enabled,
        } => {
            let mut builder = MenuItemBuilder::with_id(command, label).enabled(*enabled);
            // Shown only: the global shortcut, not the menu, runs it.
            if let Some(accel) = shortcut.as_deref().and_then(accelerator) {
                builder = builder.accelerator(accel);
            }
            MenuItemKind::MenuItem(builder.build(app)?)
        }
        MenuEntry::Radio {
            label,
            command,
            checked,
        } => MenuItemKind::Check(
            CheckMenuItemBuilder::with_id(command, label)
                .checked(*checked)
                .build(app)?,
        ),
        MenuEntry::Submenu { label, items } => {
            let mut builder = SubmenuBuilder::new(app, label);
            for item in items {
                builder = builder.item(&build_entry(app, item)?);
            }
            MenuItemKind::Submenu(builder.build()?)
        }
    })
}
