//! Which notes match the terms, in which order, with which line as the snippet and which parts highlighted.

use std::cmp::Ordering;

use regex::{Regex, RegexBuilder};
use serde::Serialize;
use ts_rs::TS;

use super::note::{IndexedNote, is_heading};

const MAX_RESULTS: usize = 50;
const LINE_LENGTH: usize = 160;
const LINE_LEAD: usize = 40;
const DAY_MS: f64 = 86_400_000.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    pub path: String,
    pub name: String,
    pub folder: String,
    /// The snippet; `""` when the note has no text line.
    pub line: String,
}

/// A piece of a shown text; `hit` pieces are highlighted.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct TextPart {
    pub text: String,
    pub hit: bool,
}

/// How well a note matches: in its name, its aliases, its headings, else only in its body.
fn tier(note: &IndexedNote, terms: &[String]) -> Option<u8> {
    let aliases = note.aliases_lower.join("\n");
    let level = |term: &String| {
        if note.name_lower.contains(term.as_str()) {
            Some(0)
        } else if aliases.contains(term.as_str()) {
            Some(1)
        } else if note.headings_lower.contains(term.as_str()) {
            Some(2)
        } else if note.body_lower.contains(term.as_str()) {
            Some(3)
        } else {
            None
        }
    };
    terms
        .iter()
        .map(level)
        .collect::<Option<Vec<u8>>>()?
        .into_iter()
        .max()
}

/// 1 for a note from today (or the future), falling to 0 over a year.
fn recency(date: i64, now_ms: i64) -> f64 {
    let age_days = ((now_ms - date) as f64 / DAY_MS).max(0.0);
    (1.0 - age_days / 365.0).max(0.0)
}

/// Close to `localeCompare`: case-insensitive first.
fn compare_names(a: &str, b: &str) -> Ordering {
    a.to_lowercase()
        .cmp(&b.to_lowercase())
        .then_with(|| a.cmp(b))
}

fn in_folders(path: &str, folders: &[String]) -> bool {
    folders
        .iter()
        .any(|folder| folder.is_empty() || path.starts_with(folder.as_str()))
}

/// The notes under one of `folders` (`""`: anywhere) and under none of `exclude` that hold every term,
/// best first: by tier, then recency, then name; at most `MAX_RESULTS`.
pub fn search<'a>(
    notes: impl Iterator<Item = &'a IndexedNote>,
    terms: &[String],
    folders: &[String],
    exclude: &[String],
    now_ms: i64,
) -> Vec<Hit> {
    if terms.is_empty() {
        return Vec::new();
    }
    let mut ranked: Vec<(u8, f64, &IndexedNote)> = notes
        .filter(|note| in_folders(&note.path, folders))
        .filter(|note| {
            !exclude
                .iter()
                .any(|prefix| note.path.starts_with(prefix.as_str()))
        })
        .filter_map(|note| Some((tier(note, terms)?, recency(note.date, now_ms), note)))
        .collect();
    ranked.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then_with(|| b.1.total_cmp(&a.1))
            .then_with(|| compare_names(&a.2.name, &b.2.name))
    });
    ranked
        .into_iter()
        .take(MAX_RESULTS)
        .map(|(_, _, note)| Hit {
            path: note.path.clone(),
            name: note.name.clone(),
            folder: note.folder.clone(),
            line: best_line(&note.lines, terms),
        })
        .collect()
}

/// The line holding the most terms (the first of equals), else the first text line, trimmed and cut to
/// `LINE_LENGTH` characters around the first term.
fn best_line(lines: &[String], terms: &[String]) -> String {
    pick_line(lines, terms).map_or_else(String::new, |line| snippet(line.trim(), terms))
}

