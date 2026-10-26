//! JavaScript's URI helpers, for link targets in notes and the preview's hrefs.

/// `encodeURIComponent`, except that `(` and `)` are encoded too, so the href can't end a Markdown link.
pub fn encode_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for b in text.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The byte a `%XX` escape at `at` stands for.
fn hex_escape(bytes: &[u8], at: usize) -> Option<u8> {
    let hex = bytes
        .get(at + 1..at + 3)
        .filter(|hex| hex.iter().all(u8::is_ascii_hexdigit))?;
    u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok()
}

/// `decodeURI`: escapes of the reserved characters stay; `None` for a malformed escape.
pub fn decode_uri(text: &str) -> Option<String> {
    const RESERVED: &[u8] = b";/?:@&=+$,#";
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }
        let byte = hex_escape(bytes, i)?;
        if RESERVED.contains(&byte) {
            out.extend_from_slice(&bytes[i..i + 3]);
        } else {
            out.push(byte);
        }
        i += 3;
    }
    String::from_utf8(out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_component_escapes_like_encode_uri_component_and_also_parentheses() {
        for (input, expected) in [
            ("2-Work/Plan.md", "2-Work%2FPlan.md"),
            ("a b", "a%20b"),
            ("Plan (v2)", "Plan%20%28v2%29"),
            ("é", "%C3%A9"),
            ("AZaz09-_.!~*'", "AZaz09-_.!~*'"),
            ("", ""),
        ] {
            assert_eq!(encode_component(input), expected, "{input}");
        }
    }

    #[test]
    fn decode_uri_decodes_escapes() {
        for (input, expected) in [
            ("a%20b", "a b"),
            ("%C3%A9", "é"),
            ("%28x%29", "(x)"),
            ("plain", "plain"),
        ] {
            assert_eq!(decode_uri(input).as_deref(), Some(expected), "{input}");
        }
    }

    #[test]
    fn decode_uri_keeps_escapes_of_reserved_characters() {
        for reserved in ";/?:@&=+$,#".bytes() {
            let escaped = format!("a%{reserved:02X}b");
            assert_eq!(decode_uri(&escaped).as_deref(), Some(escaped.as_str()));
        }
    }

    #[test]
    fn decode_uri_rejects_malformed_escapes_and_invalid_utf8() {
        for input in ["%zz", "%2", "100%", "%", "%FF", "%C3"] {
            assert_eq!(decode_uri(input), None, "{input}");
        }
    }
}
