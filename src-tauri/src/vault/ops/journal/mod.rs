//! The journal lives in callouts inside the journal section of today's note:
//!
//! ```text
//! ## Journal
//!
//! > [!NOTE]+ Dawn
//! > **What am I grateful for?** - _Small things count too_
//! > 1. Sunshine
//! >
//! > **The "One Thing"** - _What would I do today even without pay?_
//! > - [ ] Finish the draft
//!
//! > [!HEART]- Health
//! > Slept well.
//! ```
//!
//! The questions come from Settings. Only answered questions are written, with as many answers as
//! given; a group without answers has no callout, and a journal without callouts no section. Saving
//! a group rewrites that group's callout; questions the app doesn't know are kept as they are.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::vault::note::is_blank;

mod read;
mod write;

pub use read::*;
pub use write::*;

static CALLOUT_START: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*>\s*\[!([^\]]+)\]([+-]?)\s*(.*)$").unwrap());
static CALLOUT_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*>\s?(.*)$").unwrap());
static QUESTION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^\*\*(.+?)\*\*\s*(?:[-–—]\s*(?:_(.+?)_|\*(.+?)\*)\s*)?$").unwrap()
});
static NUMBERED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*[0-9]+[.)]\s+(.*)$").unwrap());
static BULLET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*[-*+]\s+(.*)$").unwrap());
static TASK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*[-*+]\s\[([ xX])\]\s?(.*)$").unwrap());

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default)]
pub struct JournalTask {
    pub text: String,
    pub done: bool,
}

/// One question's answer as the note holds it; only the field of the question's kind is used.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default)]
pub struct JournalAnswer {
    pub items: Vec<String>,
    pub tasks: Vec<JournalTask>,
    pub text: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(default)]
pub struct JournalGroupAnswers {
    /// By question text; a question without an entry is unanswered.
    pub answers: BTreeMap<String, JournalAnswer>,
    /// The free text of a group without questions.
    pub note: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParsedQuestion {
    pub question: String,
    /// Callout lines (without `> `) after the question line, up to a blank `>` line or the next
    /// question.
    pub slots: Vec<String>,
    /// The question line and its slots as written, for questions the app doesn't know.
    raw: Vec<String>,
}

/// One callout: `start` is its `> [!TYPE]` line, `end` the first line after it.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedGroup {
    pub title: String,
    header: String,
    start: usize,
    end: usize,
    pub questions: Vec<ParsedQuestion>,
    /// Callout lines without the `> ` prefix.
    body: Vec<String>,
}

