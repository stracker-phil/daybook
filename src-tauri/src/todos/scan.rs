//! Unticked tasks in a note, with the headings they sit under. Tasks inside callouts count too
//! (`> - [ ] …`, e.g. a journal answer); empty `- [ ] ?` slots don't. Pure.

use std::sync::LazyLock;

use regex::Regex;

static HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.+?)\s*#*\s*$").unwrap());
static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(```|~~~)").unwrap());
static OPEN_TASK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\s*(?:>\s*)*[-*+]\s\[ \]\s+(.*?)\s*$").unwrap());

#[derive(Clone, Debug, PartialEq)]
pub struct ScannedTodo {
    pub text: String,
    /// The headings from level 2 down, e.g. "Notes › Meeting Two"; empty above the first one.
    pub section: String,
    /// 0-based.
    pub line: usize,
}

/// Case and spacing ignored.
pub fn todo_key(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// `## Focus:` and `## focus` are the same heading: same level, case and edge punctuation ignored.
fn heading_key(level: usize, title: &str) -> String {
    let edge = |c: char| !c.is_alphanumeric();
    format!("{level} {}", title.trim_matches(edge).to_lowercase())
}

/// The open todos of `text`; the tasks under `skip_heading` (the focus list) aren't todos, up to the next
/// heading of its level or above.
pub fn scan_open_todos(text: &str, skip_heading: &str) -> Vec<ScannedTodo> {
    let skip_key = HEADING
        .captures(skip_heading.trim())
        .map(|heading| heading_key(heading[1].len(), &heading[2]));
    let mut todos = Vec::new();
    let mut trail: Vec<String> = Vec::new();
    let mut skip_level = 0;
    let mut in_fence = false;
    for (index, line) in text.lines().enumerate() {
        if FENCE.is_match(line) {
            in_fence = !in_fence;
            continue;
        }
        if in_fence {
            continue;
        }
        if let Some(heading) = HEADING.captures(line) {
            let level = heading[1].len();
            if skip_level > 0 && level <= skip_level {
                skip_level = 0;
            }
            if skip_key.as_deref() == Some(heading_key(level, &heading[2]).as_str()) {
                skip_level = level;
            }
            if level >= 2 {
                trail.resize(level - 2, String::new());
                trail.push(heading[2].to_string());
            }
            continue;
        }
        if skip_level > 0 {
            continue;
        }
        let Some(task) = OPEN_TASK.captures(line) else {
            continue;
        };
        let content = &task[1];
        if content.is_empty() || content == "?" {
            continue;
        }
        todos.push(ScannedTodo {
            text: content.to_string(),
            section: trail
                .iter()
                .filter(|title| !title.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" › "),
            line: index,
        });
    }
    todos
}

#[cfg(test)]
mod tests {
    use super::*;

    fn todo(text: &str, section: &str, line: usize) -> ScannedTodo {
        ScannedTodo {
            text: text.into(),
            section: section.into(),
            line,
        }
    }

    fn texts(note: &str) -> Vec<String> {
        scan_open_todos(note, "")
            .into_iter()
            .map(|todo| todo.text)
            .collect()
    }

    #[test]
    fn finds_unticked_tasks_with_their_section_also_in_callouts() {
        let note = [
            "# Day",
            "## Journal",
            "> [!NOTE]+ Dawn",
            "> **The \"One Thing\"**",
            "> - [ ] ?",
            "> - [ ] Draft the budget",
            "## Plan for Today",
            "- [ ] Book meeting rooms",
            "- [x] Done already",
            "- [ ] Call the plumber",
            "## Notes",
            "### Meeting Two",
            "- [ ] Follow up with Chris",
            "```",
            "- [ ] not a task (code)",
            "```",
        ]
        .join("\n");

        assert_eq!(
            scan_open_todos(&note, ""),
            vec![
                todo("Draft the budget", "Journal", 5),
                todo("Book meeting rooms", "Plan for Today", 7),
                todo("Call the plumber", "Plan for Today", 9),
                todo("Follow up with Chris", "Notes › Meeting Two", 12),
            ]
        );
    }

    #[test]
    fn the_skipped_section_runs_up_to_the_next_heading_of_its_level() {
        let note = "## Focus:\n- [ ] :nexo: Plan\n### Later\n- [ ] Still focus\n## Notes\n- [ ] Call Anna\n";

        assert_eq!(
            scan_open_todos(note, "## focus"),
            vec![todo("Call Anna", "Notes", 5)]
        );
        assert_eq!(scan_open_todos(note, "").len(), 3);
    }

    #[test]
    fn todo_keys_ignore_case_and_spacing() {
        assert_eq!(todo_key("  Book   MEETING\trooms "), "book meeting rooms");
        assert_eq!(
            todo_key("Book meeting rooms"),
            todo_key("book  Meeting Rooms")
        );
    }

    #[test]
    fn star_and_plus_bullets_count_and_ticked_or_empty_tasks_do_not() {
        let note = "* [ ] Star\n+ [ ] Plus\n- [x] Ticked\n- [ ]\n- [ ] ?\n";

        assert_eq!(texts(note), ["Star", "Plus"]);
    }

    #[test]
    fn a_level_one_heading_is_not_part_of_the_section_path() {
        let note = "# Day\n- [ ] Before\n## Notes\n- [ ] After\n# Other\n- [ ] Last\n";

        assert_eq!(
            scan_open_todos(note, ""),
            vec![
                todo("Before", "", 1),
                todo("After", "Notes", 3),
                todo("Last", "Notes", 5),
            ]
        );
    }

    #[test]
    fn a_tilde_fence_hides_its_tasks_too() {
        let note = "~~~\n- [ ] code\n~~~\n- [ ] real\n";

        assert_eq!(texts(note), ["real"]);
    }
}
