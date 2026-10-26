//! Obsidian Markdown rewritten for the preview's renderer: callouts become quotes with a bold title,
//! `[[links]]` and relative links become `#note=` links (`#missing=` when unresolved), note embeds become
//! links and vault images `#asset=` images. Code blocks and inline code stay as they are.

use std::sync::LazyLock;

use regex::{Captures, Regex};

use crate::search::files::is_image;
use crate::search::posix;
use crate::search::uri::{decode_uri, encode_component};

static FENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\s*(```|~~~)").unwrap());
static CALLOUT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*>\s*)\[!([^\]]+)\][+-]?\s*(.*)$").unwrap());
static EMBED: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"!\[\[([^\]]+)\]\]").unwrap());
static WIKILINK: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\[\[([^\]]+)\]\]").unwrap());
static IMAGE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"!\[([^\]]*)\]\(([^)\s]+)\)").unwrap());
static LINK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(^|[^!])\[([^\]]*)\]\(([^)\s]+)\)").unwrap());
static EXTERNAL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)^[a-z][a-z0-9+.-]*:").unwrap());

/// How link targets map to vault paths; the search index answers in the app.
pub trait Links {
    /// `Note`, `folder/Note` or a relative path → the note's vault path.
    fn resolve_note(&self, target: &str, from: &str) -> Option<String>;
    /// `image.png` or a relative image path → the image's vault path.
    fn resolve_asset(&self, target: &str, from: &str) -> Option<String>;
    fn has_note(&self, path: &str) -> bool;
}

fn escape_label(label: &str) -> String {
    label.replace('[', "\\[").replace(']', "\\]")
}

fn is_external(href: &str) -> bool {
    EXTERNAL.is_match(href) || href.starts_with('#')
}

/// `target` and the alias of `target|alias`.
fn split_alias(inner: &str) -> (&str, Option<&str>) {
    let mut parts = inner.split('|');
    (parts.next().unwrap_or_default(), parts.next())
}

/// `Note` → `Note`, `tip` → `Tip`.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first
            .to_uppercase()
            .chain(chars.flat_map(char::to_lowercase))
            .collect()
    })
}

/// The byte ranges of `` `code` `` spans: a backtick run up to the next run of the same length.
fn code_spans(line: &str) -> Vec<(usize, usize)> {
    let bytes = line.as_bytes();
    let run_at = |at: usize| bytes[at..].iter().take_while(|b| **b == b'`').count();
    let mut spans = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != b'`' {
            at += 1;
            continue;
        }
        let open = run_at(at);
        let mut close = at + open;
        let end = loop {
            if close >= bytes.len() {
                break None;
            }
            if bytes[close] != b'`' {
                close += 1;
                continue;
            }
            let run = run_at(close);
            if run == open {
                break Some(close + run);
            }
            close += run;
        };
        match end {
            Some(end) => {
                spans.push((at, end));
                at = end;
            }
            None => at += open,
        }
    }
    spans
}

/// A callout's first line as a quote with its bold title (the type when it has none).
fn callout(line: &str) -> Option<String> {
    let caps = CALLOUT.captures(line)?;
    let title = match caps[3].trim() {
        "" => capitalized(caps[2].trim()),
        title => title.to_string(),
    };
    Some(format!("{}**{title}**", &caps[1]))
}

struct Rewriter<'a, L: Links> {
    from: &'a str,
    links: &'a L,
}