fn key(text: &str) -> String {
    text.replace(['“', '”', '„'], "\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// `?` marks an unanswered slot.
fn is_placeholder(text: &str) -> bool {
    matches!(text.trim(), "" | "?")
}

/// `lines` without the blank lines at either end.
fn trimmed_block<S: AsRef<str>>(lines: &[S]) -> &[S] {
    let start = lines.iter().position(|line| !is_blank(line.as_ref()));
    let end = lines.iter().rposition(|line| !is_blank(line.as_ref()));
    match (start, end) {
        (Some(start), Some(end)) => &lines[start..=end],
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::{AnswerKind, Settings};
    use crate::vault::note::Note;

    const HEADING: &str = "## Journal";

    fn order() -> Vec<String> {
        ["## Journal", "## Plan for Today", "## Activity", "## Notes"]
            .map(String::from)
            .to_vec()
    }

    fn try_write(text: &str, group: &str, answers: &JournalGroupAnswers) -> Result<String, String> {
        let mut note = Note::parse(text);
        write_journal_group(
            &mut note,
            HEADING,
            &order(),
            &Settings::default().journal,
            group,
            answers,
        )?;
        Ok(note.serialize())
    }

    fn write(text: &str, group: &str, answers: &JournalGroupAnswers) -> String {
        try_write(text, group, answers).unwrap()
    }

    fn morgen(partial: Vec<(&str, JournalAnswer)>) -> JournalGroupAnswers {
        JournalGroupAnswers {
            answers: partial
                .into_iter()
                .map(|(question, answer)| (question.to_string(), answer))
                .collect(),
            note: String::new(),
        }
    }

    fn items(items: &[&str]) -> JournalAnswer {
        JournalAnswer {
            items: items.iter().map(|item| item.to_string()).collect(),
            ..Default::default()
        }
    }

    fn text(text: &str) -> JournalAnswer {
        JournalAnswer {
            text: text.into(),
            ..Default::default()
        }
    }

    fn lines(parts: &[&str]) -> String {
        parts.join("\n")
    }

    #[test]
    fn creates_the_section_below_the_title_with_only_the_answered_question() {
        let result = write(
            "# Saturday\n\n## Notes\n",
            "Dawn",
            &morgen(vec![("What am I grateful for?", items(&["Sunshine"]))]),
        );
        assert_eq!(
            result,
            lines(&[
                "# Saturday",
                "",
                "## Journal",
                "",
                "> [!NOTE]+ Dawn",
                "> **What am I grateful for?** - _Small things count too_",
                "> 1. Sunshine",
                "",
                "## Notes",
                "",
            ])
        );
    }

    #[test]
    fn writes_as_many_answers_as_given_and_an_optional_task() {
        let result = write(
            "# Day\n",
            "Dawn",
            &morgen(vec![
                ("What am I grateful for?", items(&["a", "b", "c", "d", "e"])),
                (
                    "The \"One Thing\"",
                    JournalAnswer {
                        tasks: vec![JournalTask {
                            text: "Blogpost".into(),
                            done: true,
                        }],
                        ..Default::default()
                    },
                ),
            ]),
        );
        assert!(
            result.contains(&lines(&[
                "> 5. e",
                ">",
                "> **The \"One Thing\"** - _What would I do today even without pay?_",
                "> - [x] Blogpost",
            ])),
            "{result}"
        );
        assert!(!result.contains("My current goal"), "{result}");
    }

    #[test]
    fn replaces_the_old_templates_placeholder_callout_and_keeps_its_fold_state() {
        let note = lines(&[
            "## Journal",
            "",
            "> [!NOTE]- Dawn",
            "> **What am I grateful for?** - _Small things count too_",
            "> 1. ?",
            "> 2. ?",
            ">",
            "> **My current goal** - _Where do I want to be in a year?_",
            "> ?",
            "",
            "> [!NOTE]+ Dusk",
            "> **What did I learn today?**",
            "> ?",
            "",
        ]);
        let result = write(
            &note,
            "Dawn",
            &morgen(vec![("My current goal", text("Work calmly"))]),
        );
        assert_eq!(
            result,
            lines(&[
                "## Journal",
                "",
                "> [!NOTE]- Dawn",
                "> **My current goal** - _Where do I want to be in a year?_",
                "> Work calmly",
                "",
                "> [!NOTE]+ Dusk",
                "> **What did I learn today?**",
                "> ?",
                "",
            ])
        );
    }

    #[test]
    fn removes_the_callout_and_the_empty_section_when_the_last_answer_is_cleared() {
        let note =
            "# Day\n\n## Journal\n\n> [!NOTE]+ Dawn\n> **My current goal**\n> x\n\n## Notes\n";
        assert_eq!(write(note, "Dawn", &morgen(vec![])), "# Day\n\n## Notes\n");
    }

    #[test]
    fn inserts_a_group_in_settings_order() {
        let note = "## Journal\n\n> [!NOTE]+ Dusk\n> **What did I learn today?**\n> viel\n";
        let result = write(note, "Dawn", &morgen(vec![("My current goal", text("x"))]));
        assert!(result.find("Dawn") < result.find("Dusk"), "{result}");
    }

    #[test]
    fn keeps_questions_the_app_does_not_know() {
        let note = "## Journal\n\n> [!NOTE]+ Dawn\n> **Eigene Frage**\n> meine Antwort\n";
        let result = write(note, "Dawn", &morgen(vec![("My current goal", text("x"))]));
        assert!(
            result.contains("> **Eigene Frage**\n> meine Antwort"),
            "{result}"
        );
    }

    #[test]
    fn writes_free_text_groups_and_drops_blank_lines_inside_text_answers() {
        let health = JournalGroupAnswers {
            answers: BTreeMap::new(),
            note: "Slept well.\n\nTired.".into(),
        };
        let result = write("# Day\n", "Health", &health);
        assert!(
            result.contains("> [!HEART]- Health\n> Slept well.\n>\n> Tired."),
            "{result}"
        );
        let result = write(
            "# Day\n",
            "Dawn",
            &morgen(vec![("My current goal", text("a\n\nb"))]),
        );
        assert!(result.contains("> a\n> b"), "{result}");
    }

    #[test]
    fn writing_a_group_that_is_not_configured_is_an_error() {
        let answers = morgen(vec![("My current goal", text("x"))]);
        assert_eq!(
            try_write("# Day\n", "Nachts", &answers),
            Err("“Nachts” isn't a journal group.".to_string())
        );
    }

    #[test]
    fn reads_items_tasks_and_text_and_ignores_placeholders() {
        let note: Vec<String> = [
            "## Journal",
            "",
            "> [!NOTE]+ Dawn",
            "> **What am I grateful for?** - _hint_",
            "> 1. Sunshine",
            "> 2. ?",
            ">",
            "> **The \"One Thing\"**",
            "> - [ ] ?",
        ]
        .map(String::from)
        .to_vec();
        let groups = parse_journal_groups(&note, HEADING);
        let config = &Settings::default().journal[0];
        let answers = group_answers(config, find_group(groups.as_deref(), "Dawn")).answers;
        assert_eq!(answers["What am I grateful for?"].items, vec!["Sunshine"]);
        assert_eq!(
            answers["The \"One Thing\""].tasks,
            Vec::<JournalTask>::new()
        );
        assert_eq!(answers["My current goal"].text, "");
        assert!(is_answered(
            answers.get("What am I grateful for?"),
            AnswerKind::List
        ));
        assert!(!is_answered(
            answers.get("The \"One Thing\""),
            AnswerKind::Task
        ));
    }
}
