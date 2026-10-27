//! Daybook's whole configuration in `settings.json`. Reading repairs instead of rejecting: a broken
//! part falls back to its default, so a hand-edited file never loses the rest.

mod calls;
mod contexts;
mod defaults;
mod links;
mod music;
mod rail;
mod read;
mod shell;
mod slack;
mod store;
mod timetracker;
mod types;
mod vault;
mod workflows;

use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use ts_rs::TS;

pub use calls::*;
pub use contexts::*;
pub use links::*;
pub use music::*;
pub use rail::*;
pub use shell::*;
pub use slack::*;
pub use store::{FILE_NAME as SETTINGS_FILE, SettingsStore};
pub use timetracker::*;
pub use types::*;
pub use vault::*;
pub use workflows::*;

use read::{field, flag, is_list, items, list, parsed, string};

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    /// The look while macOS is in light mode.
    pub light_scheme: Scheme,
    /// The look while macOS is in dark mode.
    pub dark_scheme: Scheme,
    /// The app accent: `system` (the macOS accent color) or one of `ACCENTS`.
    pub accent: String,
    pub text_size: TextSize,
    pub launch_at_login: bool,
    pub vault: Vault,
    pub daily_note: DailyNote,
    pub status_messages: StatusMessages,
    pub slack: Slack,
    pub end_of_day: EndOfDay,
    pub focus: Focus,
    pub status_rules: StatusRules,
    pub timeline: Timeline,
    pub meetings: Meetings,
    pub joining: Joining,
    pub projects: Vec<Project>,
    pub contexts: Vec<Context>,
    pub links: Vec<QuickLink>,
    /// What the Music tool offers, shared by all contexts.
    pub music: Vec<MusicEntry>,
    pub search: Search,
    pub journal: Vec<JournalGroup>,
    pub routines: Vec<Routine>,
    /// In priority order; every routine and journal group has exactly one.
    pub reminders: Vec<ReminderRule>,
    pub rail: Rail,
    pub tools: Vec<ToolPlacement>,
    pub hotkeys: Hotkeys,
    pub api: Api,
    pub jira: Jira,
    pub timetracker: TimeTrackerSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            light_scheme: Scheme::Default,
            dark_scheme: Scheme::Dark,
            accent: SYSTEM_ACCENT.into(),
            text_size: TextSize::Medium,
            launch_at_login: false,
            vault: Vault::default(),
            daily_note: DailyNote::default(),
            status_messages: StatusMessages::default(),
            slack: Slack::default(),
            end_of_day: EndOfDay::default(),
            focus: Focus::default(),
            status_rules: StatusRules::default(),
            timeline: Timeline::default(),
            meetings: Meetings::default(),
            joining: Joining::default(),
            projects: defaults::projects(),
            contexts: defaults::contexts(),
            links: vec![],
            music: vec![],
            search: Search::default(),
            journal: defaults::journal(),
            routines: defaults::routines(),
            reminders: defaults::reminders(),
            rail: Rail::default(),
            tools: default_tools(),
            hotkeys: Hotkeys::default(),
            api: Api::default(),
            jira: Jira::default(),
            timetracker: TimeTrackerSettings::default(),
        }
    }
}

