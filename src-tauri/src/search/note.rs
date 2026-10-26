//! One note as the search index keeps it: its name, aliases, headings and body, lowercased for matching.

use std::ops::Range;
use std::sync::LazyLock;

use chrono::{Local, NaiveDate, TimeZone};
use regex::Regex;

use super::posix;

static DATE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(\d{4})-(\d{2})-(\d{2})").unwrap());
static HEADING: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^#{1,6}\s").unwrap());
static TRAILING_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+#.*$").unwrap());
static LIST_ITEM: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s+-\s").unwrap());

#[derive(Clone, Debug, PartialEq)]
pub struct IndexedNote {
    /// Relative to the vault, `/`-separated.
    pub path: String,
    /// The file name without `.md`.
    pub name: String,
    /// `""` at the vault root.
    pub folder: String,
    pub name_lower: String,
    /// The frontmatter's `aliases` and `alias`.
    pub aliases_lower: Vec<String>,
    /// The heading lines with their `#`s, one per line.
    pub headings_lower: String,
    /// The lines after the frontmatter.
    pub lines: Vec<String>,
    pub body_lower: String,
    /// Local midnight of the first date in the name, else the file's modification time (ms).
    pub date: i64,
}

pub fn is_heading(line: &str) -> bool {
    HEADING.is_match(line)
}

/// The file's lines: one trailing newline dropped, split on `\n` or `\r\n`; none for an empty file.
pub fn split_lines(text: &str) -> Vec<String> {
    let text = text
        .strip_suffix("\r\n")
        .or_else(|| text.strip_suffix('\n'))
        .unwrap_or(text);
    if text.is_empty() {
        return Vec::new();
    }
    text.split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line).to_string())
        .collect()
}

/// The frontmatter's lines (between `---` on line 0 and the first `---` or `...` after it) and where the
/// body starts; without a closed frontmatter, none and 0.
fn split_frontmatter(lines: &[String]) -> (Range<usize>, usize) {
    let none = (0..0, 0);
    if lines.first().is_none_or(|line| line.trim() != "---") {
        return none;
    }
    let closing = lines
        .iter()
        .skip(1)
        .position(|line| matches!(line.trim(), "---" | "..."))
        .map(|at| at + 1);
    closing.map_or(none, |closing| (1..closing, closing + 1))
}

/// Where the body starts (see `split_frontmatter`).
pub fn body_start(lines: &[String]) -> usize {
    split_frontmatter(lines).1
}

fn unquote(value: &str) -> String {
    let value = value.trim();
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner.to_string();
        }
    }
    value.to_string()
}

/// The value of a `key: value` line (`key` unindented, spaces allowed before the colon).
fn value_after_key<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    let (head, value) = line.split_once(':')?;
    head.strip_prefix(key)?.trim().is_empty().then_some(value)
}

/// A frontmatter list: `key: [a, "b"]`, `key: a` or `key:` followed by indented `- a` lines. The first
/// line with `key` wins.
fn frontmatter_list(frontmatter: &[String], key: &str) -> Vec<String> {
    let Some((at, value)) = frontmatter
        .iter()
        .enumerate()
        .find_map(|(at, line)| Some((at, value_after_key(line, key)?)))
    else {
        return Vec::new();
    };
    let inline = TRAILING_COMMENT.replace(value, "").trim().to_string();
    let values: Vec<String> = if let Some(list) = inline.strip_prefix('[') {
        let list = list.strip_suffix(']').unwrap_or(list);
        list.split(',').map(unquote).collect()
    } else if !inline.is_empty() {
        vec![unquote(&inline)]
    } else {
        frontmatter[at + 1..]
            .iter()
            .map_while(|line| {
                LIST_ITEM
                    .find(line)
                    .map(|item| unquote(&line[item.end()..]))
            })
            .collect()
    };
    values
        .into_iter()
        .filter(|value| !value.is_empty())
        .collect()
}

/// Local midnight of the first `YYYY-MM-DD` in `name`, in ms.
fn date_in_name(name: &str) -> Option<i64> {
    let caps = DATE.captures(name)?;
    let year = caps[1].parse::<i32>().ok()?;
    let month = caps[2].parse::<u32>().ok()?;
    let day_of_month = caps[3].parse::<u32>().ok()?;
    let day = NaiveDate::from_ymd_opt(year, month, day_of_month)?;
    let midnight = Local
        .from_local_datetime(&day.and_hms_opt(0, 0, 0)?)
        .earliest()?;
    Some(midnight.timestamp_millis())
}

