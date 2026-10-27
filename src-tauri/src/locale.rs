//! Dates as the Daybook window shows them, in the Mac's locale (`AppleLocale`, read once): the names come from
//! chrono's locales, the order and punctuation from the language. Pure apart from `system`, `system_async`
//! and `apple_locale`.

use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use chrono::{Datelike, Locale, NaiveDate};

use crate::process;

const READ_TIMEOUT: Duration = Duration::from_secs(3);

/// Languages that write "27. Oktober 2026".
const DAY_DOT_LANGUAGES: &[&str] = &[
    "de", "da", "nb", "nn", "no", "fi", "cs", "sk", "sl", "hr", "is",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Order {
    /// "27. Oktober 2026", "27. Okt."
    DayDot,
    /// "27 October 2026", "27 Oct"
    DayFirst,
    /// "October 27, 2026", "Oct 27"
    MonthFirst,
}

impl Order {
    fn long(self) -> &'static str {
        match self {
            Order::DayDot => "%-d. %B %Y",
            Order::DayFirst => "%-d %B %Y",
            Order::MonthFirst => "%B %-d, %Y",
        }
    }

    fn day_title(self) -> &'static str {
        match self {
            Order::DayDot => "%A, %-d. %B",
            Order::DayFirst => "%A, %-d %B",
            Order::MonthFirst => "%A, %B %-d",
        }
    }

    /// `year` only when it isn't this year.
    fn short(self, day: u32, month: &str, year: Option<i32>) -> String {
        match (self, year) {
            (Order::DayDot, None) => format!("{day}. {month}"),
            (Order::DayDot, Some(year)) => format!("{day}. {month} {year}"),
            (Order::DayFirst, None) => format!("{day} {month}"),
            (Order::DayFirst, Some(year)) => format!("{day} {month} {year}"),
            (Order::MonthFirst, None) => format!("{month} {day}"),
            (Order::MonthFirst, Some(year)) => format!("{month} {day}, {year}"),
        }
    }

    /// English names stand in for an unknown language, so it takes the US order.
    fn for_language(language: &str, region: Option<&str>, has_names: bool) -> Self {
        if DAY_DOT_LANGUAGES.contains(&language) {
            Order::DayDot
        } else if !has_names || (language == "en" && region.is_none_or(|region| region == "US")) {
            Order::MonthFirst
        } else {
            Order::DayFirst
        }
    }
}

/// The region's own names, else the language's.
fn chrono_locale(language: &str, region: Option<&str>) -> Option<Locale> {
    region
        .iter()
        .map(|region| format!("{language}_{region}"))
        .chain([format!("{language}_{}", language.to_uppercase())])
        .find_map(|name| Locale::try_from(name.as_str()).ok())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DateLocale {
    names: Locale,
    order: Order,
}

impl Default for DateLocale {
    fn default() -> Self {
        Self {
            names: Locale::en_US,
            order: Order::MonthFirst,
        }
    }
}

impl DateLocale {
    /// `de_AT`, `en_GB`, `en-US`, `de`, `en_US@rg=atzzzz`; an unknown language gets English names.
    pub fn parse(tag: &str) -> Self {
        let base = tag
            .trim()
            .split('@')
            .next()
            .unwrap_or_default()
            .replace('-', "_");
        let mut parts = base.split('_');
        let language = parts.next().unwrap_or_default().to_lowercase();
        let region = parts.next_back().map(str::to_uppercase);
        let names = chrono_locale(&language, region.as_deref());
        Self {
            order: Order::for_language(&language, region.as_deref(), names.is_some()),
            names: names.unwrap_or(Locale::en_US),
        }
    }

    fn format(&self, day: NaiveDate, pattern: &str) -> String {
        day.format_localized(pattern, self.names).to_string()
    }

    /// "Dienstag", "Tuesday".
    pub fn weekday(&self, day: NaiveDate) -> String {
        self.format(day, "%A")
    }

    /// "Oktober", "October".
    pub fn month(&self, day: NaiveDate) -> String {
        self.format(day, "%B")
    }

    /// "27. Oktober 2026", "27 October 2026", "October 27, 2026".
    pub fn long_date(&self, day: NaiveDate) -> String {
        self.format(day, self.order.long())
    }

    /// Another day's title: "Dienstag, 27. Oktober", "Tuesday, 27 October", "Tuesday, October 27".
    pub fn day_title(&self, day: NaiveDate) -> String {
        self.format(day, self.order.day_title())
    }

    /// "27. Okt.", "27 Oct", "Oct 27"; with the year when it isn't `today`'s.
    pub fn short_date(&self, date: NaiveDate, today: NaiveDate) -> String {
        let mut month = self.format(date, "%b");
        if self.order == Order::DayDot && month != self.month(date) && !month.ends_with('.') {
            month.push('.');
        }
        let year = date.year();
        let other_year = (year != today.year()).then_some(year);
        self.order.short(date.day(), &month, other_year)
    }

    /// `short_date` after the short weekday: "Di 27. Okt.", "Tue 27 Oct", "Tue Oct 27".
    pub fn short_day(&self, date: NaiveDate, today: NaiveDate) -> String {
        format!(
            "{} {}",
            self.format(date, "%a"),
            self.short_date(date, today)
        )
    }
}

/// The Mac's locale; US English when it can't be read. Blocking the first time.
pub fn system() -> DateLocale {
    static SYSTEM: OnceLock<DateLocale> = OnceLock::new();
    *SYSTEM.get_or_init(|| apple_locale().map_or_else(DateLocale::default, DateLocale::parse))
}

/// `system` off the async workers, for commands that may be the first to ask.
pub async fn system_async() -> DateLocale {
    tauri::async_runtime::spawn_blocking(system)
        .await
        .unwrap_or_default()
}

/// `AppleLocale` as macOS stores it (`de_AT`, `en_US@rg=atzzzz`), read once. Blocking the first time.
pub fn apple_locale() -> Option<&'static str> {
    static TAG: OnceLock<Option<String>> = OnceLock::new();
    TAG.get_or_init(read_apple_locale).as_deref()
}

