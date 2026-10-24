//! The menu items that run app commands: one entry per command id, mirrored by `menu: true` on the
//! TypeScript command in `src/commands`.

pub struct MenuItemDef {
    pub id: &'static str,
    pub label: &'static str,
    /// Mirrors the `defaultBinding` in `src/commands`; empty means none.
    pub default_binding: &'static str,
    /// A checkbox item; the page reports which are checked.
    pub check: bool,
}

const fn item(id: &'static str, label: &'static str, default_binding: &'static str) -> MenuItemDef {
    MenuItemDef {
        id,
        label,
        default_binding,
        check: false,
    }
}

const fn check(
    id: &'static str,
    label: &'static str,
    default_binding: &'static str,
) -> MenuItemDef {
    MenuItemDef {
        check: true,
        ..item(id, label, default_binding)
    }
}

pub const MENU_ITEMS: &[MenuItemDef] = &[
    // Daybook
    item("settings.toggle", "Settings…", "Mod+,"),
    check("app.launchAtLogin", "Launch at Login", ""),
    // ⌘Q only hides: the app keeps running in the menu bar until Quit is chosen.
    item("app.background", "Hide to Menu Bar", "Mod+Q"),
    item("app.quit", "Quit Daybook", ""),
    // File
    item("window.hide", "Close Window", "Mod+W"),
    // View
    item("sidebar.toggle", "Toggle Sidebar", "Mod+Ctrl+S"),
    // Help
    item("shortcuts.show", "Keyboard Shortcuts", "Shift+?"),
    item("palette.toggle", "Command Palette…", "Mod+K"),
    item("logs.open", "Open Log Folder", ""),
    item("help.readme", "README", ""),
];

pub fn menu_item(id: &str) -> Option<&'static MenuItemDef> {
    MENU_ITEMS.iter().find(|def| def.id == id)
}

pub fn is_menu_command(id: &str) -> bool {
    menu_item(id).is_some()
}

/// The default binding of a command a menu item shows.
pub fn default_binding(command: &str) -> Option<&'static str> {
    menu_item(command)
        .map(|def| def.default_binding)
        .filter(|b| !b.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::keybindings::{accelerator, is_chord};

    const DOCUMENTED_IDS: &[&str] = &[
        "settings.toggle",
        "app.launchAtLogin",
        "app.background",
        "app.quit",
        "window.hide",
        "sidebar.toggle",
        "shortcuts.show",
        "palette.toggle",
        "logs.open",
        "help.readme",
    ];

    #[test]
    fn every_documented_menu_command_is_a_menu_item() {
        for id in DOCUMENTED_IDS {
            assert!(menu_item(id).is_some(), "{id}");
        }
        assert_eq!(MENU_ITEMS.len(), DOCUMENTED_IDS.len());
    }

    #[test]
    fn only_listed_ids_are_menu_commands() {
        for item in MENU_ITEMS {
            assert!(is_menu_command(item.id), "{}", item.id);
        }
        assert!(!is_menu_command("nope.unknown"));
        assert!(!is_menu_command(""));
    }

    #[test]
    fn no_two_items_share_a_default_binding() {
        let mut bindings: Vec<&str> = MENU_ITEMS
            .iter()
            .map(|def| def.default_binding)
            .filter(|b| !b.is_empty())
            .collect();
        bindings.sort_unstable();
        let before = bindings.len();
        bindings.dedup();
        assert_eq!(bindings.len(), before);
    }

    #[test]
    fn default_chords_convert_to_accelerators() {
        for def in MENU_ITEMS.iter().filter(|d| is_chord(d.default_binding)) {
            assert!(
                accelerator(def.default_binding).is_some(),
                "{}: {}",
                def.id,
                def.default_binding
            );
        }
    }

    #[test]
    fn default_binding_is_none_for_unbound_and_unknown_commands() {
        assert_eq!(default_binding("settings.toggle"), Some("Mod+,"));
        assert_eq!(default_binding("logs.open"), None);
        assert_eq!(default_binding("unknown.command"), None);
    }

    #[test]
    fn mod_q_hides_to_the_menu_bar_and_quit_has_no_default_binding() {
        assert_eq!(default_binding("app.background"), Some("Mod+Q"));
        assert_eq!(default_binding("app.quit"), None);
    }

    #[test]
    fn only_launch_at_login_is_a_check_item() {
        let checks: Vec<&str> = MENU_ITEMS
            .iter()
            .filter(|def| def.check)
            .map(|def| def.id)
            .collect();
        assert_eq!(checks, ["app.launchAtLogin"]);
    }
}
