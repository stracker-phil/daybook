//! Quick notes: the `###` (and deeper) sections inside the notes section of today's note (default
//! `## Notes`), read and rewritten one section at a time.

use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::vault::note::{
    Note, body_end, find_heading, heading_level, insert_lines, is_blank, is_fence, place_heading,
    section_end,
};

static SUB_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.+?)\s*#*\s*$").unwrap());

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct NoteSectionRef {
    pub title: String,
    /// 3 for `###`, 4 for `####`, …
    pub level: u8,
}

impl NoteSectionRef {
    fn is(&self, other: &Self) -> bool {
        self.level == other.level
            && self.title.trim().to_lowercase() == other.title.trim().to_lowercase()
    }
}

/// Headings below the notes heading, inside its section, outside code blocks, in note order.
fn sub_headings(lines: &[String], notes_index: usize) -> Vec<(usize, NoteSectionRef)> {
    let notes_level = match heading_level(lines[notes_index].trim()) {
        0 => 2,
        level => level,
    };
    let mut found = Vec::new();
    let mut in_fence = false;
    for (i, line) in lines
        .iter()
        .enumerate()
        .take(section_end(lines, notes_index))
        .skip(notes_index + 1)
    {
        let trimmed = line.trim();
        if is_fence(trimmed) {
            in_fence = !in_fence;
        }
        if in_fence {
            continue;
        }
        if let Some(heading) = SUB_HEADING.captures(trimmed)
            && heading[1].len() > notes_level
        {
            let level = heading[1].len() as u8;
            let title = heading[2].to_string();
            found.push((i, NoteSectionRef { title, level }));
        }
    }
    found
}

fn find_section(lines: &[String], notes_index: usize, wanted: &NoteSectionRef) -> Option<usize> {
    sub_headings(lines, notes_index)
        .into_iter()
        .find(|(_, section)| section.is(wanted))
        .map(|(index, _)| index)
}

pub fn list_note_sections(lines: &[String], notes_heading: &str) -> Vec<NoteSectionRef> {
    find_heading(lines, notes_heading).map_or_else(Vec::new, |notes_index| {
        sub_headings(lines, notes_index)
            .into_iter()
            .map(|(_, section)| section)
            .collect()
    })
}

/// The section's text (its own subsections included), or `None` when it doesn't exist.
pub fn read_note_section(
    lines: &[String],
    notes_heading: &str,
    section: &NoteSectionRef,
) -> Option<String> {
    let notes_index = find_heading(lines, notes_heading)?;
    let index = find_section(lines, notes_index, section)?;
    let text = lines[index + 1..body_end(lines, index)].join("\n");
    Some(text.trim_start_matches('\n').to_string())
}

pub const SECTION_CHANGED: &str = "The topic changed in the note meanwhile.";

/// Whether the section's text is `expected`, a missing section's text being empty. Blank lines around
/// the text don't count, as writing drops them.
pub fn section_holds(
    lines: &[String],
    notes_heading: &str,
    section: &NoteSectionRef,
    expected: &str,
) -> bool {
    let current = read_note_section(lines, notes_heading, section).unwrap_or_default();
    body_lines(&current) == body_lines(expected)
}

/// The body without leading blank lines and trailing whitespace; the first line keeps its indent.
fn body_lines(body: &str) -> Vec<String> {
    let indent = body.len() - body.trim_start().len();
    let body = match body[..indent].rfind('\n') {
        Some(newline) => &body[newline + 1..],
        None => body,
    };
    let trimmed = body.trim_end();
    if trimmed.is_empty() {
        Vec::new()
    } else {
        trimmed.split('\n').map(String::from).collect()
    }
}

