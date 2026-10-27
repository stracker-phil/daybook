//! Key combinations for `act kind=keys`: parsing any notation ("⌘⇧K", "cmd+shift+k", Settings'
//! "ctrl+alt+cmd+j"), one canonical spelling, and finding the global shortcut or command key a
//! combination is. Pure.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::menu::command_shortcuts::MenuKey;
use crate::shortcuts::HOTKEY_ACTIONS;
use crate::store::settings::Hotkeys;

/// A key combination as a page dispatches it (`KeyboardEvent` names).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct StageKeys {
    pub key: String,
    /// "KeyK", "Digit2", "Enter", "F2"; empty when there is none.
    pub code: String,
    pub meta: bool,
    pub shift: bool,
    pub alt: bool,
    pub ctrl: bool,
}

#[derive(Clone, Copy)]
enum Modifier {
    Meta,
    Shift,
    Alt,
    Ctrl,
}

impl StageKeys {
    fn set(&mut self, modifier: Modifier) {
        match modifier {
            Modifier::Meta => self.meta = true,
            Modifier::Shift => self.shift = true,
            Modifier::Alt => self.alt = true,
            Modifier::Ctrl => self.ctrl = true,
        }
    }
}

fn modifier_glyph(c: &str) -> Option<Modifier> {
    Some(match c {
        "⌘" => Modifier::Meta,
        "⇧" => Modifier::Shift,
        "⌥" => Modifier::Alt,
        "⌃" | "^" => Modifier::Ctrl,
        _ => return None,
    })
}

fn modifier_word(word: &str) -> Option<Modifier> {
    Some(match word {
        "cmd" | "command" | "meta" => Modifier::Meta,
        "shift" => Modifier::Shift,
        "alt" | "opt" | "option" => Modifier::Alt,
        "ctrl" | "control" => Modifier::Ctrl,
        _ => return None,
    })
}

fn named_key(name: &str) -> Option<(&'static str, &'static str)> {
    Some(match name {
        "enter" | "return" | "↵" | "↩" | "⏎" => ("Enter", "Enter"),
        "esc" | "escape" | "⎋" => ("Escape", "Escape"),
        "space" => (" ", "Space"),
        "tab" | "⇥" => ("Tab", "Tab"),
        "backspace" | "⌫" => ("Backspace", "Backspace"),
        "delete" | "⌦" => ("Delete", "Delete"),
        "up" | "arrowup" | "↑" => ("ArrowUp", "ArrowUp"),
        "down" | "arrowdown" | "↓" => ("ArrowDown", "ArrowDown"),
        "left" | "arrowleft" | "←" => ("ArrowLeft", "ArrowLeft"),
        "right" | "arrowright" | "→" => ("ArrowRight", "ArrowRight"),
        "home" => ("Home", "Home"),
        "end" => ("End", "End"),
        "pageup" => ("PageUp", "PageUp"),
        "pagedown" => ("PageDown", "PageDown"),
        _ => return None,
    })
}

/// The modifiers and the key's text: leading glyphs need no separator (⌘⇧K), words are joined with
/// `+` (cmd+shift+k). None when a modifier is unknown or no key is left.
fn split_modifiers(text: &str) -> Option<(StageKeys, &str)> {
    let mut keys = StageKeys::default();
    let mut rest = text.trim();
    while rest.chars().count() > 1 {
        let first = rest.chars().next()?;
        let Some(modifier) = modifier_glyph(&rest[..first.len_utf8()]) else {
            break;
        };
        keys.set(modifier);
        rest = &rest[first.len_utf8()..];
    }
    let mut parts: Vec<&str> = rest
        .split('+')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    // A lone "+" is the plus key, not a separator.
    let last = match parts.pop() {
        Some(last) => last,
        None if rest.contains('+') => "+",
        None => return None,
    };
    for part in parts {
        keys.set(modifier_word(&part.to_lowercase()).or_else(|| modifier_glyph(part))?);
    }
    Some((keys, last))
}

fn function_key(lower: &str) -> Option<u8> {
    let digits = lower.strip_prefix('f')?;
    if digits.starts_with('0') {
        return None;
    }
    digits.parse().ok().filter(|n| (1..=20).contains(n))
}