impl Settings {
    /// The settings in any JSON value, each part repaired or replaced by its default.
    pub fn from_json(raw: &Value) -> Self {
        let defaults = Self::default();
        let source = raw.as_object();
        let get = |key| field(source, key);
        let all = |key| is_list(get(key)).then(|| items(get(key)));
        let journal = list(get("journal"), JournalGroup::read).unwrap_or_else(defaults::journal);
        let routines = all("routines").map_or_else(defaults::routines, Routine::read_all);
        let music = all("music").map(MusicEntry::read_all).unwrap_or_default();
        let contexts = all("contexts").map_or_else(defaults::contexts, Context::read_all);
        let reminders =
            list(get("reminders"), ReminderRule::read).unwrap_or_else(defaults::reminders);
        Self {
            light_scheme: parsed(get("lightScheme")).unwrap_or(defaults.light_scheme),
            dark_scheme: parsed(get("darkScheme")).unwrap_or(defaults.dark_scheme),
            accent: string(get("accent"))
                .filter(|accent| is_app_accent(accent))
                .unwrap_or_else(|| SYSTEM_ACCENT.into()),
            text_size: parsed(get("textSize")).unwrap_or(defaults.text_size),
            launch_at_login: flag(get("launchAtLogin")).unwrap_or(defaults.launch_at_login),
            vault: Vault::read(get("vault")),
            daily_note: DailyNote::read(get("dailyNote")),
            status_messages: StatusMessages::read(get("statusMessages")),
            slack: Slack::read(get("slack")),
            end_of_day: EndOfDay::read(get("endOfDay")),
            focus: Focus::read(get("focus")),
            status_rules: StatusRules::read(get("statusRules")),
            timeline: Timeline::read(get("timeline")),
            meetings: Meetings::read(get("meetings")),
            joining: Joining::read(get("joining")),
            projects: list(get("projects"), Project::read).unwrap_or_else(defaults::projects),
            contexts: with_known_start(contexts, &music),
            links: list(get("links"), QuickLink::read).unwrap_or_default(),
            music,
            search: Search::read(get("search")),
            reminders: with_item_reminders(reminders, &routines, &journal),
            journal,
            routines,
            rail: Rail::read(get("rail")),
            tools: read_tools(get("tools")),
            hotkeys: Hotkeys::read(get("hotkeys")),
            api: Api::read(get("api")),
            jira: Jira::read(get("jira")),
            timetracker: TimeTrackerSettings::read(get("timetracker")),
        }
    }

    /// Repairs values set in Rust the same way as a loaded file.
    pub fn sanitized(self) -> Self {
        match serde_json::to_value(&self) {
            Ok(value) => Self::from_json(&value),
            Err(_) => self,
        }
    }
}

impl<'de> Deserialize<'de> for Settings {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::from_json(&Value::deserialize(deserializer)?))
    }
}

/// A context can only start the day with a music entry that exists.
fn with_known_start(mut contexts: Vec<Context>, music: &[MusicEntry]) -> Vec<Context> {
    for context in &mut contexts {
        let start = &context.music.start_with;
        if !start.is_empty() && !music.iter().any(|entry| &entry.id == start) {
            context.music.start_with.clear();
        }
    }
    contexts
}

#[cfg(test)]
mod rules;

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
        let value = serde_json::to_value(Settings::default()).unwrap();

        assert_eq!(value["lightScheme"], json!("default"));
        assert_eq!(value["darkScheme"], json!("dark"));
        assert_eq!(value["accent"], json!("system"));
        assert_eq!(value["textSize"], json!("medium"));
        assert_eq!(value["launchAtLogin"], json!(false));
        let keys: Vec<&str> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(
            keys,
            [
                "lightScheme",
                "darkScheme",
                "accent",
                "textSize",
                "launchAtLogin",
                "vault",
                "dailyNote",
                "statusMessages",
                "slack",
                "endOfDay",
                "focus",
                "statusRules",
                "timeline",
                "meetings",
                "joining",
                "projects",
                "contexts",
                "links",
                "music",
                "search",
                "journal",
                "routines",
                "reminders",
                "rail",
                "tools",
                "hotkeys",
                "api",
                "jira",
                "timetracker"
            ]
        );
    }

    #[test]
    fn the_frontend_default_settings_fixture_matches_the_defaults() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../src/test/default-settings.json");
        let expected = serde_json::to_value(Settings::default()).unwrap();

        if std::env::var("UPDATE_FIXTURES").is_ok_and(|flag| flag == "1") {
            let mut text = serde_json::to_string_pretty(&expected).unwrap();
            text.push('\n');
            std::fs::write(&path, text).unwrap();
            return;
        }

        let stored: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(
            stored, expected,
            "src/test/default-settings.json is stale; run the test with UPDATE_FIXTURES=1"
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
