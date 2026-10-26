//! How the focus list shows on the board: each task's checkbox, its project (from the Slack emoji the
//! line starts with) and text; other lines as plain sub-items. Pure.

use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;
use ts_rs::TS;

use crate::store::settings::Project;

static TASK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[-*+]\s+`?\[(.)\]`?\s*(.*)$").unwrap());
static LEADING_EMOJI: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(:[\w+-]+:)\s*").unwrap());
static BULLET: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[-*+]\s+").unwrap());

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "kebab-case")]
pub enum FocusMark {
    Open,
    InProgress,
    Done,
    Cancelled,
}

impl FocusMark {
    fn from_box(mark: &str) -> Self {
        match mark.to_lowercase().as_str() {
            "/" => Self::InProgress,
            "x" => Self::Done,
            "-" => Self::Cancelled,
            _ => Self::Open,
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct FocusLine {
    /// `None`: a plain line, shown as a sub-item.
    pub mark: Option<FocusMark>,
    /// The project of the line's emoji; the shortcode itself when no project has it.
    pub tag: Option<String>,
    pub text: String,
}

/// Splits a leading `:shortcode:` off the text, as its project's name.
fn split_tag(text: &str, projects: &[Project]) -> (Option<String>, String) {
    let Some(found) = LEADING_EMOJI.captures(text) else {
        return (None, text.to_string());
    };
    let emoji = &found[1];
    let tag = projects
        .iter()
        .find(|project| project.slack_emoji == emoji)
        .map_or_else(|| emoji.to_string(), |project| project.name.clone());
    (Some(tag), text[found[0].len()..].to_string())
}

/// The list's lines as the board shows them; blank lines left out.
pub fn focus_lines(body: &str, projects: &[Project]) -> Vec<FocusLine> {
    body.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| match TASK.captures(line) {
            Some(task) => {
                let (tag, text) = split_tag(&task[2], projects);
                FocusLine {
                    mark: Some(FocusMark::from_box(&task[1])),
                    tag,
                    text,
                }
            }
            None => FocusLine {
                mark: None,
                tag: None,
                text: BULLET.replace(line.trim(), "").into_owned(),
            },
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projects() -> Vec<Project> {
        let mut nexo = Project::new("Nexo");
        nexo.slack_emoji = ":nexo:".into();
        let mut oscorp = Project::new("Oscorp");
        oscorp.slack_emoji = ":oscorp:".into();
        vec![nexo, oscorp, Project::new("Internal")]
    }

    fn line(mark: Option<FocusMark>, tag: Option<&str>, text: &str) -> FocusLine {
        FocusLine {
            mark,
            tag: tag.map(String::from),
            text: text.into(),
        }
    }

    #[test]
    fn each_checkbox_character_maps_to_its_mark() {
        for (boxed, expected) in [
            (" ", FocusMark::Open),
            ("/", FocusMark::InProgress),
            ("x", FocusMark::Done),
            ("X", FocusMark::Done),
            ("-", FocusMark::Cancelled),
        ] {
            assert_eq!(
                focus_lines(&format!("- `[{boxed}]` Task"), &[]),
                vec![line(Some(expected), None, "Task")],
                "{boxed:?}"
            );
        }
    }

    #[test]
    fn a_markdown_checklist_item_counts_as_a_task() {
        assert_eq!(
            focus_lines("- [x] Task\n* [/] Other", &[]),
            vec![
                line(Some(FocusMark::Done), None, "Task"),
                line(Some(FocusMark::InProgress), None, "Other"),
            ]
        );
    }

    #[test]
    fn a_leading_emoji_becomes_the_projects_name() {
        assert_eq!(
            focus_lines("- `[ ]` :oscorp:  Fix v2", &projects()),
            vec![line(Some(FocusMark::Open), Some("Oscorp"), "Fix v2")]
        );
    }

    #[test]
    fn an_emoji_of_no_project_stays_as_the_tag() {
        assert_eq!(
            focus_lines("- `[ ]` :wave: Test", &projects()),
            vec![line(Some(FocusMark::Open), Some(":wave:"), "Test")]
        );
    }

    #[test]
    fn a_task_without_an_emoji_has_no_tag() {
        assert_eq!(
            focus_lines("- `[/]` Plain", &projects()),
            vec![line(Some(FocusMark::InProgress), None, "Plain")]
        );
    }

    #[test]
    fn other_lines_are_plain_sub_items_without_bullet_and_indentation() {
        assert_eq!(
            focus_lines(
                "- `[x]` :nexo: Test\n    - Fixed it / ID-2\n        - Deeper\n\tJust text",
                &projects()
            ),
            vec![
                line(Some(FocusMark::Done), Some("Nexo"), "Test"),
                line(None, None, "Fixed it / ID-2"),
                line(None, None, "Deeper"),
                line(None, None, "Just text"),
            ]
        );
    }

    #[test]
    fn blank_lines_are_left_out() {
        assert_eq!(
            focus_lines("\n- `[ ]` One\n  \n\n- `[ ]` Two\n", &[]),
            vec![
                line(Some(FocusMark::Open), None, "One"),
                line(Some(FocusMark::Open), None, "Two"),
            ]
        );
        assert_eq!(focus_lines("", &[]), vec![]);
    }

    #[test]
    fn a_line_serializes_with_a_kebab_case_mark() {
        assert_eq!(
            serde_json::to_value(line(Some(FocusMark::InProgress), Some("Nexo"), "Test")).unwrap(),
            serde_json::json!({ "mark": "in-progress", "tag": "Nexo", "text": "Test" })
        );
    }
}
