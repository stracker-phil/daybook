//! The day's focus list in its own section of today's note (default `## Focus`), kept as typed: a
//! bullet list whose tasks carry their checkbox as inline code (``- `[/]` :nexo: Task``), so Obsidian
//! plugins don't count them as todos.

use std::sync::LazyLock;

use regex::Regex;

use crate::vault::note::{Note, body_end, find_heading, is_blank, replace_section};

/// A list written as a Markdown checklist (`- [x] Task`, by hand or before the format changed).
static CHECKLIST_TASK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*[-*+]\s+)\[(.)\](\s|$)").unwrap());

/// The body's lines without trailing spaces, blank lines around them dropped.
fn body_lines(body: &str) -> Vec<String> {
    let lines: Vec<String> = body
        .split('\n')
        .map(|line| line.trim_end().to_string())
        .collect();
    let start = lines.iter().position(|line| !line.is_empty());
    let end = lines.iter().rposition(|line| !line.is_empty());
    match (start, end) {
        (Some(start), Some(end)) => lines[start..=end].to_vec(),
        _ => Vec::new(),
    }
}

/// The section's text, or "" when the note has none. Checklist tasks are read as focus tasks.
pub fn read_focus(lines: &[String], heading: &str) -> String {
    let Some(index) = find_heading(lines, heading) else {
        return String::new();
    };
    let end = body_end(lines, index);
    let start = (index + 1..end)
        .find(|&i| !is_blank(&lines[i]))
        .unwrap_or(end);
    lines[start..end]
        .iter()
        .map(|line| CHECKLIST_TASK.replace(line, "$1`[$2]`$3").into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Replaces the section's text; a missing section is created once there is text.
pub fn write_focus(note: &mut Note, heading: &str, order: &[String], body: &str) {
    replace_section(note, heading, order, body_lines(body));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn order() -> Vec<String> {
        [
            "## Journal",
            "## Focus",
            "## Plan for Today",
            "## Activity",
            "## Notes",
        ]
        .map(String::from)
        .to_vec()
    }

    fn write(text: &str, body: &str) -> String {
        let mut note = Note::parse(text);
        write_focus(&mut note, "## Focus", &order(), body);
        note.serialize()
    }

    #[test]
    fn creates_the_section_at_its_place_in_the_order() {
        assert_eq!(
            write(
                "# Day\n\n## Journal\n\ntext\n\n## Activity\n",
                "- [ ] :nexo: Plan\n"
            ),
            "# Day\n\n## Journal\n\ntext\n\n## Focus\n\n- [ ] :nexo: Plan\n\n## Activity\n"
        );
    }

    #[test]
    fn replaces_the_list_and_keeps_what_follows() {
        let text = "## Focus\n\n- [ ] One\n- [ ] Two\n\n## Activity\n\n- 09:00 _working_\n";
        assert_eq!(
            write(text, "- [x] One\n    - Outcome / ID-1\n- [/] Three"),
            "## Focus\n\n- [x] One\n    - Outcome / ID-1\n- [/] Three\n\n## Activity\n\n- 09:00 _working_\n"
        );
    }

    #[test]
    fn changes_nothing_when_the_list_is_the_same_and_does_not_create_an_empty_section() {
        let text = "## Focus\n\n- [ ] One\n\n## Activity\n";
        assert_eq!(write(text, "- [ ] One"), text);
        assert_eq!(write("# Day\n", "  \n"), "# Day\n");
    }

    #[test]
    fn reads_the_list_as_written_and_a_markdown_checklist_as_focus_tasks() {
        let note = Note::parse("## Focus\n\n- [x] One\n    - sub\n\n## Activity\n");
        assert_eq!(
            read_focus(&note.lines, "## Focus"),
            "- `[x]` One\n    - sub"
        );
        assert_eq!(read_focus(&note.lines, "## Nope"), "");
    }
}
