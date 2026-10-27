//! One line of the menu-bar menu and the helpers that build lines. Pure.

#[derive(Clone, Debug, PartialEq)]
pub enum MenuEntry {
    Command {
        label: String,
        command: String,
        /// A global shortcut, shown only (Settings notation).
        shortcut: Option<String>,
        enabled: bool,
    },
    Radio {
        label: String,
        command: String,
        checked: bool,
    },
    /// An on/off setting; its command toggles it.
    Checkbox {
        label: String,
        command: String,
        checked: bool,
    },
    /// A greyed line that only tells something.
    Info(String),
    Submenu {
        label: String,
        items: Vec<MenuEntry>,
    },
    Separator,
}

impl MenuEntry {
    pub fn label(&self) -> Option<&str> {
        match self {
            Self::Command { label, .. }
            | Self::Radio { label, .. }
            | Self::Checkbox { label, .. }
            | Self::Info(label)
            | Self::Submenu { label, .. } => Some(label),
            Self::Separator => None,
        }
    }
}

/// A command with its global shortcut; an empty one shows none.
pub(super) fn command(label: &str, id: &str, shortcut: &str) -> MenuEntry {
    MenuEntry::Command {
        label: label.into(),
        command: id.into(),
        shortcut: (!shortcut.is_empty()).then(|| shortcut.to_string()),
        enabled: true,
    }
}

/// A command without a shortcut.
pub(super) fn plain(label: &str, id: &str) -> MenuEntry {
    command(label, id, "")
}

pub(super) fn submenu(label: impl Into<String>, items: Vec<MenuEntry>) -> MenuEntry {
    MenuEntry::Submenu {
        label: label.into(),
        items,
    }
}
