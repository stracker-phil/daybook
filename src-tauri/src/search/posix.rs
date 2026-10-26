//! `/`-separated vault paths, as Node's `path.posix` treats them.

/// The folder of `path`; `.` at the vault root.
pub fn dirname(path: &str) -> &str {
    match path.trim_end_matches('/').rsplit_once('/') {
        Some(("", _)) => "/",
        Some((folder, _)) => folder,
        None => ".",
    }
}

/// The last segment of `path`.
pub fn basename(path: &str) -> &str {
    let path = path.trim_end_matches('/');
    path.rsplit_once('/').map_or(path, |(_, name)| name)
}

/// `relative` joined to `folder`, with `.` and `..` segments resolved; `..` past the start stays.
pub fn join_normalize(folder: &str, relative: &str) -> String {
    let joined = format!("{folder}/{relative}");
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." if parts.last().is_some_and(|last| *last != "..") => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    let normalized = parts.join("/");
    let normalized = if joined.starts_with('/') {
        format!("/{normalized}")
    } else {
        normalized
    };
    if normalized.is_empty() {
        ".".into()
    } else {
        normalized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dirname_is_the_folder_and_a_dot_at_the_root() {
        for (path, expected) in [
            ("2-Work/Plan.md", "2-Work"),
            ("a/b/c.md", "a/b"),
            ("Plan.md", "."),
            ("/x", "/"),
        ] {
            assert_eq!(dirname(path), expected, "{path}");
        }
    }

    #[test]
    fn basename_is_the_last_segment() {
        for (path, expected) in [("a/b.md", "b.md"), ("b.md", "b.md"), ("a/b/", "b")] {
            assert_eq!(basename(path), expected, "{path}");
        }
    }

    #[test]
    fn join_normalize_resolves_dot_segments() {
        for (folder, relative, expected) in [
            ("1-Journal", "../2-Work/Plan.md", "2-Work/Plan.md"),
            (".", "a.md", "a.md"),
            ("a", "./b", "a/b"),
            ("a", "../../b", "../b"),
            ("a", "..", "."),
        ] {
            assert_eq!(
                join_normalize(folder, relative),
                expected,
                "{folder} + {relative}"
            );
        }
    }
}