fn pick_line<'a>(lines: &'a [String], terms: &[String]) -> Option<&'a String> {
    let mut best: Option<&String> = None;
    let mut best_count = 0;
    for line in lines {
        let lower = line.to_lowercase();
        let count = terms
            .iter()
            .filter(|term| lower.contains(term.as_str()))
            .count();
        if count > best_count {
            best = Some(line);
            best_count = count;
            if count == terms.len() {
                break;
            }
        }
    }
    best.or_else(|| {
        lines
            .iter()
            .find(|line| !line.trim().is_empty() && !is_heading(line))
    })
}

fn snippet(line: &str, terms: &[String]) -> String {
    let length = line.chars().count();
    if length <= LINE_LENGTH {
        return line.to_string();
    }
    let lower = line.to_lowercase();
    let first = terms
        .iter()
        .find_map(|term| lower.find(term.as_str()))
        .map_or(0, |at| lower[..at].chars().count().min(length));
    let start = first.saturating_sub(LINE_LEAD);
    let cut: String = line.chars().skip(start).take(LINE_LENGTH).collect();
    let lead = if start > 0 { "…" } else { "" };
    format!("{lead}{cut}…")
}

/// Highlights the terms in texts.
pub struct Highlighter(Option<Regex>);

impl Highlighter {
    pub fn new(terms: &[String]) -> Self {
        let pattern = terms
            .iter()
            .filter(|term| !term.is_empty())
            .map(|term| regex::escape(term))
            .collect::<Vec<_>>()
            .join("|");
        if pattern.is_empty() {
            return Self(None);
        }
        Self(
            RegexBuilder::new(&pattern)
                .case_insensitive(true)
                .build()
                .ok(),
        )
    }

