//! User shortcut overrides (`keybindings.json`): `{ "commandId": "Mod+Shift+X" | null }` for the
//! keys the page resolves.

use std::collections::BTreeMap;
use std::path::Path;

use super::json_store::JsonStore;
use crate::error::AppResult;

/// `None` means the user unbound the command.
pub type Keybindings = BTreeMap<String, Option<String>>;

pub struct KeybindingsStore {
    store: JsonStore<Keybindings>,
}

impl KeybindingsStore {
    pub fn open(dir: &Path) -> Self {
        Self {
            store: JsonStore::open_with(dir.join("keybindings.json"), sanitize),
        }
    }

    pub fn get(&self) -> Keybindings {
        self.store.read(Keybindings::clone)
    }

    /// Replaces all overrides; malformed entries are dropped (see `sanitize`).
    pub async fn update(&self, bindings: Keybindings) -> AppResult<Keybindings> {
        let bindings = sanitize(bindings);
        self.store
            .update_async(move |current| {
                *current = bindings.clone();
                bindings
            })
            .await
    }
}

const MODIFIERS: &[&str] = &["Mod", "Ctrl", "Alt", "Shift"];

/// Drops overrides whose command id or binding the webview could never match. Ids are not
/// checked against the page's registry, which Rust does not know.
pub fn sanitize(bindings: Keybindings) -> Keybindings {
    bindings
        .into_iter()
        .filter(|(id, binding)| {
            let valid = is_command_id(id) && binding.as_deref().is_none_or(|b| split(b).is_some());
            if !valid {
                log::warn!("Ignoring key binding {id:?} = {binding:?}");
            }
            valid
        })
        .collect()
}

fn is_command_id(id: &str) -> bool {
    !id.is_empty() && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
}

/// `Mod+Shift+K` into its modifiers and key. A trailing `++` (or a bare `+`) is the `+` key.
fn split(binding: &str) -> Option<(Vec<&str>, &str)> {
    let (modifiers, key) = if binding == "+" {
        ("", "+")
    } else if let Some(modifiers) = binding.strip_suffix("++") {
        (modifiers, "+")
    } else {
        binding.rsplit_once('+').unwrap_or(("", binding))
    };
    let modifiers: Vec<&str> = match modifiers {
        "" => Vec::new(),
        some => some.split('+').collect(),
    };
    let modifiers_valid = modifiers.iter().all(|m| MODIFIERS.contains(m));
    let single_char = key.chars().count() == 1 && !key.trim().is_empty();
    let named_key = !key.is_empty() && key.chars().all(|c| c.is_ascii_alphanumeric());
    let key_valid = !MODIFIERS.contains(&key) && (single_char || named_key);
    (modifiers_valid && key_valid).then_some((modifiers, key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_config_dir;

    fn overrides(entries: &[(&str, Option<&str>)]) -> Keybindings {
        entries
            .iter()
            .map(|(id, binding)| (id.to_string(), binding.map(String::from)))
            .collect()
    }

    #[test]
    fn without_a_file_there_are_no_overrides() {
        let dir = temp_config_dir();
        assert_eq!(KeybindingsStore::open(dir.path()).get(), Keybindings::new());
    }

    #[tokio::test]
    async fn override_persists_across_reopen() {
        let dir = temp_config_dir();
        KeybindingsStore::open(dir.path())
            .update(overrides(&[("settings.toggle", Some("Mod+Shift+P"))]))
            .await
            .unwrap();
        assert_eq!(
            KeybindingsStore::open(dir.path()).get(),
            overrides(&[("settings.toggle", Some("Mod+Shift+P"))])
        );
    }

    #[tokio::test]
    async fn null_override_persists_as_an_unbound_command() {
        let dir = temp_config_dir();
        let store = KeybindingsStore::open(dir.path());
        store
            .update(overrides(&[("window.hide", None)]))
            .await
            .unwrap();
        let expected = overrides(&[("window.hide", None)]);
        assert_eq!(store.get(), expected);
        assert_eq!(KeybindingsStore::open(dir.path()).get(), expected);
    }

    #[test]
    fn sanitize_drops_bad_syntax_and_bad_ids() {
        let cleaned = sanitize(overrides(&[
            ("a.bad", Some("Mod+")),
            ("b.bad", Some("Hyper+K")),
            ("c.bad", Some("Mod+K+J")),
            ("bad id!", Some("Mod+K")),
            ("", Some("Mod+K")),
            ("settings.toggle", Some("Mod+Shift+P")),
            ("window.hide", None),
            ("zoom.in", Some("Mod++")),
        ]));
        assert_eq!(
            cleaned,
            overrides(&[
                ("settings.toggle", Some("Mod+Shift+P")),
                ("window.hide", None),
                ("zoom.in", Some("Mod++")),
            ])
        );
    }

    #[tokio::test]
    async fn update_returns_the_sanitized_map_and_persists_it() {
        let dir = temp_config_dir();
        let store = KeybindingsStore::open(dir.path());
        let saved = store
            .update(overrides(&[
                ("settings.toggle", Some("Mod+Shift+P")),
                ("window.hide", Some("Hyper+K")),
            ]))
            .await
            .unwrap();
        let expected = overrides(&[("settings.toggle", Some("Mod+Shift+P"))]);
        assert_eq!(saved, expected);
        assert_eq!(KeybindingsStore::open(dir.path()).get(), expected);
    }

    #[test]
    fn hand_edited_file_with_a_malformed_entry_loads_without_it() {
        let dir = temp_config_dir();
        std::fs::write(
            dir.path().join("keybindings.json"),
            r#"{"settings.toggle": "Mod+", "window.hide": "Mod+Q"}"#,
        )
        .unwrap();
        let store = KeybindingsStore::open(dir.path());
        assert_eq!(store.get(), overrides(&[("window.hide", Some("Mod+Q"))]));
    }
}