/// Parses the note at `path` (vault-relative) from its text; `modified_ms` dates a name without a date.
pub fn parse(path: &str, text: &str, modified_ms: i64) -> IndexedNote {
    let all = split_lines(text);
    let (frontmatter, start) = split_frontmatter(&all);
    let frontmatter = &all[frontmatter];
    let aliases_lower = ["aliases", "alias"]
        .iter()
        .flat_map(|key| frontmatter_list(frontmatter, key))
        .map(|alias| alias.to_lowercase())
        .collect();
    let lines = all[start..].to_vec();
    let headings_lower = lines
        .iter()
        .filter(|line| is_heading(line))
        .map(|line| line.to_lowercase())
        .collect::<Vec<_>>()
        .join("\n");
    let file = posix::basename(path);
    let name = file.strip_suffix(".md").unwrap_or(file).to_string();
    let folder = match posix::dirname(path) {
        "." => String::new(),
        folder => folder.to_string(),
    };
    IndexedNote {
        path: path.to_string(),
        name_lower: name.to_lowercase(),
        date: date_in_name(&name).unwrap_or(modified_ms),
        name,
        folder,
        aliases_lower,
        headings_lower,
        body_lower: lines.join("\n").to_lowercase(),
        lines,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn text_is_split_into_lines_without_the_final_newline() {
        for (text, expected) in [
            ("a\nb\n", &["a", "b"][..]),
            ("a\nb", &["a", "b"][..]),
            ("a\r\nb\r\n", &["a", "b"][..]),
            ("a\n\n", &["a", ""][..]),
            ("", &[][..]),
            ("\n", &[][..]),
        ] {
            assert_eq!(split_lines(text), lines(expected), "{text:?}");
        }
    }

    #[test]
    fn the_body_starts_after_the_closing_line_of_the_frontmatter() {
        for (input, expected) in [
            (&["---", "a: 1", "---", "text"][..], 3),
            (&["---", "a: 1", "...", "text"][..], 3),
            (&["  ---  ", "---", "text"][..], 2),
            (&["---", "a: 1", "text"][..], 0),
            (&["text", "---", "a", "---"][..], 0),
            (&[][..], 0),
        ] {
            assert_eq!(body_start(&lines(input)), expected, "{input:?}");
        }
    }

    #[test]
    fn a_frontmatter_list_is_read_in_every_notation() {
        for (input, expected) in [
            (&["aliases: [a, \"b\", 'c']"][..], &["a", "b", "c"][..]),
            (&["aliases: [a, , b]"][..], &["a", "b"][..]),
            (&["aliases: Solo"][..], &["Solo"][..]),
            (&["aliases: \"Quoted Name\""][..], &["Quoted Name"][..]),
            (&["aliases: [a, b] # note"][..], &["a", "b"][..]),
            (
                &["aliases:", "  - x", "  - 'y'", "other: 1"][..],
                &["x", "y"][..],
            ),
            (&["aliases: # none", "  - x"][..], &["x"][..]),
            (&["aliases:", "- x"][..], &[][..]),
            (&["aliases:"][..], &[][..]),
            (&["  aliases: a"][..], &[][..]),
            (&["title: a"][..], &[][..]),
        ] {
            assert_eq!(
                frontmatter_list(&lines(input), "aliases"),
                lines(expected),
                "{input:?}"
            );
        }
    }

    #[test]
    fn the_key_must_match_whole() {
        let frontmatter = lines(&["aliases: [a]", "alias: b"]);
        assert_eq!(frontmatter_list(&frontmatter, "alias"), lines(&["b"]));
        assert_eq!(frontmatter_list(&frontmatter, "aliases"), lines(&["a"]));
    }

    #[test]
    fn the_first_line_of_a_key_wins() {
        let frontmatter = lines(&["alias: first", "alias: second"]);
        assert_eq!(frontmatter_list(&frontmatter, "alias"), lines(&["first"]));
    }

    #[test]
    fn a_date_in_the_name_is_local_midnight() {
        let expected = Local
            .with_ymd_and_hms(2026, 10, 12, 0, 0, 0)
            .earliest()
            .unwrap()
            .timestamp_millis();
        assert_eq!(date_in_name("2026-10-12"), Some(expected));
        assert_eq!(date_in_name("Retro 2026-10-12 notes"), Some(expected));
    }

    #[test]
    fn a_name_without_a_valid_date_has_none() {
        for name in ["Plan", "2026-13-45", "2026-02-30", "26-10-12"] {
            assert_eq!(date_in_name(name), None, "{name}");
        }
    }

    #[test]
    fn a_note_is_parsed_into_its_searchable_parts() {
        let text =
            "---\naliases: [Alpha, \"Beta\"]\nalias: Gamma\n---\n# Title\nBody Text\n## Sub\n";
        let note = parse("2-Work/Plan.md", text, 5);
        assert_eq!(
            note,
            IndexedNote {
                path: "2-Work/Plan.md".into(),
                name: "Plan".into(),
                folder: "2-Work".into(),
                name_lower: "plan".into(),
                aliases_lower: lines(&["alpha", "beta", "gamma"]),
                headings_lower: "# title\n## sub".into(),
                lines: lines(&["# Title", "Body Text", "## Sub"]),
                body_lower: "# title\nbody text\n## sub".into(),
                date: 5,
            }
        );
    }

    #[test]
    fn a_note_at_the_vault_root_has_an_empty_folder() {
        assert_eq!(parse("Plan.md", "text", 0).folder, "");
    }

    #[test]
    fn a_note_without_closed_frontmatter_keeps_every_line_as_body() {
        let note = parse("a.md", "---\naliases: [x]\ntext\n", 0);
        assert_eq!(note.lines, lines(&["---", "aliases: [x]", "text"]));
        assert_eq!(note.aliases_lower, Vec::<String>::new());
    }

    #[test]
    fn a_hash_without_a_space_is_not_a_heading() {
        let note = parse("a.md", "#tag\n####### seven\n# real\n", 0);
        assert_eq!(note.headings_lower, "# real");
    }

    #[test]
    fn the_date_comes_from_the_name_else_from_the_modification_time() {
        let dated = parse("1-Journal/2026-10-12.md", "", 5);
        assert_eq!(dated.date, date_in_name("2026-10-12").unwrap());
        assert_eq!(parse("Plan.md", "", 5).date, 5);
    }
}