/// The key's `KeyboardEvent` `key` and `code`; letters are upper case with Shift.
fn parse_last(last: &str, shift: bool) -> Option<(String, String)> {
    let lower = last.to_lowercase();
    if let Some((key, code)) = named_key(&lower) {
        return Some((key.into(), code.into()));
    }
    if let Some(n) = function_key(&lower) {
        return Some((format!("F{n}"), format!("F{n}")));
    }
    let mut chars = last.chars();
    let (Some(c), None) = (chars.next(), chars.next()) else {
        return None;
    };
    if c.is_ascii_alphabetic() {
        let key = if shift {
            c.to_ascii_uppercase()
        } else {
            c.to_ascii_lowercase()
        };
        return Some((key.to_string(), format!("Key{}", c.to_ascii_uppercase())));
    }
    if c.is_ascii_digit() {
        return Some((c.to_string(), format!("Digit{c}")));
    }
    Some((c.to_string(), String::new()))
}

/// "⌘⇧K", "cmd+shift+k", "Enter" or "⌃⌥⌘J" as a key combination; None when it isn't one.
pub fn parse_keys(text: &str) -> Option<StageKeys> {
    let (keys, last) = split_modifiers(text)?;
    let (key, code) = parse_last(last, keys.shift)?;
    Some(StageKeys { key, code, ..keys })
}