/// Replaces a section's text and, with a new title, renames it. A missing section is added at the
/// end of the notes section; a missing notes section is created at its place in `order`.
pub fn write_note_section(
    note: &mut Note,
    notes_heading: &str,
    order: &[String],
    section: &NoteSectionRef,
    new_title: &str,
    body: &str,
) {
    let notes_index = match find_heading(&note.lines, notes_heading) {
        Some(index) => index,
        None => place_heading(note, notes_heading, order, Vec::new()),
    };
    let new_title = new_title.trim();
    let title = if new_title.is_empty() {
        section.title.trim()
    } else {
        new_title
    };
    let heading = format!("{} {title}", "#".repeat(section.level.into()));
    // Obsidian wants a blank line before and after every heading.
    let content = body_lines(body);
    let mut below = Vec::new();
    if !content.is_empty() {
        below.push(String::new());
        below.extend(content);
    }
    let found = find_section(&note.lines, notes_index, section);
    let lines = &mut note.lines;
    let Some(mut at) = found else {
        let last = (notes_index + 1..section_end(lines, notes_index))
            .rev()
            .find(|&i| !is_blank(&lines[i]))
            .unwrap_or(notes_index);
        let mut block = vec![String::new(), heading];
        block.extend(below);
        insert_lines(lines, last + 1, block);
        return;
    };
    if !is_blank(&lines[at - 1]) {
        lines.insert(at, String::new());
        at += 1;
    }
    lines[at] = heading;
    let after = at + 1 + below.len();
    let end = body_end(lines, at);
    lines.splice(at + 1..end, below);
    if lines.get(after).is_some_and(|line| !is_blank(line)) {
        lines.insert(after, String::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTE: &str = "# Friday\n\n## Notes\n\n### Standup\n- one\n- two\n\n#### Details\ntext\n\n### Review\nnotes\n\n## Activity\n- 09:00 _working_";

    fn order() -> Vec<String> {
        ["## Journal", "## Plan for Today", "## Activity", "## Notes"]
            .map(String::from)
            .to_vec()
    }

    fn section(title: &str, level: u8) -> NoteSectionRef {
        NoteSectionRef {
            title: title.into(),
            level,
        }
    }

    fn write(text: &str, title: &str, new_title: &str, body: &str) -> String {
        let mut note = Note::parse(text);
        write_note_section(
            &mut note,
            "## Notes",
            &order(),
            &section(title, 3),
            new_title,
            body,
        );
        note.serialize()
    }

    #[test]
    fn lists_the_sections_below_the_notes_heading() {
        assert_eq!(
            list_note_sections(&Note::parse(NOTE).lines, "## Notes"),
            vec![
                section("Standup", 3),
                section("Details", 4),
                section("Review", 3)
            ]
        );
    }

    #[test]
    fn reads_a_section_with_its_subsections() {
        let lines = Note::parse(NOTE).lines;
        assert_eq!(
            read_note_section(&lines, "## Notes", &section("standup", 3)),
            Some("- one\n- two\n\n#### Details\ntext".to_string())
        );
        assert_eq!(
            read_note_section(&lines, "## Notes", &section("Missing", 3)),
            None
        );
    }

    #[test]
    fn replaces_a_sections_text_and_keeps_the_rest() {
        let result = write(NOTE, "Review", "Review", "- done\n- next");
        assert!(
            result.contains("\n\n### Review\n\n- done\n- next\n\n## Activity"),
            "{result}"
        );
        assert!(result.contains("### Standup\n- one\n- two"), "{result}");
    }

    #[test]
    fn renames_a_section() {
        let result = write(NOTE, "Review", "Sprint review", "notes");
        assert!(
            result.contains("### Sprint review\n\nnotes\n\n## Activity"),
            "{result}"
        );
    }

    #[test]
    fn adds_a_new_section_at_the_end_of_the_notes() {
        let result = write(NOTE, "Call with Anna", "Call with Anna", "");
        assert!(
            result.contains("### Review\nnotes\n\n### Call with Anna\n\n## Activity"),
            "{result}"
        );
    }

    #[test]
    fn creates_the_notes_section_at_its_place_when_it_is_missing() {
        assert_eq!(
            write(
                "# Friday\n\n## Journal\n\nx\n\n## Activity\n- 09:00 _working_\n",
                "Standup",
                "Standup",
                "- hi"
            ),
            "# Friday\n\n## Journal\n\nx\n\n## Activity\n- 09:00 _working_\n\n## Notes\n\n### Standup\n\n- hi\n"
        );
    }

    #[test]
    fn puts_a_blank_line_before_and_after_a_heading_it_rewrites() {
        let tight = "## Notes\n### Standup\n- one\n### Review\nnotes\n";
        assert_eq!(
            write(tight, "Standup", "Standup", "- one\n- two"),
            "## Notes\n\n### Standup\n\n- one\n- two\n\n### Review\nnotes\n"
        );
    }

    #[test]
    fn a_section_holds_its_text_ignoring_blank_lines_around_it() {
        let lines = Note::parse(NOTE).lines;
        let review = section("Review", 3);
        for expected in ["notes", "notes\n", "\n\nnotes\n\n", "notes  \n"] {
            assert!(
                section_holds(&lines, "## Notes", &review, expected),
                "{expected:?}"
            );
        }
        for expected in ["", "other", "notes\nmore"] {
            assert!(
                !section_holds(&lines, "## Notes", &review, expected),
                "{expected:?}"
            );
        }
    }

    #[test]
    fn a_missing_section_or_notes_heading_holds_empty_text() {
        let lines = Note::parse(NOTE).lines;
        assert!(section_holds(
            &lines,
            "## Notes",
            &section("Missing", 3),
            "\n"
        ));
        assert!(!section_holds(
            &lines,
            "## Notes",
            &section("Missing", 3),
            "x"
        ));
        assert!(section_holds(
            &lines,
            "## Elsewhere",
            &section("Review", 3),
            ""
        ));
    }

    #[test]
    fn a_section_holds_its_subsections_text_too() {
        let lines = Note::parse(NOTE).lines;
        assert!(section_holds(
            &lines,
            "## Notes",
            &section("Standup", 3),
            "- one\n- two\n\n#### Details\ntext"
        ));
        assert!(!section_holds(
            &lines,
            "## Notes",
            &section("Standup", 3),
            "- one\n- two"
        ));
    }

    #[test]
    fn reads_a_section_without_the_blank_line_after_its_heading() {
        let written = write(NOTE, "Review", "Review", "- done");
        assert_eq!(
            read_note_section(
                &Note::parse(&written).lines,
                "## Notes",
                &section("Review", 3)
            ),
            Some("- done".to_string())
        );
    }
}
