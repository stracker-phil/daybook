//! Where a day's note is: Obsidian's daily-notes folder and name format
//! (`<vault>/.obsidian/daily-notes.json`, moment.js tokens), else the fallback from Settings.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{LazyLock, Mutex};
use std::time::SystemTime;

use chrono::{Datelike, NaiveDate};
use regex::{Captures, Regex};
use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use crate::lock::MutexExt;
use crate::store::settings::DailyNote;

const MONTHS: [&str; 12] = [
    "January",
    "February",
    "March",
    "April",
    "May",
    "June",
    "July",
    "August",
    "September",
    "October",
    "November",
    "December",
];
const WEEKDAYS: [&str; 7] = [
    "Sunday",
    "Monday",
    "Tuesday",
    "Wednesday",
    "Thursday",
    "Friday",
    "Saturday",
];
const DEFAULT_FORMAT: &str = "YYYY-MM-DD";

static TOKENS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\[([^\]]*)\]|YYYY|YY|MMMM|MMM|MM|M|Do|DD|D|dddd|ddd|dd|d|GGGG|gggg|WW|ww|W|w|Q")
        .unwrap()
});

fn ordinal(day: u32) -> String {
    let suffix = match (day % 100, day % 10) {
        (11..=13, _) => "th",
        (_, 1) => "st",
        (_, 2) => "nd",
        (_, 3) => "rd",
        _ => "th",
    };
    format!("{day}{suffix}")
}

/// Formats a date with the moment.js tokens Obsidian uses for daily note names.
pub fn format_moment(day: NaiveDate, format: &str) -> String {
    let month = MONTHS[day.month0() as usize];
    let weekday = WEEKDAYS[day.weekday().num_days_from_sunday() as usize];
    let week = day.iso_week();
    TOKENS
        .replace_all(format, |captures: &Captures| {
            if let Some(literal) = captures.get(1) {
                return literal.as_str().to_string();
            }
            match &captures[0] {
                "YYYY" => day.year().to_string(),
                "YY" => format!("{:02}", day.year() % 100),
                "MMMM" => month.to_string(),
                "MMM" => month[..3].to_string(),
                "MM" => format!("{:02}", day.month()),
                "M" => day.month().to_string(),
                "Do" => ordinal(day.day()),
                "DD" => format!("{:02}", day.day()),
                "D" => day.day().to_string(),
                "dddd" => weekday.to_string(),
                "ddd" => weekday[..3].to_string(),
                "dd" => weekday[..2].to_string(),
                "d" => day.weekday().num_days_from_sunday().to_string(),
                "GGGG" | "gggg" => week.year().to_string(),
                "WW" | "ww" => format!("{:02}", week.week()),
                "W" | "w" => week.week().to_string(),
                "Q" => (day.month0() / 3 + 1).to_string(),
                token => token.to_string(),
            }
        })
        .into_owned()
}

/// The daily-notes folder (relative to the vault) and the note name format.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct DailyNoteFormat {
    /// Empty: the vault's root.
    pub folder: String,
    pub format: String,
    /// Read from Obsidian's settings rather than the fallback.
    pub from_obsidian: bool,
}

fn clean_folder(folder: &str) -> String {
    folder.trim().trim_matches('/').to_string()
}

fn obsidian_settings(vault: &Path) -> PathBuf {
    vault.join(".obsidian").join("daily-notes.json")
}

fn load_format(vault: &Path, fallback: &DailyNote) -> DailyNoteFormat {
    let raw = fs::read_to_string(obsidian_settings(vault))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object);
    match raw {
        Some(raw) => DailyNoteFormat {
            folder: raw["folder"].as_str().map(clean_folder).unwrap_or_default(),
            format: raw["format"]
                .as_str()
                .map(str::trim)
                .filter(|format| !format.is_empty())
                .unwrap_or(DEFAULT_FORMAT)
                .to_string(),
            from_obsidian: true,
        },
        None => DailyNoteFormat {
            folder: clean_folder(&fallback.fallback_folder),
            format: fallback.fallback_format.clone(),
            from_obsidian: false,
        },
    }
}

impl DailyNoteFormat {
    fn folder_in(&self, vault: &Path) -> PathBuf {
        if self.folder.is_empty() {
            vault.to_path_buf()
        } else {
            vault.join(&self.folder)
        }
    }

    pub fn note_path(&self, vault: &Path, day: NaiveDate) -> PathBuf {
        self.folder_in(vault)
            .join(format!("{}.md", format_moment(day, &self.format)))
    }