fn read_apple_locale() -> Option<String> {
    let args = ["read", "-g", "AppleLocale"].map(String::from);
    let output = process::run(Path::new("/usr/bin/defaults"), &args, READ_TIMEOUT).ok()?;
    let tag = output.stdout.trim();
    (output.success && !tag.is_empty()).then(|| tag.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn today() -> NaiveDate {
        day(2026, 10, 27)
    }

    #[test]
    fn weekdays_are_named_in_the_language() {
        for (tag, expected) in [("de_AT", "Dienstag"), ("en_US", "Tuesday")] {
            assert_eq!(DateLocale::parse(tag).weekday(today()), expected, "{tag}");
        }
    }

    #[test]
    fn months_are_named_in_the_language_and_region() {
        for (tag, date, expected) in [
            ("de_AT", day(2026, 1, 15), "Jänner"),
            ("de_DE", day(2026, 10, 27), "Oktober"),
            ("en_US", day(2026, 10, 27), "October"),
        ] {
            assert_eq!(DateLocale::parse(tag).month(date), expected, "{tag}");
        }
    }

    #[test]
    fn long_dates_follow_the_order_of_the_language() {
        for (tag, expected) in [
            ("de_AT", "27. Oktober 2026"),
            ("en_US", "October 27, 2026"),
            ("en_GB", "27 October 2026"),
        ] {
            assert_eq!(DateLocale::parse(tag).long_date(today()), expected, "{tag}");
        }
    }

    #[test]
    fn another_days_title_names_the_weekday_first() {
        for (tag, expected) in [
            ("de_AT", "Dienstag, 27. Oktober"),
            ("en_US", "Tuesday, October 27"),
            ("en_GB", "Tuesday, 27 October"),
        ] {
            assert_eq!(DateLocale::parse(tag).day_title(today()), expected, "{tag}");
        }
    }

    #[test]
    fn short_dates_leave_out_the_year_of_today() {
        for (tag, expected) in [
            ("de_AT", "27. Okt."),
            ("en_US", "Oct 27"),
            ("en_GB", "27 Oct"),
        ] {
            assert_eq!(
                DateLocale::parse(tag).short_date(today(), today()),
                expected,
                "{tag}"
            );
        }
    }

    #[test]
    fn short_dates_add_the_year_when_it_is_not_this_year() {
        for (tag, expected) in [
            ("de_AT", "31. Dez. 2025"),
            ("en_US", "Dec 31, 2025"),
            ("en_GB", "31 Dec 2025"),
        ] {
            assert_eq!(
                DateLocale::parse(tag).short_date(day(2025, 12, 31), today()),
                expected,
                "{tag}"
            );
        }
    }

    #[test]
    fn short_days_name_the_weekday_before_the_short_date() {
        for (tag, expected) in [
            ("de_AT", "Di 27. Okt."),
            ("en_US", "Tue Oct 27"),
            ("en_GB", "Tue 27 Oct"),
        ] {
            assert_eq!(
                DateLocale::parse(tag).short_day(today(), today()),
                expected,
                "{tag}"
            );
        }
    }

    #[test]
    fn short_days_add_the_year_when_it_is_not_this_year() {
        for (tag, expected) in [
            ("de_AT", "Mi 31. Dez. 2025"),
            ("en_US", "Wed Dec 31, 2025"),
            ("en_GB", "Wed 31 Dec 2025"),
        ] {
            assert_eq!(
                DateLocale::parse(tag).short_day(day(2025, 12, 31), today()),
                expected,
                "{tag}"
            );
        }
    }

    #[test]
    fn a_month_name_that_is_already_short_gets_no_dot() {
        assert_eq!(
            DateLocale::parse("de_AT").short_date(day(2026, 5, 3), today()),
            "3. Mai"
        );
    }

    #[test]
    fn tags_as_macos_writes_them_resolve_to_their_language_and_region() {
        for (tag, same_as) in [
            ("en_US@rg=atzzzz", "en_US"),
            ("de", "de_DE"),
            ("en-GB", "en_GB"),
            (" de_AT ", "de_AT"),
            ("DE_at", "de_AT"),
            ("en", "en_US"),
        ] {
            assert_eq!(DateLocale::parse(tag), DateLocale::parse(same_as), "{tag}");
        }
        assert_eq!(DateLocale::parse("en_US"), DateLocale::default());
    }

    #[test]
    fn an_unknown_language_gets_english_names() {
        let locale = DateLocale::parse("xx_YY");
        assert_eq!(locale.weekday(today()), "Tuesday");
        assert_eq!(locale.month(today()), "October");
    }

    #[test]
    fn a_known_language_in_an_unlisted_region_falls_back_to_the_languages_own_names() {
        let locale = DateLocale::parse("de_ZZ");
        assert_eq!(locale.weekday(today()), "Dienstag");
        assert_eq!(locale.long_date(today()), "27. Oktober 2026");
    }
}
