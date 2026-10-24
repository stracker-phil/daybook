//! The keys that run commands while a Daybook window is in front, in Settings notation. The app menu
//! takes its key equivalents from here. Pure.

pub struct MenuKey {
    pub command: &'static str,
    pub shortcut: &'static str,
}

const fn key(command: &'static str, shortcut: &'static str) -> MenuKey {
    MenuKey { command, shortcut }
}

pub const SETTINGS: MenuKey = key("app:settings", "cmd+,");
/// ⌘Q only closes the window in front; quitting is left to the menu-bar menu.
pub const CLOSE_WITH_Q: MenuKey = key("window:close", "cmd+q");
pub const PALETTE: MenuKey = key("palette:open", "cmd+k");
pub const SEARCH: MenuKey = key("search:open", "cmd+f");
pub const JOURNAL: MenuKey = key("journal:open", "cmd+j");
pub const DAYBOOK: MenuKey = key("hub:open", "cmd+0");
pub const CLOSE: MenuKey = key("window:close", "cmd+w");

pub const MENU_KEYS: [&MenuKey; 7] = [
    &SETTINGS,
    &CLOSE_WITH_Q,
    &PALETTE,
    &SEARCH,
    &JOURNAL,
    &DAYBOOK,
    &CLOSE,
];