    /// Older years are archived to `<folder>/YYYY/`.
    pub fn archived_path(&self, vault: &Path, day: NaiveDate) -> PathBuf {
        let name = format_moment(day, &self.format);
        let name = name
            .rsplit_once('/')
            .map_or(name.as_str(), |(_, last)| last);
        self.folder_in(vault)
            .join(day.year().to_string())
            .join(format!("{name}.md"))
    }
}

/// What a cached format was read from; any change reads the file again.
#[derive(PartialEq)]
struct Source {
    vault: PathBuf,
    modified: Option<SystemTime>,
    len: Option<u64>,
    fallback_folder: String,
    fallback_format: String,
}

/// The format of the last lookup, kept until Obsidian's file or the fallback changes.
#[derive(Default)]
pub struct FormatCache {
    cached: Mutex<Option<(Source, DailyNoteFormat)>>,
}

impl FormatCache {
    pub fn get(&self, vault: &Path, fallback: &DailyNote) -> DailyNoteFormat {
        let meta = fs::metadata(obsidian_settings(vault)).ok();
        let source = Source {
            vault: vault.to_path_buf(),
            modified: meta.as_ref().and_then(|m| m.modified().ok()),
            len: meta.as_ref().map(|m| m.len()),
            fallback_folder: fallback.fallback_folder.clone(),
            fallback_format: fallback.fallback_format.clone(),
        };
        let mut cached = self.cached.lock_unpoisoned();
        if let Some((known, format)) = cached.as_ref()
            && *known == source
        {
            return format.clone();
        }
        let format = load_format(vault, fallback);
        *cached = Some((source, format.clone()));
        format
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: i32, month: u32, date: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, date).unwrap()
    }

    fn monday() -> NaiveDate {
        day(2026, 1, 5)
    }

    fn obsidian(folder: &str, format: &str) -> DailyNoteFormat {
        DailyNoteFormat {
            folder: folder.into(),
            format: format.into(),
            from_obsidian: true,
        }
    }

    fn write_settings(vault: &Path, json: &str) {
        let dir = vault.join(".obsidian");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("daily-notes.json"), json).unwrap();
    }

    fn fallback(folder: &str, format: &str) -> DailyNote {
        DailyNote {
            fallback_folder: folder.into(),
            fallback_format: format.into(),
            ..DailyNote::default()
        }
    }

    #[test]
    fn the_name_follows_the_obsidian_tokens() {
        let vault = Path::new("/v");
        for (folder, format, expected) in [
            ("1-Journal", "YYYY-MM-DD", "/v/1-Journal/2026-01-05.md"),
            (
                "",
                "YYYY/MM/YYYY-MM-DD dddd",
                "/v/2026/01/2026-01-05 Monday.md",
            ),
            ("", "[Week] WW-GGGG", "/v/Week 02-2026.md"),
            ("", "MMM Do, YY [Q]Q", "/v/Jan 5th, 26 Q1.md"),
        ] {
            assert_eq!(
                obsidian(folder, format).note_path(vault, monday()),
                PathBuf::from(expected),
                "{format}"
            );
        }
    }

    #[test]
    fn older_notes_are_found_in_the_year_folder() {
        assert_eq!(
            obsidian("1-Journal", "YYYY-MM-DD").archived_path(Path::new("/v"), monday()),
            PathBuf::from("/v/1-Journal/2026/2026-01-05.md")
        );
    }

    #[test]
    fn the_archived_path_keeps_only_the_last_segment_of_the_name() {
        assert_eq!(
            obsidian("", "YYYY/MM/YYYY-MM-DD").archived_path(Path::new("/v"), monday()),
            PathBuf::from("/v/2026/2026-01-05.md")
        );
    }

    #[test]
    fn the_remaining_tokens_are_formatted() {
        for (format, expected) in [
            ("MMMM M MM", "January 1 01"),
            ("D DD", "5 05"),
            ("dddd ddd dd d", "Monday Mon Mo 1"),
            ("W WW", "2 02"),
            ("[YYYY] YYYY", "YYYY 2026"),
        ] {
            assert_eq!(format_moment(monday(), format), expected, "{format}");
        }
    }

    #[test]
    fn ordinals_use_the_right_suffix() {
        for (date, expected) in [
            (1, "1st"),
            (2, "2nd"),
            (3, "3rd"),
            (4, "4th"),
            (11, "11th"),
            (12, "12th"),
            (13, "13th"),
            (21, "21st"),
            (22, "22nd"),
        ] {
            assert_eq!(format_moment(day(2026, 3, date), "Do"), expected, "{date}");
        }
    }

    #[test]
    fn the_iso_week_year_differs_from_the_calendar_year_at_a_year_boundary() {
        for (date, expected) in [(day(2027, 1, 1), "2026-53"), (day(2024, 12, 30), "2025-1")] {
            assert_eq!(format_moment(date, "GGGG-W"), expected, "{date}");
        }
    }

    #[test]
    fn the_quarter_follows_the_month() {
        for (month, expected) in [(1, "1"), (3, "1"), (4, "2"), (9, "3"), (12, "4")] {
            assert_eq!(
                format_moment(day(2026, month, 10), "Q"),
                expected,
                "{month}"
            );
        }
    }

    #[test]
    fn obsidian_settings_are_read_and_the_folder_is_cleaned() {
        let vault = tempfile::tempdir().unwrap();
        write_settings(
            vault.path(),
            r#"{"folder": " /Daily/Notes/ ", "format": "DD.MM.YYYY"}"#,
        );
        assert_eq!(
            FormatCache::default().get(vault.path(), &DailyNote::default()),
            obsidian("Daily/Notes", "DD.MM.YYYY")
        );
    }

    #[test]
    fn a_missing_or_blank_format_means_iso_dates() {
        for json in [r#"{"folder": "J"}"#, r#"{"folder": "J", "format": "  "}"#] {
            let vault = tempfile::tempdir().unwrap();
            write_settings(vault.path(), json);
            assert_eq!(
                FormatCache::default().get(vault.path(), &DailyNote::default()),
                obsidian("J", "YYYY-MM-DD"),
                "{json}"
            );
        }
    }

    #[test]
    fn a_missing_folder_means_the_vault_root() {
        let vault = tempfile::tempdir().unwrap();
        write_settings(vault.path(), "{}");
        assert_eq!(
            FormatCache::default().get(vault.path(), &DailyNote::default()),
            obsidian("", "YYYY-MM-DD")
        );
    }

    #[test]
    fn a_missing_file_uses_the_settings_fallback() {
        let vault = tempfile::tempdir().unwrap();
        assert_eq!(
            FormatCache::default().get(vault.path(), &DailyNote::default()),
            DailyNoteFormat {
                folder: "1-Journal".into(),
                format: "YYYY-MM-DD".into(),
                from_obsidian: false,
            }
        );
    }

    #[test]
    fn invalid_json_uses_the_settings_fallback() {
        for json in ["not json", "[1, 2]", ""] {
            let vault = tempfile::tempdir().unwrap();
            write_settings(vault.path(), json);
            assert_eq!(
                FormatCache::default().get(vault.path(), &fallback("/Fall/", "YYYY")),
                DailyNoteFormat {
                    folder: "Fall".into(),
                    format: "YYYY".into(),
                    from_obsidian: false,
                },
                "{json}"
            );
        }
    }

    #[test]
    fn a_changed_obsidian_file_is_noticed() {
        let vault = tempfile::tempdir().unwrap();
        let cache = FormatCache::default();
        write_settings(vault.path(), r#"{"folder": "Old"}"#);
        assert_eq!(cache.get(vault.path(), &DailyNote::default()).folder, "Old");
        write_settings(vault.path(), r#"{"folder": "Much newer"}"#);
        assert_eq!(
            cache.get(vault.path(), &DailyNote::default()).folder,
            "Much newer"
        );
    }

    #[test]
    fn a_file_created_after_the_first_lookup_is_noticed() {
        let vault = tempfile::tempdir().unwrap();
        let cache = FormatCache::default();
        assert!(!cache.get(vault.path(), &DailyNote::default()).from_obsidian);
        write_settings(vault.path(), r#"{"folder": "J"}"#);
        assert_eq!(
            cache.get(vault.path(), &DailyNote::default()),
            obsidian("J", "YYYY-MM-DD")
        );
    }

    #[test]
    fn a_changed_fallback_takes_effect() {
        let vault = tempfile::tempdir().unwrap();
        let cache = FormatCache::default();
        assert_eq!(cache.get(vault.path(), &fallback("A", "YYYY")).folder, "A");
        let changed = cache.get(vault.path(), &fallback("B", "MM"));
        assert_eq!(
            (changed.folder.as_str(), changed.format.as_str()),
            ("B", "MM")
        );
    }

    #[test]
    fn another_vault_is_not_served_from_the_cache() {
        let (first, second) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let cache = FormatCache::default();
        write_settings(first.path(), r#"{"folder": "One"}"#);
        write_settings(second.path(), r#"{"folder": "Two"}"#);
        assert_eq!(cache.get(first.path(), &DailyNote::default()).folder, "One");
        assert_eq!(
            cache.get(second.path(), &DailyNote::default()).folder,
            "Two"
        );
    }
}
