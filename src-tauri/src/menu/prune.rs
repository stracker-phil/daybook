//! Fitting the menu to the commands that exist now. Pure.

use super::entry::MenuEntry;
use crate::bus::CommandStatus;

/// Drops separators at the start, at the end and in a row, and empty submenus.
pub(super) fn tidy(entries: Vec<MenuEntry>) -> Vec<MenuEntry> {
    let mut result: Vec<MenuEntry> = Vec::new();
    for entry in entries {
        match entry {
            MenuEntry::Separator
                if result
                    .last()
                    .is_none_or(|last| matches!(last, MenuEntry::Separator)) => {}
            MenuEntry::Submenu { items, .. } if items.is_empty() => {}
            entry => result.push(entry),
        }
    }
    while matches!(result.last(), Some(MenuEntry::Separator)) {
        result.pop();
    }
    result
}

/// Leaves out what runs unknown commands, greys out disabled ones, then tidies what is left.
pub fn prune(entries: Vec<MenuEntry>, commands: &dyn CommandStatus) -> Vec<MenuEntry> {
    let kept = entries
        .into_iter()
        .filter_map(|entry| match entry {
            MenuEntry::Command {
                label,
                command,
                shortcut,
                enabled,
            } => commands.knows(&command).then(|| MenuEntry::Command {
                enabled: enabled && commands.is_enabled(&command),
                label,
                command,
                shortcut,
            }),
            MenuEntry::Radio { ref command, .. } | MenuEntry::Checkbox { ref command, .. } => {
                commands.knows(command).then_some(entry)
            }
            MenuEntry::Submenu { label, items } => {
                let items = prune(items, commands);
                let actionable = items
                    .iter()
                    .any(|item| !matches!(item, MenuEntry::Info(_) | MenuEntry::Separator));
                actionable.then_some(MenuEntry::Submenu { label, items })
            }
            other => Some(other),
        })
        .collect();
    tidy(kept)
}