    /// `text` split at the terms: case-insensitive, earlier terms first, leftmost match wins.
    pub fn parts(&self, text: &str) -> Vec<TextPart> {
        let mut parts = Vec::new();
        let mut push = |text: &str, hit: bool| {
            if !text.is_empty() {
                parts.push(TextPart {
                    text: text.to_string(),
                    hit,
                });
            }
        };
        let mut end = 0;
        if let Some(regex) = &self.0 {
            for found in regex.find_iter(text) {
                push(&text[end..found.start()], false);
                push(found.as_str(), true);
                end = found.end();
            }
        }
        push(&text[end..], false);
        parts
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::note::parse;

    const NOW: i64 = 1_800_000_000_000;
    const DAY: i64 = 86_400_000;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    fn note(path: &str, text: &str) -> IndexedNote {
        parse(path, text, NOW)
    }

    fn note_aged(path: &str, text: &str, days: i64) -> IndexedNote {
        parse(path, text, NOW - days * DAY)
    }

    fn found(notes: &[IndexedNote], terms: &[&str]) -> Vec<String> {
        search(notes.iter(), &strings(terms), &strings(&[""]), &[], NOW)
            .into_iter()
            .map(|hit| hit.path)
            .collect()
    }

    fn parts(text: &str, terms: &[String]) -> Vec<TextPart> {
        Highlighter::new(terms).parts(text)
    }

    fn part(text: &str, hit: bool) -> TextPart {
        TextPart {
            text: text.into(),
            hit,
        }
    }

    #[test]
    fn notes_rank_by_name_then_alias_then_heading_then_body() {
        let notes = [
            note("Fourth.md", "some alpha text"),
            note("Third.md", "# Alpha"),
            note("Other.md", "---\naliases: [Alpha]\n---\ntext"),
            note("Alpha.md", "text"),
        ];
        assert_eq!(
            found(&notes, &["alpha"]),
            strings(&["Alpha.md", "Other.md", "Third.md", "Fourth.md"])
        );
    }

    #[test]
    fn a_note_must_hold_every_term() {
        let notes = [note("Both.md", "alpha beta"), note("One.md", "alpha")];
        assert_eq!(found(&notes, &["alpha", "beta"]), strings(&["Both.md"]));
    }

    #[test]
    fn a_note_ranks_by_its_weakest_term() {
        let notes = [note("Alpha.md", "beta"), note("Other.md", "# alpha beta")];
        assert_eq!(
            found(&notes, &["alpha", "beta"]),
            strings(&["Other.md", "Alpha.md"])
        );
    }

    #[test]
    fn within_a_tier_newer_notes_come_first_and_then_names() {
        let notes = [
            note_aged("Abe.md", "alpha", 100),
            note("Zed.md", "alpha"),
            note("b.md", "alpha"),
            note("A.md", "alpha"),
        ];
        assert_eq!(
            found(&notes, &["alpha"]),
            strings(&["A.md", "b.md", "Zed.md", "Abe.md"])
        );
    }

    #[test]
    fn notes_older_than_a_year_all_rank_alike() {
        let notes = [
            note_aged("B.md", "alpha", 400),
            note_aged("A.md", "alpha", 900),
        ];
        assert_eq!(found(&notes, &["alpha"]), strings(&["A.md", "B.md"]));
    }

    #[test]
    fn only_notes_under_the_folders_and_outside_the_excluded_ones_are_found() {
        let notes = [
            note("1-Journal/Day.md", "alpha"),
            note("2-Work/Plan.md", "alpha"),
            note("_/Template.md", "alpha"),
        ];
        let paths = |folders: &[&str], exclude: &[&str]| -> Vec<String> {
            search(
                notes.iter(),
                &strings(&["alpha"]),
                &strings(folders),
                &strings(exclude),
                NOW,
            )
            .into_iter()
            .map(|hit| hit.path)
            .collect()
        };
        assert_eq!(paths(&["2-Work/"], &[]), strings(&["2-Work/Plan.md"]));
        assert_eq!(
            paths(&["1-Journal/", "2-Work/"], &[]),
            strings(&["1-Journal/Day.md", "2-Work/Plan.md"])
        );
        assert_eq!(
            paths(&[""], &["_/"]),
            strings(&["1-Journal/Day.md", "2-Work/Plan.md"])
        );
        assert_eq!(paths(&["_/"], &["_/"]), Vec::<String>::new());
    }

    #[test]
    fn no_terms_find_nothing() {
        let notes = [note("Alpha.md", "alpha")];
        assert_eq!(found(&notes, &[]), Vec::<String>::new());
    }

    #[test]
    fn at_most_the_maximum_number_of_results_is_returned() {
        let notes: Vec<IndexedNote> = (0..MAX_RESULTS + 10)
            .map(|i| note(&format!("n{i:03}.md"), "alpha"))
            .collect();
        assert_eq!(found(&notes, &["alpha"]).len(), MAX_RESULTS);
    }

    #[test]
    fn a_hit_carries_the_notes_place_and_snippet() {
        let notes = [note("2-Work/Plan.md", "# Plan\nfirst\nthe alpha line")];
        let hits = search(
            notes.iter(),
            &strings(&["alpha"]),
            &strings(&[""]),
            &[],
            NOW,
        );
        assert_eq!(
            hits,
            vec![Hit {
                path: "2-Work/Plan.md".into(),
                name: "Plan".into(),
                folder: "2-Work".into(),
                line: "the alpha line".into(),
            }]
        );
    }

    #[test]
    fn the_snippet_is_the_line_with_the_most_terms() {
        let lines = strings(&["alpha", "alpha beta", "beta"]);
        assert_eq!(
            best_line(&lines, &strings(&["alpha", "beta", "gamma"])),
            "alpha beta"
        );
    }

    #[test]
    fn the_first_of_equally_good_lines_is_the_snippet() {
        let lines = strings(&["a x", "a y", "b"]);
        assert_eq!(best_line(&lines, &strings(&["a", "b", "c"])), "a x");
    }

    #[test]
    fn the_snippet_is_trimmed_and_matched_without_regard_to_case() {
        let lines = strings(&["nothing", "   The ALPHA   "]);
        assert_eq!(best_line(&lines, &strings(&["alpha"])), "The ALPHA");
    }

    #[test]
    fn without_a_matching_line_the_snippet_is_the_first_text_line() {
        let lines = strings(&["", "# Heading", "   first text  ", "other"]);
        assert_eq!(best_line(&lines, &strings(&["zzz"])), "first text");
    }

    #[test]
    fn without_any_text_line_the_snippet_is_empty() {
        assert_eq!(
            best_line(&strings(&["# Heading", " "]), &strings(&["zzz"])),
            ""
        );
        assert_eq!(best_line(&[], &strings(&["zzz"])), "");
    }

    #[test]
    fn a_long_snippet_starts_forty_characters_before_the_first_term() {
        let line = format!("{}needle{}", "x".repeat(100), "y".repeat(200));
        let expected = format!("…{}needle{}…", "x".repeat(40), "y".repeat(114));
        assert_eq!(best_line(&[line], &strings(&["needle"])), expected);
    }

    #[test]
    fn a_long_snippet_with_the_term_near_its_start_gets_no_leading_ellipsis() {
        let line = format!("needle{}", "y".repeat(300));
        let expected = format!("needle{}…", "y".repeat(154));
        assert_eq!(best_line(&[line], &strings(&["needle"])), expected);
    }

    #[test]
    fn a_long_fallback_snippet_is_cut_from_its_start() {
        let line = "z".repeat(200);
        let expected = format!("{}…", "z".repeat(160));
        assert_eq!(best_line(&[line], &strings(&["q"])), expected);
    }

    #[test]
    fn a_snippet_of_exactly_the_maximum_length_is_not_cut() {
        let line = "é".repeat(160);
        assert_eq!(best_line(&[line.clone()], &strings(&["q"])), line);
    }

    #[test]
    fn cutting_a_snippet_never_splits_a_multibyte_character() {
        let line = format!("{}needle{}", "é".repeat(100), "ü".repeat(200));
        let expected = format!("…{}needle{}…", "é".repeat(40), "ü".repeat(114));
        assert_eq!(best_line(&[line], &strings(&["needle"])), expected);
    }

    #[test]
    fn text_is_split_at_the_terms_regardless_of_case() {
        assert_eq!(
            parts("Hello World", &strings(&["world"])),
            vec![part("Hello ", false), part("World", true)]
        );
        assert_eq!(
            parts("Foo foo", &strings(&["foo"])),
            vec![part("Foo", true), part(" ", false), part("foo", true)]
        );
    }

    #[test]
    fn at_the_same_position_the_earlier_term_wins() {
        assert_eq!(
            parts("foobar", &strings(&["foo", "foobar"])),
            vec![part("foo", true), part("bar", false)]
        );
        assert_eq!(
            parts("foobar", &strings(&["foobar", "foo"])),
            vec![part("foobar", true)]
        );
    }

    #[test]
    fn matches_do_not_overlap() {
        assert_eq!(
            parts("aaa", &strings(&["aa"])),
            vec![part("aa", true), part("a", false)]
        );
    }

    #[test]
    fn terms_are_matched_literally() {
        assert_eq!(
            parts("abc a.c", &strings(&["a.c"])),
            vec![part("abc ", false), part("a.c", true)]
        );
    }

    #[test]
    fn parts_are_never_empty() {
        assert_eq!(parts("ab", &strings(&["ab"])), vec![part("ab", true)]);
        assert_eq!(
            parts("xaby", &strings(&["ab"])),
            vec![part("x", false), part("ab", true), part("y", false)]
        );
    }

    #[test]
    fn without_terms_the_text_is_one_plain_part() {
        assert_eq!(parts("Hello", &[]), vec![part("Hello", false)]);
        assert_eq!(parts("Hello", &strings(&[""])), vec![part("Hello", false)]);
    }

    #[test]
    fn an_empty_text_has_no_parts() {
        assert_eq!(parts("", &strings(&["a"])), Vec::<TextPart>::new());
        assert_eq!(parts("", &[]), Vec::<TextPart>::new());
    }

    #[test]
    fn a_part_serializes_with_its_text_and_hit_flag() {
        assert_eq!(
            serde_json::to_value(part("a", true)).unwrap(),
            serde_json::json!({ "text": "a", "hit": true })
        );
    }
}
