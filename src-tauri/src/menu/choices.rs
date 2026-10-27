//! A tool's choices as menu lines, and the submenu a tool with choices becomes. Pure.

use super::entry::{MenuEntry, plain, submenu};
use super::model::MenuInput;
use crate::store::settings::ToolId;
use crate::tools::{Choice, Step, tool_choices};

fn steps_submenu(label: String, steps: Vec<Step>) -> MenuEntry {
    submenu(
        label,
        steps
            .into_iter()
            .map(|step| MenuEntry::Radio {
                label: format!("{} %", step.label),
                command: step.command,
                checked: step.active,
            })
            .collect(),
    )
}

/// `Label · detail`; a radio item for a pick among several.
fn choice_item(
    label: String,
    id: String,
    detail: Option<String>,
    checked: Option<bool>,
) -> MenuEntry {
    let label = match detail {
        Some(detail) => format!("{label} · {detail}"),
        None => label,
    };
    match checked {
        Some(checked) => MenuEntry::Radio {
            label,
            command: id,
            checked,
        },
        None => plain(&label, &id),
    }
}

fn choices_to_menu(choices: Vec<Choice>) -> Vec<MenuEntry> {
    choices
        .into_iter()
        .map(|choice| match choice {
            Choice::Separator => MenuEntry::Separator,
            Choice::Heading { label } => MenuEntry::Info(label),
            Choice::Steps { label, steps } => steps_submenu(label, steps),
            Choice::Item {
                label,
                command,
                detail,
                checked,
            } => choice_item(label, command, detail, checked),
        })
        .collect()
}

pub(super) fn choices_of(id: ToolId, input: &MenuInput) -> Vec<MenuEntry> {
    choices_to_menu(tool_choices(id, input.state))
}

/// What opens the overlay, then the choices: the submenu's items.
pub(super) fn overlay_then(overlay: MenuEntry, choices: Vec<MenuEntry>) -> Vec<MenuEntry> {
    let mut items = vec![overlay, MenuEntry::Separator];
    items.extend(choices);
    items
}

/// A tool with choices becomes a submenu: what opens its overlay first, then its choices.
pub(super) fn with_choices(
    label: &str,
    overlay: MenuEntry,
    id: ToolId,
    input: &MenuInput,
) -> Vec<MenuEntry> {
    let choices = choices_of(id, input);
    if choices.is_empty() {
        return vec![overlay];
    }
    vec![submenu(label, overlay_then(overlay, choices))]
}
