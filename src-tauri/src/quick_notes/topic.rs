//! A quick-notes topic as the bus and the pages name it, and the picker's rows. Pure.

use serde::Serialize;
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::vault::ops::notes::NoteSectionRef;

/// The level a new topic gets: `###`.
const NEW_LEVEL: u8 = 3;

const MAX_TITLE: usize = 200;

/// A topic the editor may write: a title and a heading level from `##` to `######`.
pub fn checked(topic: NoteSectionRef) -> AppResult<NoteSectionRef> {
    let title = topic.title.trim();
    if title.is_empty() {
        return Err(AppError::InvalidInput("The topic needs a title.".into()));
    }
    if title.chars().count() > MAX_TITLE || title.contains('\n') {
        return Err(AppError::InvalidInput(format!(
            "A topic title is one line of at most {MAX_TITLE} characters."
        )));
    }
    if !(2..=6).contains(&topic.level) {
        return Err(AppError::InvalidInput(format!(
            "A topic heading has 2 to 6 #, not {}.",
            topic.level
        )));
    }
    Ok(NoteSectionRef {
        title: title.to_string(),
        level: topic.level,
    })
}

/// `notes:topic:<level>:<title>`'s argument.
pub fn from_argument(argument: &str) -> AppResult<NoteSectionRef> {
    let invalid = || AppError::InvalidInput(format!("“{argument}” isn't <level>:<title>."));
    let (level, title) = argument.split_once(':').ok_or_else(invalid)?;
    let level = level.parse().map_err(|_| invalid())?;
    checked(NoteSectionRef {
        title: title.to_string(),
        level,
    })
}

pub fn to_argument(topic: &NoteSectionRef) -> String {
    format!("{}:{}", topic.level, topic.title)
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct TopicRow {
    pub topic: NoteSectionRef,
    /// Not in the note yet; opening it adds it.
    pub create: bool,
}

/// The topics whose title contains `query` (ignoring case), then a new topic named `query` unless one
/// has that title.
pub fn rows_for(topics: &[NoteSectionRef], query: &str) -> Vec<TopicRow> {
    let query = query.trim();
    let lower = query.to_lowercase();
    let mut rows: Vec<TopicRow> = topics
        .iter()
        .filter(|topic| topic.title.to_lowercase().contains(&lower))
        .map(|topic| TopicRow {
            topic: topic.clone(),
            create: false,
        })
        .collect();
    let named = rows
        .iter()
        .any(|row| row.topic.title.to_lowercase() == lower);
    if !query.is_empty() && !named {
        rows.push(TopicRow {
            topic: NoteSectionRef {
                title: query.to_string(),
                level: NEW_LEVEL,
            },
            create: true,
        });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    fn topic(title: &str, level: u8) -> NoteSectionRef {
        NoteSectionRef {
            title: title.into(),
            level,
        }
    }

    fn row(title: &str, level: u8, create: bool) -> TopicRow {
        TopicRow {
            topic: topic(title, level),
            create,
        }
    }

    fn today() -> Vec<NoteSectionRef> {
        vec![
            topic("Standup", 3),
            topic("Call with Anna", 3),
            topic("Details", 4),
        ]
    }

    #[test]
    fn an_empty_query_lists_every_topic_in_note_order_without_a_new_one() {
        for query in ["", "   "] {
            assert_eq!(
                rows_for(&today(), query),
                [
                    row("Standup", 3, false),
                    row("Call with Anna", 3, false),
                    row("Details", 4, false),
                ],
                "{query:?}"
            );
        }
        assert_eq!(rows_for(&[], ""), []);
    }

    #[test]
    fn a_query_keeps_the_topics_containing_it_ignoring_case_and_offers_a_new_one() {
        assert_eq!(
            rows_for(&today(), "  AN "),
            [
                row("Standup", 3, false),
                row("Call with Anna", 3, false),
                row("AN", 3, true),
            ]
        );
    }

    #[test]
    fn a_query_naming_a_topic_exactly_offers_no_new_one() {
        for query in ["Standup", "standup", "  STANDUP  "] {
            assert_eq!(
                rows_for(&today(), query),
                [row("Standup", 3, false)],
                "{query:?}"
            );
        }
        assert_eq!(rows_for(&today(), "details"), [row("Details", 4, false)]);
    }

    #[test]
    fn a_query_matching_nothing_offers_only_a_new_level_three_topic() {
        assert_eq!(rows_for(&today(), "  Retro "), [row("Retro", 3, true)]);
        assert_eq!(rows_for(&[], "Retro"), [row("Retro", 3, true)]);
    }

    #[test]
    fn a_topic_row_serializes_with_its_topic_and_create_flag() {
        assert_eq!(
            serde_json::to_value(row("Retro", 3, true)).unwrap(),
            serde_json::json!({"topic": {"title": "Retro", "level": 3}, "create": true})
        );
    }

    #[test]
    fn a_topic_title_is_trimmed() {
        assert_eq!(
            checked(topic("  Standup \t", 3)).unwrap(),
            topic("Standup", 3)
        );
    }

    #[test]
    fn invalid_topics_are_rejected_as_invalid_input() {
        let too_long = "x".repeat(MAX_TITLE + 1);
        for (title, level) in [
            ("", 3),
            ("  \t", 3),
            (too_long.as_str(), 3),
            ("two\nlines", 3),
            ("Fine", 1),
            ("Fine", 0),
            ("Fine", 7),
        ] {
            let err = checked(topic(title, level)).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{title:?} {level}");
        }
    }

    #[test]
    fn titles_of_the_longest_length_and_levels_two_to_six_are_accepted() {
        let longest = "é".repeat(MAX_TITLE);
        assert_eq!(checked(topic(&longest, 2)).unwrap(), topic(&longest, 2));
        for level in 2..=6 {
            assert!(checked(topic("Fine", level)).is_ok(), "{level}");
        }
    }

    #[test]
    fn an_argument_is_a_level_and_a_title() {
        assert_eq!(from_argument("3:Standup").unwrap(), topic("Standup", 3));
        assert_eq!(
            from_argument("4:Call: Anna").unwrap(),
            topic("Call: Anna", 4)
        );
        assert_eq!(from_argument("2: Padded ").unwrap(), topic("Padded", 2));
    }

    #[test]
    fn malformed_arguments_are_rejected_as_invalid_input() {
        for argument in [
            "",
            "Standup",
            "x:Standup",
            ":Standup",
            "3:",
            "3:  ",
            "1:Standup",
            "7:A",
            "-3:A",
            "300:A",
        ] {
            let err = from_argument(argument).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidInput, "{argument:?}");
        }
    }

    #[test]
    fn an_argument_round_trips_through_its_topic() {
        for original in [
            topic("Standup", 3),
            topic("Call: Anna", 4),
            topic("Ünï 日本", 6),
        ] {
            assert_eq!(
                from_argument(&to_argument(&original)).unwrap(),
                original,
                "{original:?}"
            );
        }
        assert_eq!(to_argument(&topic("Call: Anna", 4)), "4:Call: Anna");
    }
}
