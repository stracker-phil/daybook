//! The palette's results for what was typed: a prefix narrows to one kind, the rest is ranked. Pure.

use serde::Serialize;
use ts_rs::TS;

use super::entries::{PaletteEntry, PaletteKind};
use super::rank::rank_match;

/// Typing one of these first limits the results to one kind.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct PalettePrefix {
    pub char: String,
    pub kind: PaletteKind,
    pub label: String,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct PaletteGroup {
    pub kind: PaletteKind,
    pub heading: String,
    pub entries: Vec<PaletteEntry>,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct PaletteResults {
    /// The prefix typed, if any.
    pub prefix: Option<PalettePrefix>,
    pub prefixes: Vec<PalettePrefix>,
    /// Only groups with entries, in a fixed order.
    pub groups: Vec<PaletteGroup>,
}

/// Each kind's prefix and heading, in the order the groups show.
const KINDS: [(char, PaletteKind, &str); 5] = [
    ('>', PaletteKind::Command, "Commands"),
    ('#', PaletteKind::Link, "Links"),
    ('/', PaletteKind::Project, "Projects"),
    (':', PaletteKind::Music, "Music"),
    ('@', PaletteKind::Context, "Contexts"),
];

/// Per group with nothing typed.
const PREVIEW_LIMIT: usize = 6;
/// Per group while searching.
const SEARCH_LIMIT: usize = 8;
/// With a prefix, which narrows to one group.
const FULL_LIMIT: usize = 200;

fn prefix_of((c, kind, heading): (char, PaletteKind, &str)) -> PalettePrefix {
    PalettePrefix {
        char: c.to_string(),
        kind,
        label: heading.to_string(),
    }
}

/// The best `limit` entries of `kind` for `query`; `None` when none match.
fn group(
    entries: &[PaletteEntry],
    (kind, heading): (PaletteKind, &str),
    query: &str,
    limit: usize,
) -> Option<PaletteGroup> {
    let mut ranked: Vec<(u8, &PaletteEntry)> = entries
        .iter()
        .filter(|entry| entry.kind == kind)
        .filter_map(|entry| {
            rank_match(&entry.label, &entry.detail, query).map(|rank| (rank, entry))
        })
        .collect();
    ranked.sort_by_key(|(rank, _)| *rank);
    let top: Vec<PaletteEntry> = ranked
        .into_iter()
        .take(limit)
        .map(|(_, entry)| entry.clone())
        .collect();
    (!top.is_empty()).then(|| PaletteGroup {
        kind,
        heading: heading.to_string(),
        entries: top,
    })
}

pub fn search(entries: &[PaletteEntry], typed: &str) -> PaletteResults {
    let (prefix, rest) = KINDS
        .iter()
        .find_map(|kind| typed.strip_prefix(kind.0).map(|rest| (Some(*kind), rest)))
        .unwrap_or((None, typed));
    let query = rest.trim().to_lowercase();
    let limit = if prefix.is_some() {
        FULL_LIMIT
    } else if query.is_empty() {
        PREVIEW_LIMIT
    } else {
        SEARCH_LIMIT
    };
    let groups = KINDS
        .iter()
        .filter(|(_, kind, _)| prefix.is_none_or(|p| p.1 == *kind))
        .filter_map(|(_, kind, heading)| group(entries, (*kind, heading), &query, limit))
        .collect();
    PaletteResults {
        prefix: prefix.map(prefix_of),
        prefixes: KINDS.into_iter().map(prefix_of).collect(),
        groups,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(kind: PaletteKind, label: &str, detail: &str) -> PaletteEntry {
        PaletteEntry {
            kind,
            label: label.into(),
            detail: detail.into(),
            shortcut: None,
            command: format!("{kind:?}:{label}").to_lowercase(),
        }
    }

    fn numbered(kind: PaletteKind, count: usize) -> Vec<PaletteEntry> {
        (0..count)
            .map(|i| entry(kind, &format!("item {i:03}"), ""))
            .collect()
    }

    fn headings(results: &PaletteResults) -> Vec<&str> {
        results.groups.iter().map(|g| g.heading.as_str()).collect()
    }

    fn labels(group: &PaletteGroup) -> Vec<&str> {
        group.entries.iter().map(|e| e.label.as_str()).collect()
    }

    fn one_of_each() -> Vec<PaletteEntry> {
        vec![
            entry(PaletteKind::Context, "Private", ""),
            entry(PaletteKind::Music, "Cell", ""),
            entry(PaletteKind::Project, "QRS", ""),
            entry(PaletteKind::Link, "Board", ""),
            entry(PaletteKind::Command, "Journal…", ""),
        ]
    }

    #[test]
    fn the_prefixes_are_always_offered() {
        let results = search(&[], "");
        let offered: Vec<(&str, PaletteKind, &str)> = results
            .prefixes
            .iter()
            .map(|p| (p.char.as_str(), p.kind, p.label.as_str()))
            .collect();
        assert_eq!(
            offered,
            [
                (">", PaletteKind::Command, "Commands"),
                ("#", PaletteKind::Link, "Links"),
                ("/", PaletteKind::Project, "Projects"),
                (":", PaletteKind::Music, "Music"),
                ("@", PaletteKind::Context, "Contexts"),
            ]
        );
        assert_eq!(search(&one_of_each(), ">x").prefixes, results.prefixes);
    }

    #[test]
    fn nothing_typed_lists_every_non_empty_group_in_a_fixed_order() {
        let results = search(&one_of_each(), "");
        assert_eq!(results.prefix, None);
        assert_eq!(
            headings(&results),
            ["Commands", "Links", "Projects", "Music", "Contexts"]
        );
        assert_eq!(
            search(&one_of_each()[..2], "")
                .groups
                .iter()
                .map(|g| g.kind)
                .collect::<Vec<_>>(),
            [PaletteKind::Music, PaletteKind::Context]
        );
    }

    #[test]
    fn groups_without_entries_are_left_out() {
        let results = search(&[entry(PaletteKind::Link, "Board", "")], "");
        assert_eq!(headings(&results), ["Links"]);
        assert_eq!(search(&[], "").groups, vec![]);
    }

    #[test]
    fn a_group_with_no_match_is_left_out() {
        let results = search(&one_of_each(), "board");
        assert_eq!(headings(&results), ["Links"]);
        assert_eq!(labels(&results.groups[0]), ["Board"]);
        assert_eq!(search(&one_of_each(), "zzz").groups, vec![]);
    }

    #[test]
    fn nothing_typed_shows_a_preview_of_six_per_group() {
        let mut entries = numbered(PaletteKind::Command, 10);
        entries.extend(numbered(PaletteKind::Link, 3));
        for typed in ["", "   "] {
            let results = search(&entries, typed);
            assert_eq!(results.groups[0].entries.len(), 6, "{typed:?}");
            assert_eq!(results.groups[1].entries.len(), 3, "{typed:?}");
            assert_eq!(results.prefix, None);
        }
    }

    #[test]
    fn typing_a_search_shows_up_to_eight_per_group() {
        let mut entries = numbered(PaletteKind::Command, 10);
        entries.extend(numbered(PaletteKind::Music, 9));
        let results = search(&entries, "item");
        assert_eq!(results.groups[0].entries.len(), 8);
        assert_eq!(results.groups[1].entries.len(), 8);
    }

    #[test]
    fn a_prefix_shows_up_to_two_hundred_of_its_kind() {
        let entries = numbered(PaletteKind::Command, 250);
        assert_eq!(search(&entries, ">").groups[0].entries.len(), 200);
        assert_eq!(search(&entries, ">item").groups[0].entries.len(), 200);
        assert_eq!(search(&entries[..50], ">").groups[0].entries.len(), 50);
    }

    #[test]
    fn each_prefix_narrows_the_results_to_its_kind() {
        let entries = one_of_each();
        for (typed, kind, heading, label) in [
            (">", PaletteKind::Command, "Commands", "Journal…"),
            ("#", PaletteKind::Link, "Links", "Board"),
            ("/", PaletteKind::Project, "Projects", "QRS"),
            (":", PaletteKind::Music, "Music", "Cell"),
            ("@", PaletteKind::Context, "Contexts", "Private"),
        ] {
            let results = search(&entries, typed);
            assert_eq!(
                results.prefix,
                Some(PalettePrefix {
                    char: typed.into(),
                    kind,
                    label: heading.into(),
                }),
                "{typed}"
            );
            assert_eq!(headings(&results), [heading], "{typed}");
            assert_eq!(labels(&results.groups[0]), [label], "{typed}");
        }
    }

    #[test]
    fn what_follows_the_prefix_is_the_query() {
        let entries = vec![
            entry(PaletteKind::Link, "Board", ""),
            entry(PaletteKind::Link, "Wiki", "Oscorp"),
            entry(PaletteKind::Command, "Wiki Search", ""),
        ];
        for typed in ["#wiki", "# wiki", "#  WIKI  "] {
            let results = search(&entries, typed);
            assert_eq!(
                results.prefix.clone().map(|p| (p.kind, p.char, p.label)),
                Some((PaletteKind::Link, "#".to_string(), "Links".to_string())),
                "{typed}"
            );
            assert_eq!(headings(&results), ["Links"], "{typed}");
            assert_eq!(labels(&results.groups[0]), ["Wiki"], "{typed}");
        }
    }

    #[test]
    fn a_prefix_only_counts_as_the_first_character() {
        let entries = vec![
            entry(PaletteKind::Command, "a>b", ""),
            entry(PaletteKind::Link, "a>b link", ""),
        ];
        let results = search(&entries, "a>b");
        assert_eq!(results.prefix, None);
        assert_eq!(headings(&results), ["Commands", "Links"]);
        let spaced = search(&entries, " >b");
        assert_eq!(spaced.prefix, None);
    }

    #[test]
    fn a_prefix_with_nothing_of_its_kind_shows_no_group() {
        let results = search(&[entry(PaletteKind::Link, "Board", "")], ">");
        assert_eq!(results.prefix.map(|p| p.kind), Some(PaletteKind::Command));
        assert_eq!(results.groups, vec![]);
    }

    #[test]
    fn the_query_is_matched_case_insensitively() {
        let entries = vec![entry(PaletteKind::Command, "Quick Notes", "")];
        for typed in ["quick", "QUICK", "  Quick  "] {
            assert_eq!(search(&entries, typed).groups.len(), 1, "{typed:?}");
        }
    }

    #[test]
    fn better_matches_come_first_and_ties_keep_their_order() {
        let entries = vec![
            entry(PaletteKind::Command, "Nice Old Tidy Embedded Sky", ""),
            entry(PaletteKind::Command, "Jot", "notes"),
            entry(PaletteKind::Command, "Open Notes", ""),
            entry(PaletteKind::Command, "Notes B", ""),
            entry(PaletteKind::Command, "Notes A", ""),
            entry(PaletteKind::Command, "Other", ""),
            entry(PaletteKind::Command, "Quick Notes", ""),
        ];
        let results = search(&entries, "notes");
        assert_eq!(
            labels(&results.groups[0]),
            [
                "Notes B",
                "Notes A",
                "Open Notes",
                "Quick Notes",
                "Jot",
                "Nice Old Tidy Embedded Sky"
            ]
        );
    }

    #[test]
    fn the_detail_is_searched_too() {
        let entries = vec![
            entry(PaletteKind::Link, "Wiki", "Oscorp"),
            entry(PaletteKind::Link, "Board", ""),
        ];
        let results = search(&entries, "oscorp");
        assert_eq!(labels(&results.groups[0]), ["Wiki"]);
    }

    #[test]
    fn with_nothing_typed_the_entries_keep_their_order() {
        let entries = vec![
            entry(PaletteKind::Command, "Zeta", ""),
            entry(PaletteKind::Command, "Alpha", ""),
        ];
        assert_eq!(labels(&search(&entries, "").groups[0]), ["Zeta", "Alpha"]);
    }

    #[test]
    fn the_limit_keeps_the_best_matches() {
        let mut entries: Vec<PaletteEntry> = (0..9)
            .map(|i| entry(PaletteKind::Command, &format!("see item {i}"), ""))
            .collect();
        entries.push(entry(PaletteKind::Command, "item best", ""));
        let results = search(&entries, "item");
        assert_eq!(
            labels(&results.groups[0]),
            [
                "item best",
                "see item 0",
                "see item 1",
                "see item 2",
                "see item 3",
                "see item 4",
                "see item 5",
                "see item 6",
            ]
        );
    }
}
