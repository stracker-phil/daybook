//! Open todos grouped by how long ago their note is: Today, This Week, Last Week, then each month of this
//! year and each earlier year. Pure.

use chrono::{Datelike, Days, NaiveDate};
use serde::Serialize;
use ts_rs::TS;

use crate::locale::DateLocale;

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct OpenTodo {
    pub text: String,
    /// "12. Okt. · Notes › Meeting Two": the note's day in the Mac's locale, then the headings it sits under.
    pub detail: String,
    /// The note's day, `YYYY-MM-DD`.
    pub date: String,
    /// The note, relative to the vault.
    pub file: String,
    /// The todo's line in its note, 0-based: ticking it checks that this line still holds it.
    pub line: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct TodoGroup {
    pub label: String,
    pub todos: Vec<OpenTodo>,
}

fn group_label(date: NaiveDate, today: NaiveDate, locale: DateLocale) -> String {
    let this_week = today - Days::new(u64::from(today.weekday().num_days_from_monday()));
    let last_week = this_week - Days::new(7);
    if date >= today {
        "Today".into()
    } else if date >= this_week {
        "This Week".into()
    } else if date >= last_week {
        "Last Week".into()
    } else if date.year() == today.year() {
        locale.month(date)
    } else {
        date.year().to_string()
    }
}

/// Keeps the todos' order (newest first); a group follows the one before only when its label differs. Month
/// names follow `locale`.
pub fn group_by_date(
    todos: Vec<(NaiveDate, OpenTodo)>,
    today: NaiveDate,
    locale: DateLocale,
) -> Vec<TodoGroup> {
    let mut groups: Vec<TodoGroup> = Vec::new();
    for (date, todo) in todos {
        let label = group_label(date, today, locale);
        match groups.last_mut() {
            Some(group) if group.label == label => group.todos.push(todo),
            _ => groups.push(TodoGroup {
                label,
                todos: vec![todo],
            }),
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }

    fn today() -> NaiveDate {
        day(2026, 9, 29)
    }

    fn open_todo(text: &str) -> OpenTodo {
        OpenTodo {
            text: text.into(),
            detail: "12 Oct · Notes".into(),
            date: "2026-10-12".into(),
            file: "1-Journal/2026-10-12.md".into(),
            line: 4,
        }
    }

    fn english() -> DateLocale {
        DateLocale::parse("en_GB")
    }

    fn german() -> DateLocale {
        DateLocale::parse("de_DE")
    }

    #[test]
    fn labels_follow_how_long_ago_the_note_is() {
        for (date, expected) in [
            (day(2026, 9, 29), "Today"),
            (day(2026, 9, 30), "Today"),
            (day(2026, 9, 28), "This Week"),
            (day(2026, 9, 27), "Last Week"),
            (day(2026, 9, 21), "Last Week"),
            (day(2026, 9, 20), "September"),
            (day(2026, 3, 1), "March"),
            (day(2025, 12, 31), "2025"),
        ] {
            assert_eq!(group_label(date, today(), english()), expected, "{date}");
        }
    }

    #[test]
    fn month_labels_follow_the_locale() {
        for (date, expected) in [
            (day(2026, 9, 20), "September"),
            (day(2026, 3, 1), "März"),
            (day(2026, 5, 1), "Mai"),
            (day(2025, 12, 31), "2025"),
        ] {
            assert_eq!(group_label(date, today(), german()), expected, "{date}");
        }
        assert_eq!(
            group_label(day(2026, 3, 1), today(), DateLocale::parse("de_AT")),
            "März"
        );
        assert_eq!(group_label(day(2026, 9, 29), today(), german()), "Today");
    }

    #[test]
    fn grouping_keeps_the_order_and_leaves_out_empty_groups() {
        let groups = group_by_date(
            vec![
                (day(2026, 9, 29), open_todo("a")),
                (day(2026, 9, 28), open_todo("b")),
                (day(2025, 12, 31), open_todo("c")),
            ],
            today(),
            english(),
        );

        assert_eq!(
            groups,
            vec![
                TodoGroup {
                    label: "Today".into(),
                    todos: vec![open_todo("a")],
                },
                TodoGroup {
                    label: "This Week".into(),
                    todos: vec![open_todo("b")],
                },
                TodoGroup {
                    label: "2025".into(),
                    todos: vec![open_todo("c")],
                },
            ]
        );
    }

    #[test]
    fn todos_of_the_same_period_share_a_group_in_their_order() {
        let groups = group_by_date(
            vec![
                (day(2026, 9, 27), open_todo("a")),
                (day(2026, 9, 21), open_todo("b")),
                (day(2026, 9, 20), open_todo("c")),
            ],
            today(),
            english(),
        );

        let shape: Vec<(&str, usize)> = groups
            .iter()
            .map(|group| (group.label.as_str(), group.todos.len()))
            .collect();
        assert_eq!(shape, [("Last Week", 2), ("September", 1)]);
        assert!(group_by_date(Vec::new(), today(), english()).is_empty());
    }

    #[test]
    fn todos_and_groups_serialize_in_camel_case() {
        let group = TodoGroup {
            label: "Today".into(),
            todos: vec![open_todo("a")],
        };

        assert_eq!(
            serde_json::to_value(&group).unwrap(),
            serde_json::json!({
                "label": "Today",
                "todos": [{
                    "text": "a",
                    "detail": "12 Oct · Notes",
                    "date": "2026-10-12",
                    "file": "1-Journal/2026-10-12.md",
                    "line": 4
                }]
            })
        );
    }
}