impl<L: Links> Rewriter<'_, L> {
    fn note_href(&self, target: &str) -> String {
        match self.links.resolve_note(target, self.from) {
            Some(path) => format!("#note={}", encode_component(&path)),
            None => format!("#missing={}", encode_component(target)),
        }
    }

    fn line(&self, line: &str) -> String {
        if let Some(callout) = callout(line) {
            return callout;
        }
        let mut out = String::with_capacity(line.len());
        let mut at = 0;
        for (start, end) in code_spans(line) {
            out.push_str(&self.text(&line[at..start]));
            out.push_str(&line[start..end]);
            at = end;
        }
        out.push_str(&self.text(&line[at..]));
        out
    }

    fn text(&self, text: &str) -> String {
        if text.is_empty() {
            return String::new();
        }
        // An embed holds a wikilink, so embeds go first; each later step leaves the `#…` hrefs alone.
        let text = EMBED.replace_all(text, |caps: &Captures| self.embed(&caps[1]));
        let text = WIKILINK.replace_all(&text, |caps: &Captures| {
            let (target, alias) = split_alias(&caps[1]);
            let label = escape_label(alias.unwrap_or(target).trim());
            format!("[{label}]({})", self.note_href(target))
        });
        let text = IMAGE.replace_all(&text, |caps: &Captures| self.image(caps));
        let text = LINK.replace_all(&text, |caps: &Captures| self.link(caps));
        text.into_owned()
    }

    fn embed(&self, inner: &str) -> String {
        let (target, alias) = split_alias(inner);
        let label = escape_label(alias.unwrap_or(target));
        if !is_image(target.trim()) {
            return format!("[{label}]({})", self.note_href(target));
        }
        self.asset_image(&label, target)
            .unwrap_or_else(|| format!("*{target}*"))
    }

    /// `![label](#asset=…)` when `target` names an image of the vault.
    fn asset_image(&self, label: &str, target: &str) -> Option<String> {
        let asset = self.links.resolve_asset(target, self.from)?;
        Some(format!("![{label}](#asset={})", encode_component(&asset)))
    }

    fn image(&self, caps: &Captures) -> String {
        let href = &caps[2];
        if is_external(href) {
            return caps[0].to_string();
        }
        self.asset_image(&caps[1], href)
            .unwrap_or_else(|| caps[0].to_string())
    }

    fn link(&self, caps: &Captures) -> String {
        let href = &caps[3];
        if is_external(href) {
            return caps[0].to_string();
        }
        let decoded = decode_uri(href).unwrap_or_else(|| href.to_string());
        let relative = posix::join_normalize(posix::dirname(self.from), &decoded);
        let target = if self.links.has_note(&relative) {
            relative
        } else {
            decoded
        };
        format!("{}[{}]({})", &caps[1], &caps[2], self.note_href(&target))
    }
}

