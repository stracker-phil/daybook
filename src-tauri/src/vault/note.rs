//! Pure, line-based edits of a Markdown note: every function changes only the lines it has to, and
//! line endings and the trailing newline stay as found.

use std::sync::LazyLock;

use regex::Regex;

static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,6})\s+\S").unwrap());
static HEADING_TEXT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(#{1,6})\s+(.+)$").unwrap());
static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(```|~~~)").unwrap());
static EDGE_PUNCTUATION: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\p{P}\s]+|[\p{P}\s]+$").unwrap());

#[derive(Clone, Debug, PartialEq)]
pub struct Note {
    pub lines: Vec<String>,
    eol: &'static str,
    trailing_newline: bool,
}

impl Note {
    pub fn parse(text: &str) -> Self {
        let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
        let body = text
            .strip_suffix('\n')
            .map(|rest| rest.strip_suffix('\r').unwrap_or(rest))
            .unwrap_or(text);
        // `"\n"` is one blank line; only an empty text has none.
        let lines = if text.is_empty() {
            Vec::new()
        } else {
            body.split('\n')
                .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
                .collect()
        };
        Self {
            lines,
            eol,
            trailing_newline: text.is_empty() || text.ends_with('\n'),
        }
    }

    pub fn serialize(&self) -> String {
        let body = self.lines.join(self.eol);
        if self.trailing_newline && !self.lines.is_empty() {
            body + self.eol
        } else {
            body
        }
    }
}

pub fn is_blank(line: &str) -> bool {
    line.trim().is_empty()
}

pub fn is_fence(trimmed: &str) -> bool {
    FENCE.is_match(trimmed)
}

/// The first line after the frontmatter, 0 without one.
fn body_start(lines: &[String]) -> usize {
    if lines.first().map(|line| line.trim()) != Some("---") {
        return 0;
    }
    lines
        .iter()
        .skip(1)
        .position(|line| matches!(line.trim(), "---" | "..."))
        .map_or(0, |end| end + 2)
}

/// 2 for `## Notes`, 0 for anything that isn't a heading.
pub fn heading_level(line: &str) -> usize {
    HEADING
        .captures(line)
        .map_or(0, |captures| captures[1].len())
}

/// `## Notes:` and `## notes` are the same heading: same level, case-insensitive, edge punctuation
/// trimmed.
fn heading_key(line: &str) -> String {
    let trimmed = line.trim();
    match HEADING_TEXT.captures(trimmed) {
        Some(captures) => format!(
            "{} {}",
            &captures[1],
            EDGE_PUNCTUATION
                .replace_all(&captures[2], "")
                .to_lowercase()
        ),
        None => trimmed.to_string(),
    }
}

/// The heading's line outside code blocks and the frontmatter, searched in `from..to`.
pub fn find_heading_in(lines: &[String], heading: &str, from: usize, to: usize) -> Option<usize> {
    let wanted = heading_key(heading);
    let mut in_fence = false;
    for (i, line) in lines
        .iter()
        .enumerate()
        .take(to)
        .skip(from.max(body_start(lines)))
    {
        let trimmed = line.trim();
        if is_fence(trimmed) {
            in_fence = !in_fence;
        } else if !in_fence && HEADING.is_match(trimmed) && heading_key(trimmed) == wanted {
            return Some(i);
        }
    }
    None
}

pub fn find_heading(lines: &[String], heading: &str) -> Option<usize> {
    find_heading_in(lines, heading, 0, lines.len())
}

/// The next heading of the same or a higher level after `heading_index`, or the end of the note.
pub fn section_end(lines: &[String], heading_index: usize) -> usize {
    let level = heading_level(&lines[heading_index]);
    let mut in_fence = false;
    for (i, line) in lines.iter().enumerate().skip(heading_index + 1) {
        if is_fence(line.trim()) {
            in_fence = !in_fence;
        } else if !in_fence {
            let other = heading_level(line);
            if other > 0 && other <= level {
                return i;
            }
        }
    }
    lines.len()
}

/// The line after the section's last non-empty line.
pub fn body_end(lines: &[String], heading_index: usize) -> usize {
    let mut end = section_end(lines, heading_index);
    while end - 1 > heading_index && is_blank(&lines[end - 1]) {
        end -= 1;
    }
    end
}

/// Inserts `new_lines` at `at`, with a blank line after them when text follows.
pub fn insert_lines(lines: &mut Vec<String>, at: usize, new_lines: Vec<String>) {
    let after = at + new_lines.len();
    lines.splice(at..at, new_lines);
    if lines.get(after).is_some_and(|next| !is_blank(next)) {
        lines.insert(after, String::new());
    }
}

fn insert_below_heading(lines: &mut Vec<String>, heading_index: usize, new_lines: Vec<String>) {
    if lines
        .get(heading_index + 1)
        .is_some_and(|line| is_blank(line))
    {
        insert_lines(lines, heading_index + 2, new_lines);
    } else {
        let mut spaced = vec![String::new()];
        spaced.extend(new_lines);
        insert_lines(lines, heading_index + 1, spaced);
    }
}

/// Steps back from `to` over blank lines, never to `floor` or before it.
fn after_content(lines: &[String], floor: usize, to: usize) -> usize {
    let mut i = to;
    while i > floor && is_blank(&lines[i - 1]) {
        i -= 1;
    }
    i
}

/// Where a missing heading goes: right before the next section of `order` that the note has, else
/// after the previous one, else below the frontmatter and the note's `# Title`. So the sections line
/// up the same way whichever is written first. A heading that isn't in `order` goes to the end.
fn heading_insert_index(lines: &[String], heading: &str, order: &[String]) -> usize {
    let start = body_start(lines);
    let wanted = heading_key(heading);
    let Some(mine) = order.iter().position(|other| heading_key(other) == wanted) else {
        return after_content(lines, start, lines.len());
    };
    for next in &order[mine + 1..] {
        if let Some(index) = find_heading(lines, next) {
            return after_content(lines, start, index);
        }
    }
    for previous in order[..mine].iter().rev() {
        if let Some(index) = find_heading(lines, previous) {
            return after_content(lines, index + 1, section_end(lines, index));
        }
    }
    let first = (start..lines.len())
        .find(|&i| !is_blank(&lines[i]))
        .unwrap_or(lines.len());
    if first < lines.len() && heading_level(&lines[first]) == 1 {
        first + 1
    } else {
        start
    }
}

/// Creates `heading` (with optional first lines) at its place in `order`, blank lines around it, and
/// returns its index.
pub fn place_heading(
    note: &mut Note,
    heading: &str,
    order: &[String],
    new_lines: Vec<String>,
) -> usize {
    let lines = &mut note.lines;
    let mut index = heading_insert_index(lines, heading, order);
    if index > 0 && !is_blank(&lines[index - 1]) {
        lines.insert(index, String::new());
        index += 1;
    }
    let mut block = vec![heading.to_string()];
    if !new_lines.is_empty() {
        block.push(String::new());
        block.extend(new_lines);
    }
    insert_lines(lines, index, block);
    index
}

/// Adds lines after the section's last non-empty line. A missing section is created at its place in
/// `order` (at the end when it isn't listed).
pub fn append_to_section(note: &mut Note, heading: &str, new_lines: Vec<String>, order: &[String]) {
    let Some(index) = find_heading(&note.lines, heading) else {
        place_heading(note, heading, order, new_lines);
        return;
    };
    let lines = &mut note.lines;
    let end = section_end(lines, index);
    match (index + 1..end).rev().find(|&i| !is_blank(&lines[i])) {
        Some(last) => insert_lines(lines, last + 1, new_lines),
        None => insert_below_heading(lines, index, new_lines),
    }
}

/// Replaces the section's body with `body`; a missing section is created at its place in `order`
/// once there is a body.
pub fn replace_section(note: &mut Note, heading: &str, order: &[String], body: Vec<String>) {
    let Some(index) = find_heading(&note.lines, heading) else {
        if !body.is_empty() {
            place_heading(note, heading, order, body);
        }
        return;
    };
    let lines = &mut note.lines;
    let mut next = Vec::new();
    if !body.is_empty() {
        next.push(String::new());
        next.extend(body);
    }
    let after = index + 1 + next.len();
    let end = body_end(lines, index);
    lines.splice(index + 1..end, next);
    if lines.get(after).is_some_and(|line| !is_blank(line)) {
        lines.insert(after, String::new());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER: [&str; 4] = ["## Journal", "## Plan for Today", "## Activity", "## Notes"];

    fn v(lines: &[&str]) -> Vec<String> {
        lines.iter().map(|line| line.to_string()).collect()
    }

    fn order() -> Vec<String> {
        v(&ORDER)
    }

    fn edit(lines: &[&str], change: impl FnOnce(&mut Note)) -> Vec<String> {
        let mut note = Note::parse(&lines.join("\n"));
        change(&mut note);
        note.serialize().split('\n').map(str::to_string).collect()
    }

    fn append(lines: &[&str], heading: &str, order: &[String]) -> Vec<String> {
        edit(lines, |note| {
            append_to_section(note, heading, v(&["- new"]), order)
        })
    }

    fn add(lines: &[&str], heading: &str) -> Vec<String> {
        append(lines, heading, &order())
    }

    fn replace(lines: &[&str], heading: &str, body: &[&str]) -> Vec<String> {
        edit(lines, |note| {
            replace_section(note, heading, &order(), v(body))
        })
    }

    mod parsing {
        use super::*;

        #[test]
        fn text_round_trips_exactly() {
            for text in [
                "a\nb\n",
                "a\nb",
                "a\r\nb\r\n",
                "a\r\nb",
                "",
                "\n",
                "\r\n",
                "a\n\n\nb\n",
            ] {
                assert_eq!(Note::parse(text).serialize(), text, "{text:?}");
            }
        }

        #[test]
        fn lines_come_without_their_line_endings() {
            assert_eq!(Note::parse("a\r\nb\r\n").lines, v(&["a", "b"]));
            assert_eq!(Note::parse("a\nb").lines, v(&["a", "b"]));
            assert_eq!(Note::parse("").lines, Vec::<String>::new());
        }

        #[test]
        fn edits_on_a_crlf_note_keep_crlf_and_the_trailing_newline() {
            let mut note = Note::parse("## Notes\r\n- old\r\n");
            append_to_section(&mut note, "## Notes", v(&["- new"]), &order());
            assert_eq!(note.serialize(), "## Notes\r\n- old\r\n- new\r\n");
        }

        #[test]
        fn edits_keep_a_missing_trailing_newline_missing() {
            let mut note = Note::parse("## Notes\n- old");
            append_to_section(&mut note, "## Notes", v(&["- new"]), &order());
            assert_eq!(note.serialize(), "## Notes\n- old\n- new");
        }

        #[test]
        fn a_new_heading_in_an_empty_note_gets_a_trailing_newline() {
            let mut note = Note::parse("");
            append_to_section(&mut note, "## Journal", v(&["- new"]), &order());
            assert_eq!(note.serialize(), "## Journal\n\n- new\n");
        }

        #[test]
        fn untouched_lines_stay_byte_identical() {
            let mut note = Note::parse("## A\n  text with trailing  \n\tTabbed\n## B\nkeep  \n");
            append_to_section(&mut note, "## A", v(&["- new"]), &[]);
            assert_eq!(
                note.serialize(),
                "## A\n  text with trailing  \n\tTabbed\n- new\n\n## B\nkeep  \n"
            );
        }
    }

    mod headings {
        use super::*;

        #[test]
        fn heading_level_counts_the_hashes_of_real_headings_only() {
            for (line, expected) in [
                ("# Title", 1),
                ("## Notes", 2),
                ("###### Deep", 6),
                ("####### Too deep", 0),
                ("#tag", 0),
                ("#", 0),
                ("text", 0),
            ] {
                assert_eq!(heading_level(line), expected, "{line}");
            }
        }

        #[test]
        fn matching_ignores_case_and_edge_punctuation() {
            for (line, wanted) in [
                ("## Notes:", "## notes"),
                ("## Activity Log:", "## Activity Log"),
                ("## Activity", "## activity!"),
                ("## log items", "## Log Items"),
            ] {
                assert_eq!(
                    find_heading(&v(&[line]), wanted),
                    Some(0),
                    "{line} / {wanted}"
                );
            }
        }

        #[test]
        fn matching_does_not_ignore_the_level() {
            assert_eq!(find_heading(&v(&["### Log Items"]), "## Log Items"), None);
            assert_eq!(find_heading(&v(&["## Log Items"]), "### Log Items"), None);
        }

        #[test]
        fn headings_inside_code_blocks_are_not_found() {
            assert_eq!(
                find_heading(
                    &v(&["```", "## Log Items", "```", "## Log Items"]),
                    "## Log Items"
                ),
                Some(3)
            );
            assert_eq!(
                find_heading(&v(&["~~~", "## Log Items", "~~~"]), "## Log Items"),
                None
            );
        }

        #[test]
        fn headings_inside_the_frontmatter_are_not_found() {
            assert_eq!(
                find_heading(&v(&["---", "## Notes", "---"]), "## Notes"),
                None
            );
            assert_eq!(
                find_heading(&v(&["---", "## Notes", "---", "## Notes"]), "## Notes"),
                Some(3)
            );
        }

        #[test]
        fn the_search_stays_inside_the_given_range() {
            let lines = v(&["## A", "## A"]);
            assert_eq!(find_heading_in(&lines, "## A", 1, 2), Some(1));
            assert_eq!(find_heading_in(&lines, "## A", 0, 0), None);
            assert_eq!(find_heading_in(&lines, "## A", 2, 2), None);
        }

        #[test]
        fn a_section_ends_at_the_next_heading_of_the_same_or_a_higher_level() {
            let lines = v(&["## A", "x", "### B", "y", "## C"]);
            assert_eq!(section_end(&lines, 0), 4);
            assert_eq!(section_end(&lines, 2), 4);
            assert_eq!(section_end(&lines, 4), 5);
        }

        #[test]
        fn a_section_does_not_end_at_a_heading_inside_a_code_block() {
            let lines = v(&["## A", "```", "## X", "```", "t"]);
            assert_eq!(section_end(&lines, 0), 5);
        }

        #[test]
        fn the_body_ends_after_the_last_non_empty_line() {
            assert_eq!(body_end(&v(&["## A", "x", "", "", "## B"]), 0), 2);
            assert_eq!(body_end(&v(&["## A", "", ""]), 0), 1);
        }

        #[test]
        fn inserted_lines_are_followed_by_a_blank_line_when_text_follows() {
            let mut lines = v(&["a", "b"]);
            insert_lines(&mut lines, 1, v(&["x"]));
            assert_eq!(lines, v(&["a", "x", "", "b"]));

            let mut lines = v(&["a", "", "b"]);
            insert_lines(&mut lines, 1, v(&["x"]));
            assert_eq!(lines, v(&["a", "x", "", "b"]));

            let mut lines = v(&["a"]);
            insert_lines(&mut lines, 1, v(&["x"]));
            assert_eq!(lines, v(&["a", "x"]));
        }
    }

    mod finding_the_heading {
        use super::*;

        #[test]
        fn appends_at_the_end_of_an_existing_section() {
            assert_eq!(
                append(&["## Log Items", "- old entry"], "## Log Items", &[]),
                v(&["## Log Items", "- old entry", "- new"])
            );
        }

        #[test]
        fn matches_case_insensitively() {
            assert_eq!(
                append(&["## log items", "- old entry"], "## Log Items", &[]),
                v(&["## log items", "- old entry", "- new"])
            );
        }

        #[test]
        fn fills_an_empty_section_below_its_heading() {
            assert_eq!(
                append(
                    &["## Log Items", "", "## Next Section"],
                    "## Log Items",
                    &[]
                ),
                v(&["## Log Items", "", "- new", "", "## Next Section"])
            );
        }

        #[test]
        fn goes_after_the_last_content_not_after_trailing_blank_lines() {
            assert_eq!(
                append(
                    &["## Log Items", "- entry", "", "", "## Next"],
                    "## Log Items",
                    &[]
                ),
                v(&["## Log Items", "- entry", "- new", "", "", "## Next"])
            );
        }

        #[test]
        fn picks_the_right_section_among_several() {
            assert_eq!(
                append(
                    &["## First", "text", "## Log Items", "- entry", "## Third"],
                    "## Log Items",
                    &[]
                ),
                v(&[
                    "## First",
                    "text",
                    "## Log Items",
                    "- entry",
                    "- new",
                    "",
                    "## Third"
                ])
            );
        }

        #[test]
        fn keeps_subsections_inside_the_section() {
            assert_eq!(
                append(
                    &["# Top", "## Log Items", "- entry", "### Sub", "sub text"],
                    "## Log Items",
                    &[]
                ),
                v(&[
                    "# Top",
                    "## Log Items",
                    "- entry",
                    "### Sub",
                    "sub text",
                    "- new"
                ])
            );
        }
    }

    mod creating_a_missing_heading {
        use super::*;

        #[test]
        fn goes_before_the_next_section_of_the_order_that_the_note_has() {
            assert_eq!(
                add(&["# Friday", "", "## Notes"], "## Journal"),
                v(&["# Friday", "", "## Journal", "", "- new", "", "## Notes"])
            );
        }

        #[test]
        fn lines_sections_up_in_the_order_whichever_is_written_first() {
            let first = add(&["# Friday", "", "## Notes", "### Topic"], "## Activity");
            let first: Vec<&str> = first.iter().map(String::as_str).collect();
            let second = add(&first, "## Journal");
            let second: Vec<&str> = second.iter().map(String::as_str).collect();
            let third = add(&second, "## Plan for Today");
            let headings: Vec<&str> = third
                .iter()
                .map(String::as_str)
                .filter(|line| line.starts_with('#'))
                .collect();
            assert_eq!(
                headings,
                [
                    "# Friday",
                    "## Journal",
                    "## Plan for Today",
                    "## Activity",
                    "## Notes",
                    "### Topic"
                ]
            );
        }

        #[test]
        fn goes_after_the_previous_section_of_the_order_when_no_later_one_exists() {
            assert_eq!(
                add(&["## Journal", "j", "## Other", "o"], "## Activity"),
                v(&[
                    "## Journal",
                    "j",
                    "",
                    "## Activity",
                    "",
                    "- new",
                    "",
                    "## Other",
                    "o"
                ])
            );
        }

        #[test]
        fn goes_below_the_frontmatter_and_the_title_when_the_note_has_none_of_the_sections() {
            assert_eq!(
                add(&["---", "status: off", "---", "# Friday"], "## Journal"),
                v(&[
                    "---",
                    "status: off",
                    "---",
                    "# Friday",
                    "",
                    "## Journal",
                    "",
                    "- new"
                ])
            );
        }

        #[test]
        fn without_a_title_it_goes_to_the_very_start() {
            assert_eq!(
                add(&["Content"], "## Journal"),
                v(&["## Journal", "", "- new", "", "Content"])
            );
        }

        #[test]
        fn without_a_title_it_goes_after_the_frontmatter() {
            assert_eq!(
                add(&["---", "title: Test", "---", ""], "## Journal")[..5],
                v(&["---", "title: Test", "---", "", "## Journal"])[..]
            );
        }

        #[test]
        fn a_heading_that_is_not_in_the_order_goes_to_the_end() {
            assert_eq!(
                append(&["Some content", "More content"], "## Log Items", &[]),
                v(&[
                    "Some content",
                    "More content",
                    "",
                    "## Log Items",
                    "",
                    "- new"
                ])
            );
        }

        #[test]
        fn a_tag_line_does_not_end_a_section() {
            let order = v(&["## A", "## Log Items"]);
            assert_eq!(
                append(
                    &["## A", "text", "#ai #claude", "more", "## B"],
                    "## Log Items",
                    &order
                )[..6],
                v(&["## A", "text", "#ai #claude", "more", "", "## Log Items"])[..]
            );
        }

        #[test]
        fn headings_in_code_blocks_do_not_count_as_sections() {
            let result = add(&["## Journal", "```", "## Notes", "```"], "## Activity");
            assert_eq!(
                result[result.len() - 4..],
                v(&["", "## Activity", "", "- new"])[..]
            );
        }

        #[test]
        fn an_empty_note_gets_the_heading() {
            assert_eq!(add(&[], "## Journal"), v(&["## Journal", "", "- new", ""]));
        }

        #[test]
        fn place_heading_returns_the_index_of_the_new_heading() {
            let mut note = Note::parse("# Friday");
            let index = place_heading(&mut note, "## Journal", &[], Vec::new());
            assert_eq!(index, 2);
            assert_eq!(note.lines, v(&["# Friday", "", "## Journal"]));
        }
    }

    mod replacing_a_section {
        use super::*;

        #[test]
        fn replaces_the_body_and_keeps_the_next_section_separated() {
            assert_eq!(
                replace(
                    &["## Notes", "- old", "- older", "", "## Next", "x"],
                    "## Notes",
                    &["- fresh"]
                ),
                v(&["## Notes", "", "- fresh", "", "## Next", "x"])
            );
        }

        #[test]
        fn adds_a_blank_line_before_a_section_that_followed_directly() {
            assert_eq!(
                replace(&["## Notes", "- old", "## Next"], "## Notes", &["- fresh"]),
                v(&["## Notes", "", "- fresh", "", "## Next"])
            );
        }

        #[test]
        fn an_empty_body_clears_the_section() {
            assert_eq!(
                replace(&["## Notes", "", "- old", "", "## Next"], "## Notes", &[]),
                v(&["## Notes", "", "## Next"])
            );
        }

        #[test]
        fn an_empty_body_on_a_missing_section_creates_nothing() {
            let before = Note::parse("# Day\n\n## Journal\n- j\n");
            let mut note = before.clone();
            replace_section(&mut note, "## Notes", &order(), Vec::new());
            assert_eq!(note, before);
        }

        #[test]
        fn a_missing_section_with_a_body_is_placed_at_its_spot_in_the_order() {
            assert_eq!(
                replace(&["# Day", "", "## Notes", "- n"], "## Journal", &["- j"]),
                v(&["# Day", "", "## Journal", "", "- j", "", "## Notes", "- n"])
            );
        }
    }
}
