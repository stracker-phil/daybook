use std::path::Path;

use super::Settings;
use crate::error::AppResult;
use crate::store::json_store::JsonStore;

const FILE_NAME: &str = "settings.json";

pub struct SettingsStore {
    store: JsonStore<Settings>,
}

impl SettingsStore {
    pub fn open(dir: &Path) -> Self {
        Self {
            store: JsonStore::open_with(dir.join(FILE_NAME), Settings::sanitized),
        }
    }

    pub fn get(&self) -> Settings {
        self.store.read(Settings::clone)
    }

    /// Returns the settings before the change and the sanitized settings that were saved.
    pub async fn update(&self, settings: Settings) -> AppResult<(Settings, Settings)> {
        self.store
            .update_async(move |current| {
                let previous = std::mem::replace(current, settings.sanitized());
                (previous, current.clone())
            })
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::tests::settings;
    use crate::store::settings::*;
    use crate::test_support::temp_config_dir;

    fn write_settings(dir: &Path, raw: &str) {
        std::fs::write(dir.join(FILE_NAME), raw).unwrap();
    }

    fn written(dir: &Path) -> serde_json::Value {
        serde_json::from_str(&std::fs::read_to_string(dir.join(FILE_NAME)).unwrap()).unwrap()
    }

    fn dir_entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    fn corrupt_backups(dir: &Path) -> Vec<String> {
        dir_entries(dir)
            .into_iter()
            .filter(|name| name.starts_with(&format!("{FILE_NAME}.")) && name.ends_with(".corrupt"))
            .collect()
    }

    #[test]
    fn a_missing_null_or_invalid_dark_scheme_loads_as_dark_and_keeps_the_light_scheme() {
        for raw in [
            r#"{"lightScheme": "light"}"#,
            r#"{"lightScheme": "light", "darkScheme": null}"#,
            r#"{"lightScheme": "light", "darkScheme": "nord"}"#,
            r#"{"lightScheme": "light", "darkScheme": 7}"#,
        ] {
            let dir = temp_config_dir();
            write_settings(dir.path(), raw);

            let loaded = SettingsStore::open(dir.path()).get();

            assert_eq!(
                (loaded.light_scheme, loaded.dark_scheme),
                (Scheme::Light, Scheme::Dark),
                "{raw}"
            );
        }
    }

    #[test]
    fn unknown_enum_values_fall_back_to_defaults_without_quarantining_the_file() {
        let dir = temp_config_dir();
        let raw = r#"{"lightScheme": "neon", "darkScheme": "light", "textSize": "huge", "accent": "teal"}"#;
        write_settings(dir.path(), raw);

        let loaded = SettingsStore::open(dir.path()).get();

        assert_eq!(
            loaded,
            settings(|s| {
                s.dark_scheme = Scheme::Light;
                s.accent = "teal".into();
            })
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap(),
            raw
        );
        assert_eq!(dir_entries(dir.path()), vec![FILE_NAME.to_string()]);
    }

    #[test]
    fn partial_file_fills_missing_fields_with_defaults() {
        let dir = temp_config_dir();
        write_settings(dir.path(), r#"{"accent": "teal"}"#);

        assert_eq!(
            SettingsStore::open(dir.path()).get(),
            settings(|s| s.accent = "teal".into())
        );
    }

    #[test]
    fn values_in_a_file_are_sanitized_on_open() {
        let dir = temp_config_dir();
        write_settings(dir.path(), r#"{"accent": "nope"}"#);

        assert_eq!(SettingsStore::open(dir.path()).get().accent, SYSTEM_ACCENT);
    }

    #[tokio::test]
    async fn unknown_keys_load_without_harm_and_are_dropped_on_the_next_save() {
        let dir = temp_config_dir();
        write_settings(
            dir.path(),
            r#"{"fontUi": "Helvetica", "accent": "teal", "tracker": {"idleMinutes": 9}}"#,
        );
        let store = SettingsStore::open(dir.path());

        let loaded = store.get();
        store.update(loaded.clone()).await.unwrap();

        assert_eq!(loaded, settings(|s| s.accent = "teal".into()));
        let written = written(dir.path());
        assert!(written.get("fontUi").is_none());
        assert!(written.get("tracker").is_none());
        assert_eq!(written["accent"], "teal");
    }

    #[test]
    fn a_corrupt_file_loads_defaults_and_is_moved_to_a_corrupt_backup() {
        let dir = temp_config_dir();
        write_settings(dir.path(), "{ not json");

        assert_eq!(SettingsStore::open(dir.path()).get(), Settings::default());

        let backups = corrupt_backups(dir.path());
        assert_eq!(backups.len(), 1);
        assert_eq!(
            std::fs::read_to_string(dir.path().join(&backups[0])).unwrap(),
            "{ not json"
        );
        assert_eq!(dir_entries(dir.path()), backups);
    }

    #[test]
    fn without_a_file_the_store_loads_defaults_and_creates_nothing() {
        let dir = temp_config_dir();

        assert_eq!(SettingsStore::open(dir.path()).get(), Settings::default());
        assert!(dir_entries(dir.path()).is_empty());
    }

    #[test]
    fn a_wrongly_typed_accent_or_launch_at_login_falls_back_to_its_default_and_keeps_the_other_fields()
     {
        for (raw, expected) in [
            (
                r#"{"accent": 7, "lightScheme": "light"}"#,
                settings(|s| s.light_scheme = Scheme::Light),
            ),
            (
                r#"{"accent": null, "textSize": "large"}"#,
                settings(|s| s.text_size = TextSize::Large),
            ),
            (
                r#"{"launchAtLogin": "yes", "accent": "teal"}"#,
                settings(|s| s.accent = "teal".into()),
            ),
            (
                r#"{"launchAtLogin": null, "darkScheme": "light"}"#,
                settings(|s| s.dark_scheme = Scheme::Light),
            ),
        ] {
            let dir = temp_config_dir();
            write_settings(dir.path(), raw);

            let loaded = SettingsStore::open(dir.path()).get();

            assert_eq!(loaded, expected, "{raw}");
            assert_eq!(
                std::fs::read_to_string(dir.path().join(FILE_NAME)).unwrap(),
                raw,
                "{raw}"
            );
            assert_eq!(
                dir_entries(dir.path()),
                vec![FILE_NAME.to_string()],
                "{raw}"
            );
        }
    }

    #[tokio::test]
    async fn update_returns_previous_and_sanitized_saved_and_persists_across_reopen() {
        let dir = temp_config_dir();
        let store = SettingsStore::open(dir.path());

        let (previous, saved) = store
            .update(settings(|s| {
                s.dark_scheme = Scheme::Light;
                s.accent = "nope".into();
            }))
            .await
            .unwrap();

        assert_eq!(previous, Settings::default());
        assert_eq!(saved, settings(|s| s.dark_scheme = Scheme::Light));
        assert_eq!(store.get(), saved);
        assert_eq!(SettingsStore::open(dir.path()).get(), saved);
    }

    #[tokio::test]
    async fn second_update_sees_the_first_as_previous() {
        let dir = temp_config_dir();
        let store = SettingsStore::open(dir.path());
        let (_, first) = store
            .update(settings(|s| s.accent = "blue".into()))
            .await
            .unwrap();

        let (previous, _) = store.update(Settings::default()).await.unwrap();

        assert_eq!(previous, first);
    }
}