/// One spelling per combination, so "⌘F2" and "cmd+f2" compare equal: `ctrl+alt+shift+cmd+<key>`.
pub fn canonical_keys(keys: &StageKeys) -> String {
    let name = if let Some(letter) = keys.code.strip_prefix("Key") {
        letter.to_lowercase()
    } else if let Some(digit) = keys.code.strip_prefix("Digit") {
        digit.to_string()
    } else if keys.key == " " {
        "space".into()
    } else {
        keys.key.to_lowercase()
    };
    [
        keys.ctrl.then_some("ctrl"),
        keys.alt.then_some("alt"),
        keys.shift.then_some("shift"),
        keys.meta.then_some("cmd"),
        Some(name.as_str()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join("+")
}

/// A shortcut in any notation, spelled canonically; as given when it doesn't parse.
pub fn notation(shortcut: &str) -> String {
    parse_keys(shortcut).map_or_else(|| shortcut.to_string(), |keys| canonical_keys(&keys))
}

/// Whether the combination is this shortcut, in any notation.
pub fn is_shortcut(keys: &StageKeys, shortcut: &str) -> bool {
    parse_keys(shortcut).is_some_and(|parsed| canonical_keys(&parsed) == canonical_keys(keys))
}

/// The global shortcut (its action, e.g. `journal`) this combination is; an empty shortcut is none.
pub fn hotkey_for(keys: &StageKeys, hotkeys: &Hotkeys) -> Option<&'static str> {
    HOTKEY_ACTIONS
        .iter()
        .find(|hotkey| {
            let shortcut = (hotkey.shortcut)(hotkeys);
            !shortcut.is_empty() && is_shortcut(keys, shortcut)
        })
        .map(|hotkey| hotkey.action)
}

/// The command whose key equivalent this combination is.
pub fn command_for_keys(keys: &StageKeys, shortcuts: &[&MenuKey]) -> Option<&'static str> {
    shortcuts
        .iter()
        .find(|key| is_shortcut(keys, key.shortcut))
        .map(|key| key.command)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::command_shortcuts::COMMAND_SHORTCUTS;

    fn keys(text: &str) -> StageKeys {
        parse_keys(text).unwrap_or_else(|| panic!("{text} should parse"))
    }

    fn combination(key: &str, code: &str, modifiers: &str) -> StageKeys {
        StageKeys {
            key: key.into(),
            code: code.into(),
            meta: modifiers.contains('m'),
            shift: modifiers.contains('s'),
            alt: modifiers.contains('a'),
            ctrl: modifiers.contains('c'),
        }
    }

    #[test]
    fn glyphs_and_words_parse_to_the_same_combination() {
        let expected = combination("K", "KeyK", "ms");

        assert_eq!(keys("⌘⇧K"), expected);
        assert_eq!(keys("cmd+shift+k"), expected);
    }

    #[test]
    fn modifiers_have_several_spellings() {
        for (text, modifiers) in [
            ("cmd+k", "m"),
            ("command+k", "m"),
            ("meta+k", "m"),
            ("opt+k", "a"),
            ("option+k", "a"),
            ("alt+k", "a"),
            ("control+k", "c"),
            ("ctrl+k", "c"),
            ("^k", "c"),
            ("⌥k", "a"),
            ("⌃⌥⌘k", "mac"),
        ] {
            assert_eq!(keys(text), combination("k", "KeyK", modifiers), "{text}");
        }
    }

    #[test]
    fn digits_function_keys_and_symbols_get_their_codes() {
        assert_eq!(keys("⌘2"), combination("2", "Digit2", "m"));
        assert_eq!(keys("⌘F2"), combination("F2", "F2", "m"));
        assert_eq!(keys("f20"), combination("F20", "F20", ""));
        assert_eq!(keys("⌘,"), combination(",", "", "m"));
        assert_eq!(keys("+"), combination("+", "", ""));
    }

    #[test]
    fn named_keys_parse_in_any_case_and_spelling() {
        for (text, key, code) in [
            ("Enter", "Enter", "Enter"),
            ("return", "Enter", "Enter"),
            ("↩", "Enter", "Enter"),
            ("esc", "Escape", "Escape"),
            ("Escape", "Escape", "Escape"),
            ("space", " ", "Space"),
            ("Tab", "Tab", "Tab"),
            ("backspace", "Backspace", "Backspace"),
            ("Delete", "Delete", "Delete"),
            ("Down", "ArrowDown", "ArrowDown"),
            ("up", "ArrowUp", "ArrowUp"),
            ("←", "ArrowLeft", "ArrowLeft"),
            ("arrowright", "ArrowRight", "ArrowRight"),
            ("PageDown", "PageDown", "PageDown"),
            ("home", "Home", "Home"),
        ] {
            assert_eq!(keys(text), combination(key, code, ""), "{text}");
        }
    }

    #[test]
    fn a_letter_is_upper_case_only_with_shift() {
        assert_eq!(keys("k").key, "k");
        assert_eq!(keys("shift+k").key, "K");
        assert_eq!(keys("⇧k").key, "K");
    }

    #[test]
    fn text_that_is_not_a_combination_does_not_parse() {
        for text in [
            "", "   ", "cmd+", "nonsense", "foo+k", "f0", "f21", "f02", "cmd+k+j",
        ] {
            assert_eq!(parse_keys(text), None, "{text:?}");
        }
    }

    #[test]
    fn one_spelling_is_canonical() {
        for (text, canonical) in [
            ("⌘F2", "cmd+f2"),
            ("shift+cmd+Enter", "shift+cmd+enter"),
            ("⌃⌥⇧⌘J", "ctrl+alt+shift+cmd+j"),
            ("⌘2", "cmd+2"),
            ("space", "space"),
            ("⌘,", "cmd+,"),
            ("Down", "arrowdown"),
        ] {
            assert_eq!(canonical_keys(&keys(text)), canonical, "{text}");
        }
    }

    #[test]
    fn a_shortcut_is_recognized_in_any_notation() {
        for (shortcut, text, expected) in [
            ("cmd+f2", "⌘F2", true),
            ("ctrl+alt+cmd+j", "⌃⌥⌘J", true),
            ("cmd+k", "cmd+shift+k", false),
            ("cmd+k", "k", false),
            ("not a shortcut", "k", false),
            ("", "k", false),
        ] {
            assert_eq!(
                is_shortcut(&keys(text), shortcut),
                expected,
                "{shortcut} {text}"
            );
        }
    }

    #[test]
    fn a_shortcut_is_notated_canonically_or_kept_as_given() {
        assert_eq!(notation("⌘F2"), "cmd+f2");
        assert_eq!(notation("ctrl+cmd+i"), "ctrl+cmd+i");
        assert_eq!(notation("not a shortcut"), "not a shortcut");
    }

    #[test]
    fn a_menu_key_equivalent_finds_its_command_in_any_notation() {
        for (text, command) in [
            ("⌘,", Some("app:settings")),
            ("cmd+k", Some("palette:open")),
            ("⌃⌘I", Some("view:toggleInspector")),
            ("⌘W", Some("window:close")),
            ("⌘Q", Some("window:close")),
            ("⌘0", Some("hub:open")),
            ("Esc", None),
            ("⌘⇧K", None),
        ] {
            assert_eq!(
                command_for_keys(&keys(text), &COMMAND_SHORTCUTS),
                command,
                "{text}"
            );
        }
    }

    #[test]
    fn a_global_shortcut_is_found_by_its_action() {
        let hotkeys = Hotkeys {
            switch_activity: "cmd+f2".into(),
            journal: "ctrl+alt+cmd+j".into(),
            ..Hotkeys::none()
        };

        for (text, action) in [
            ("⌘F2", Some("switchActivity")),
            ("⌃⌥⌘J", Some("journal")),
            ("Enter", None),
            ("⌘F1", None),
        ] {
            assert_eq!(hotkey_for(&keys(text), &hotkeys), action, "{text}");
        }
    }

    #[test]
    fn a_cleared_global_shortcut_matches_nothing() {
        let hotkeys = Hotkeys::none();

        assert_eq!(hotkey_for(&keys("⌘F2"), &hotkeys), None);
        assert_eq!(hotkey_for(&StageKeys::default(), &hotkeys), None);
    }
}
