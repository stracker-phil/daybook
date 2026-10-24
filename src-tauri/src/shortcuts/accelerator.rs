//! Shortcut notation (`shift+cmd+k`, as Settings stores it) to the accelerator names menus and global
//! shortcuts parse. Pure.

fn key_name(key: &str) -> Option<&'static str> {
    Some(match key {
        "backspace" => "Backspace",
        "delete" => "Delete",
        "enter" | "return" => "Enter",
        "escape" => "Escape",
        "space" => "Space",
        "tab" => "Tab",
        "arrowup" => "Up",
        "arrowdown" => "Down",
        "arrowleft" => "Left",
        "arrowright" => "Right",
        _ => return None,
    })
}

fn modifier_name(modifier: &str) -> Option<&'static str> {
    Some(match modifier {
        "cmd" | "command" => "Command",
        "ctrl" | "control" => "Control",
        "alt" | "opt" | "option" => "Alt",
        "shift" => "Shift",
        _ => return None,
    })
}

/// `None` for an empty shortcut, one without a key, or an unknown modifier.
pub fn accelerator(shortcut: &str) -> Option<String> {
    let lower = shortcut.to_lowercase();
    let mut parts: Vec<&str> = lower.split('+').map(str::trim).collect();
    let key = parts.pop().filter(|key| !key.is_empty())?;
    let mut names = parts
        .into_iter()
        .map(|modifier| modifier_name(modifier).map(str::to_string))
        .collect::<Option<Vec<_>>>()?;
    names.push(key_name(key).map_or_else(|| key.to_uppercase(), str::to_string));
    Some(names.join("+"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::MENU_KEYS;

    #[test]
    fn shortcuts_become_the_names_the_runtime_parses() {
        for (shortcut, expected) in [
            ("ctrl+alt+cmd+j", Some("Control+Alt+Command+J")),
            ("Shift+CMD+Enter", Some("Shift+Command+Enter")),
            ("cmd+f2", Some("Command+F2")),
            ("opt+arrowup", Some("Alt+Up")),
            ("cmd+return", Some("Command+Enter")),
            ("cmd+,", Some("Command+,")),
            ("", None),
            ("cmd+", None),
            ("hyper+k", None),
        ] {
            assert_eq!(accelerator(shortcut).as_deref(), expected, "{shortcut}");
        }
    }

    #[test]
    fn every_menu_key_converts() {
        for key in MENU_KEYS {
            assert!(
                accelerator(key.shortcut).is_some(),
                "{} ({})",
                key.shortcut,
                key.command
            );
        }
    }
}
