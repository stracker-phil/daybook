//! How well a palette entry matches what was typed. Pure.

/// The characters of `needle` appear in order in `hay` ("qn" → "Quick Notes").
fn subsequence(hay: &str, needle: &str) -> bool {
    let mut wanted = needle.chars().peekable();
    for c in hay.chars() {
        if wanted.peek() == Some(&c) {
            wanted.next();
        }
    }
    wanted.peek().is_none()
}

fn without_spaces(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

/// Ranks an entry against a lower-cased query: the label starting with it, then a word of the label,
/// then every query word somewhere in label or detail, then scattered letters. Lower is better; `None`
/// doesn't match.
pub fn rank_match(label: &str, detail: &str, query: &str) -> Option<u8> {
    if query.is_empty() {
        return Some(0);
    }
    let lower = label.to_lowercase();
    let hay = format!("{lower} {}", detail.to_lowercase());
    if lower.starts_with(query) {
        return Some(0);
    }
    let is_break = |c: char| c.is_whitespace() || matches!(c, ':' | '·' | '/' | '_' | '-');
    if lower.split(is_break).any(|word| word.starts_with(query)) {
        return Some(1);
    }
    if query.split_whitespace().all(|word| hay.contains(word)) {
        return Some(2);
    }
    subsequence(&without_spaces(&lower), &without_spaces(query)).then_some(3)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_query_matches_everything_equally() {
        assert_eq!(rank_match("Quick Notes", "", ""), Some(0));
        assert_eq!(rank_match("", "", ""), Some(0));
    }

    #[test]
    fn a_label_starting_with_the_query_ranks_best() {
        for (label, query) in [
            ("Quick Notes", "quick"),
            ("QUICK Notes", "qui"),
            ("Quick Notes", "quick notes"),
        ] {
            assert_eq!(rank_match(label, "", query), Some(0), "{label} / {query}");
        }
    }

    #[test]
    fn a_word_of_the_label_ranks_next() {
        for (label, query) in [
            ("Open Quick Notes", "notes"),
            ("Quick Notes…", "notes"),
            ("Music: Pause", "pause"),
            ("Journal · Dawn", "dawn"),
            ("Links/Work", "work"),
            ("open_now", "now"),
            ("Open-Now", "now"),
        ] {
            assert_eq!(rank_match(label, "", query), Some(1), "{label} / {query}");
        }
    }

    #[test]
    fn query_words_found_anywhere_in_label_or_detail_rank_third() {
        for (label, detail, query) in [
            ("Wiki", "Oscorp", "osc"),
            ("Wiki", "Oscorp", "oscorp wiki"),
            ("Open Quick Notes", "", "notes quick"),
            ("Journal: Dawn · 1/3", "", "1/3"),
            ("Quick Notes", "", "ick"),
        ] {
            assert_eq!(
                rank_match(label, detail, query),
                Some(2),
                "{label} / {detail} / {query}"
            );
        }
    }

    #[test]
    fn scattered_letters_in_order_rank_last() {
        for (label, query) in [
            ("Quick Notes", "qn"),
            ("Quick Notes", "qk n"),
            ("Switch Context…", "switch ctx"),
        ] {
            assert_eq!(rank_match(label, "", query), Some(3), "{label} / {query}");
        }
    }

    #[test]
    fn the_letters_must_come_in_order_and_the_detail_does_not_count_for_them() {
        for (label, detail, query) in [
            ("Quick", "", "kq"),
            ("Quick Notes", "", "zz"),
            ("Wiki", "Oscorp", "wp"),
            ("Quick", "", "quickest"),
        ] {
            assert_eq!(
                rank_match(label, detail, query),
                None,
                "{label} / {detail} / {query}"
            );
        }
    }

    #[test]
    fn a_better_kind_of_match_has_a_lower_rank() {
        let ranks: Vec<Option<u8>> = [
            ("Notes Archive", ""),
            ("Quick Notes", ""),
            ("Jot", "notes"),
            ("Nice Old Tidy Embedded Sky", ""),
            ("Other", ""),
        ]
        .iter()
        .map(|(label, detail)| rank_match(label, detail, "notes"))
        .collect();
        assert_eq!(ranks, [Some(0), Some(1), Some(2), Some(3), None]);
    }
}
