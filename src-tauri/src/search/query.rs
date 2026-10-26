//! The search field's text as terms and `@scope`s.

use std::sync::LazyLock;

use regex::Regex;

/// Longer text is cut before parsing.
const MAX_QUERY_CHARS: usize = 500;

static PHRASE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#""([^"]+)""#).unwrap());

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    /// Lowercased; words first, then quoted phrases.
    pub terms: Vec<String>,
    /// Lowercased names after `@`, unchecked.
    pub scopes: Vec<String>,
}

/// `Release "next week" @work` → terms `release`, `next week`; scope `work`.
pub fn parse(text: &str) -> Query {
    let text: String = text.chars().take(MAX_QUERY_CHARS).collect();
    let phrases: Vec<String> = PHRASE
        .captures_iter(&text)
        .map(|caps| caps[1].trim().to_lowercase())
        .filter(|phrase| !phrase.is_empty())
        .collect();
    let rest = PHRASE.replace_all(&text, " ");
    let mut query = Query::default();
    for token in rest.split_whitespace() {
        if let Some(scope) = token.strip_prefix('@') {
            if !scope.is_empty() {
                query.scopes.push(scope.to_lowercase());
            }
            continue;
        }
        let word = token.replace('"', "").to_lowercase();
        if !word.is_empty() {
            query.terms.push(word);
        }
    }
    query.terms.extend(phrases);
    query
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn words_phrases_and_scopes_are_told_apart() {
        for (input, terms, scopes) in [
            (
                "Release \"next week\" @work notes",
                &["release", "notes", "next week"][..],
                &["work"][..],
            ),
            ("@ @Journal", &[][..], &["journal"][..]),
            ("\"  \" lonely\"quote", &["lonelyquote"][..], &[][..]),
        ] {
            assert_eq!(
                parse(input),
                Query {
                    terms: strings(terms),
                    scopes: strings(scopes)
                },
                "{input}"
            );
        }
    }

    #[test]
    fn words_come_before_phrases_and_duplicates_stay() {
        assert_eq!(
            parse("\"b c\" a a \"b c\"").terms,
            strings(&["a", "a", "b c", "b c"])
        );
    }

    #[test]
    fn terms_and_scopes_are_lowercased() {
        let query = parse("ÄPFEL \"Next Week\" @Work");
        assert_eq!(query.terms, strings(&["äpfel", "next week"]));
        assert_eq!(query.scopes, strings(&["work"]));
    }

    #[test]
    fn text_is_cut_to_the_maximum_length_before_parsing() {
        let text = format!("{} tail", "a".repeat(MAX_QUERY_CHARS));
        assert_eq!(parse(&text).terms, vec!["a".repeat(MAX_QUERY_CHARS)]);
    }

    #[test]
    fn an_empty_text_has_no_terms_and_no_scopes() {
        assert_eq!(parse(""), Query::default());
        assert_eq!(parse("   "), Query::default());
    }
}