/// The note's body lines (`from` is its vault path) as Markdown the preview renders.
pub fn preview_markdown(lines: &[String], from: &str, links: &impl Links) -> String {
    let rewriter = Rewriter { from, links };
    let mut in_fence = false;
    lines
        .iter()
        .map(|line| {
            let fence = FENCE.is_match(line);
            if fence {
                in_fence = !in_fence;
            }
            if in_fence || fence {
                line.clone()
            } else {
                rewriter.line(line)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FROM: &str = "1-Journal/2026-10-12.md";

    struct Fixture;

    const NOTES: [&str; 3] = [
        "2-Work/Plan.md",
        "2-Work/Plan (v2).md",
        "1-Journal/2026-10-12.md",
    ];

    impl Links for Fixture {
        fn resolve_note(&self, target: &str, _from: &str) -> Option<String> {
            let wanted = format!("{}.md", target.strip_suffix(".md").unwrap_or(target));
            NOTES
                .iter()
                .find(|note| **note == wanted || note.ends_with(&format!("/{wanted}")))
                .map(|note| note.to_string())
        }

        fn resolve_asset(&self, target: &str, _from: &str) -> Option<String> {
            target
                .ends_with(".png")
                .then(|| format!("_/assets/{target}"))
        }

        fn has_note(&self, path: &str) -> bool {
            NOTES.contains(&path)
        }
    }

    fn render_from(lines: &[&str], from: &str) -> String {
        let lines: Vec<String> = lines.iter().map(|line| line.to_string()).collect();
        preview_markdown(&lines, from, &Fixture)
    }

    fn render(line: &str) -> String {
        render_from(&[line], FROM)
    }

    #[test]
    fn a_callout_becomes_a_quote_with_a_bold_title() {
        for (input, expected) in [
            ("> [!NOTE]+ Dawn", "> **Dawn**"),
            ("> [!tip]", "> **Tip**"),
            ("> [!WARNING]- ", "> **Warning**"),
        ] {
            assert_eq!(render(input), expected, "{input}");
        }
    }

    #[test]
    fn wikilinks_link_to_notes_and_mark_missing_ones() {
        assert_eq!(
            render("See [[Plan|the plan]] and [[Nope]]"),
            "See [the plan](#note=2-Work%2FPlan.md) and [Nope](#missing=Nope)"
        );
    }

    #[test]
    fn a_note_embed_becomes_a_plain_link() {
        assert_eq!(render("![[Plan]]"), "[Plan](#note=2-Work%2FPlan.md)");
    }

    #[test]
    fn an_image_embed_becomes_an_asset_image() {
        assert_eq!(
            render("![[shot.png|Screen]]"),
            "![Screen](#asset=_%2Fassets%2Fshot.png)"
        );
    }

    #[test]
    fn an_image_embed_without_the_file_is_emphasised_text() {
        assert_eq!(render("![[missing.jpg]]"), "*missing.jpg*");
    }

    #[test]
    fn external_links_and_images_stay_as_they_are() {
        let line = "[Site](https://example.com) ![x](https://example.com/x.png)";
        assert_eq!(render(line), line);
    }

    #[test]
    fn a_relative_link_resolves_from_the_notes_folder() {
        assert_eq!(
            render("[Plan](../2-Work/Plan.md)"),
            "[Plan](#note=2-Work%2FPlan.md)"
        );
    }

    #[test]
    fn a_relative_link_is_percent_decoded_before_it_is_resolved() {
        assert_eq!(
            render("[Plan](../2-Work/Plan%20%28v2%29.md)"),
            "[Plan](#note=2-Work%2FPlan%20%28v2%29.md)"
        );
    }

    #[test]
    fn a_malformed_percent_escape_falls_back_to_the_raw_target() {
        assert_eq!(render("[Odd](100%.md)"), "[Odd](#missing=100%25.md)");
    }

    #[test]
    fn parentheses_in_a_target_are_encoded_so_they_cannot_end_the_link() {
        assert_eq!(
            render("[[Plan (v2)]]"),
            "[Plan (v2)](#note=2-Work%2FPlan%20%28v2%29.md)"
        );
    }

    #[test]
    fn a_local_markdown_image_becomes_an_asset_image_when_found() {
        assert_eq!(
            render("![alt](shot.png)"),
            "![alt](#asset=_%2Fassets%2Fshot.png)"
        );
        assert_eq!(render("![alt](shot.gif)"), "![alt](shot.gif)");
    }

    #[test]
    fn links_inside_code_fences_stay_as_they_are() {
        assert_eq!(
            render_from(&["```", "[[Plan]]", "```", "[[Plan]]"], "x.md"),
            "```\n[[Plan]]\n```\n[Plan](#note=2-Work%2FPlan.md)"
        );
        assert_eq!(
            render_from(&["~~~", "[[Plan]]", "~~~"], "x.md"),
            "~~~\n[[Plan]]\n~~~"
        );
    }

    #[test]
    fn links_inside_inline_code_stay_as_they_are() {
        assert_eq!(
            render("Use `[[Plan]]` and [[Plan]], not ``[[Plan]]``"),
            "Use `[[Plan]]` and [Plan](#note=2-Work%2FPlan.md), not ``[[Plan]]``"
        );
    }

    #[test]
    fn an_unclosed_backtick_does_not_protect_the_line() {
        assert_eq!(
            render("a ` b [[Plan]]"),
            "a ` b [Plan](#note=2-Work%2FPlan.md)"
        );
    }

    #[test]
    fn lines_are_joined_with_newlines() {
        assert_eq!(render_from(&["a", "", "b"], FROM), "a\n\nb");
        assert_eq!(render_from(&[], FROM), "");
    }
}
