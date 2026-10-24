mod serde_helpers;
mod store;
mod types;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use serde_helpers::lenient;
pub use store::SettingsStore;
pub use types::*;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// The look while macOS is in light mode.
    #[serde(deserialize_with = "lenient")]
    pub light_scheme: Scheme,
    /// The look while macOS is in dark mode.
    #[serde(deserialize_with = "lenient_dark_scheme")]
    pub dark_scheme: Scheme,
    /// The app accent: `system` (the macOS accent color) or one of `ACCENTS`.
    #[serde(deserialize_with = "lenient")]
    pub accent: String,
    #[serde(deserialize_with = "lenient")]
    pub text_size: TextSize,
    #[serde(deserialize_with = "lenient")]
    pub launch_at_login: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            light_scheme: Scheme::Default,
            dark_scheme: Scheme::Dark,
            accent: SYSTEM_ACCENT.into(),
            text_size: TextSize::Medium,
            launch_at_login: false,
        }
    }
}

impl Settings {
    /// Resets values the app doesn't know, so a hand-edited file can't break the app.
    pub fn sanitized(mut self) -> Self {
        if !is_app_accent(&self.accent) {
            self.accent = SYSTEM_ACCENT.into();
        }
        self
    }
}

/// `lenient`, but an unknown value falls back to the dark-mode default, `Dark`.
fn lenient_dark_scheme<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Scheme, D::Error> {
    lenient::<D, Option<Scheme>>(deserializer).map(|scheme| scheme.unwrap_or(Scheme::Dark))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    pub(super) fn settings(edit: impl FnOnce(&mut Settings)) -> Settings {
        let mut settings = Settings::default();
        edit(&mut settings);
        settings
    }

    fn quoted_strings(text: &str) -> Vec<String> {
        text.split('"')
            .skip(1)
            .step_by(2)
            .map(String::from)
            .collect()
    }

    #[test]
    fn defaults_are_already_sanitized() {
        assert_eq!(Settings::default().sanitized(), Settings::default());
    }

    #[test]
    fn an_unknown_accent_resets_to_the_default() {
        let sanitized = settings(|s| s.accent = "chartreuse".into()).sanitized();
        assert_eq!(sanitized.accent, SYSTEM_ACCENT);
    }

    #[test]
    fn the_default_accent_follows_the_system() {
        assert_eq!(Settings::default().accent, SYSTEM_ACCENT);
    }

    #[test]
    fn accents_survive_sanitizing() {
        for accent in [SYSTEM_ACCENT, "orange", "teal"] {
            let sanitized = settings(|s| s.accent = accent.into()).sanitized();
            assert_eq!(sanitized.accent, accent, "{accent}");
        }
    }

    #[test]
    fn known_accent_and_appearance_are_kept() {
        let kept = settings(|s| {
            s.accent = "teal".into();
            s.light_scheme = Scheme::Light;
            s.dark_scheme = Scheme::Default;
        });
        assert_eq!(kept.clone().sanitized(), kept);
    }

    #[test]
    fn schemes_round_trip_as_lowercase_camel_case_json() {
        for (light, dark, light_json, dark_json) in [
            (Scheme::Default, Scheme::Light, "default", "light"),
            (Scheme::Light, Scheme::Dark, "light", "dark"),
            (Scheme::Dark, Scheme::Default, "dark", "default"),
        ] {
            let original = settings(|s| {
                s.light_scheme = light;
                s.dark_scheme = dark;
            });

            let value = serde_json::to_value(&original).unwrap();

            assert_eq!(value["lightScheme"], json!(light_json));
            assert_eq!(value["darkScheme"], json!(dark_json));
            let back: Settings = serde_json::from_value(value).unwrap();
            assert_eq!(back, original, "{light_json}/{dark_json}");
        }
    }

    #[test]
    fn text_size_defaults_to_medium_and_round_trips_as_lowercase_json() {
        assert_eq!(Settings::default().text_size, TextSize::Medium);
        for (size, json_value) in [
            (TextSize::Small, "small"),
            (TextSize::Medium, "medium"),
            (TextSize::Large, "large"),
        ] {
            let original = settings(|s| s.text_size = size);

            let value = serde_json::to_value(&original).unwrap();

            assert_eq!(value["textSize"], json!(json_value));
            let back: Settings = serde_json::from_value(value).unwrap();
            assert_eq!(back, original, "{json_value}");
        }
    }

    #[test]
    fn default_settings_serialize_with_camel_case_keys() {
        assert_eq!(
            serde_json::to_value(Settings::default()).unwrap(),
            json!({
                "lightScheme": "default",
                "darkScheme": "dark",
                "accent": "system",
                "textSize": "medium",
                "launchAtLogin": false
            })
        );
    }

    #[test]
    fn accents_match_the_frontend_list() {
        let types_ts = include_str!("../../../../src/api/types.ts");

        let accents_line = types_ts
            .lines()
            .find(|line| line.starts_with("export const ACCENTS = ["))
            .expect("src/api/types.ts should declare `export const ACCENTS = [...]` on one line");
        assert_eq!(quoted_strings(accents_line), ACCENTS, "ACCENTS in types.ts");
    }
}
