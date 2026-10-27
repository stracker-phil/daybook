//! The Obsidian CLI (Obsidian → Settings → General → Command line interface, 1.12+) and
//! `obsidian://` links.

use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use crate::lock::MutexExt;
use crate::process::{self, is_executable};

const CANDIDATES: &[&str] = &[
    "/usr/local/bin/obsidian",
    "/opt/homebrew/bin/obsidian",
    "/Applications/Obsidian.app/Contents/MacOS/obsidian",
];
const LOOKUP_TIMEOUT: Duration = Duration::from_secs(5);
const CLI_TIMEOUT: Duration = Duration::from_secs(20);
const NOT_FOUND: &str =
    "Obsidian CLI not found. Turn it on in Obsidian → Settings → General → Command line interface.";

/// Remembers where the CLI was found, per configured path; a CLI that wasn't found is looked for again.
#[derive(Default)]
pub struct ObsidianCli {
    found: Mutex<Option<(String, PathBuf)>>,
}

impl ObsidianCli {
    /// `configured` (Settings → Connections → Vault) wins; else the login shell's `PATH`, else the usual places.
    pub fn find(&self, configured: &str) -> Option<PathBuf> {
        if let Some((key, path)) = self.found.lock_unpoisoned().as_ref()
            && key == configured
        {
            return Some(path.clone());
        }
        let found = lookup(configured)?;
        *self.found.lock_unpoisoned() = Some((configured.to_string(), found.clone()));
        Some(found)
    }

    /// Runs `obsidian vault=<vault_name> <args>` and returns its output.
    pub fn run(&self, configured: &str, vault_name: &str, args: &[&str]) -> Result<String, String> {
        let cli = self.find(configured).ok_or_else(|| NOT_FOUND.to_string())?;
        let mut all = vec![format!("vault={vault_name}")];
        all.extend(args.iter().map(|arg| arg.to_string()));
        let output = process::run(&cli, &all, CLI_TIMEOUT).map_err(|e| e.to_string())?;
        if output.success {
            return Ok(output.stdout);
        }
        let failed = format!("obsidian {} failed", args.join(" "));
        let stderr = output.stderr.trim();
        log::warn!("{failed}: {stderr}");
        Err(if stderr.is_empty() {
            failed
        } else {
            stderr.to_string()
        })
    }
}

fn lookup(configured: &str) -> Option<PathBuf> {
    let configured = configured.trim();
    if !configured.is_empty() {
        let path = PathBuf::from(configured);
        return is_executable(&path).then_some(path);
    }
    from_login_shell().or_else(|| {
        CANDIDATES
            .iter()
            .map(PathBuf::from)
            .find(|path| is_executable(path))
    })
}

/// `command -v obsidian` in the owner's shell; a failed shell just means "not found there".
fn from_login_shell() -> Option<PathBuf> {
    let output = process::run_in_login_shell("command -v obsidian", LOOKUP_TIMEOUT).ok()?;
    let last = output.stdout.trim().lines().last()?.trim();
    let path = PathBuf::from(last);
    (last.starts_with('/') && is_executable(&path)).then_some(path)
}

/// JavaScript's `encodeURIComponent`.
fn encode_component(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// `http://`, `https://` or `obsidian://`; anything else is a vault path.
pub fn is_url(target: &str) -> bool {
    target.split_once("://").is_some_and(|(scheme, _)| {
        ["http", "https", "obsidian"]
            .iter()
            .any(|known| scheme.eq_ignore_ascii_case(known))
    })
}

/// The file a vault-relative `target` names (a note when it has no extension); `None` when it would leave
/// the vault.
pub fn vault_file(vault: &Path, target: &str) -> Option<PathBuf> {
    let relative = Path::new(target.trim());
    let inside = relative
        .components()
        .all(|part| matches!(part, Component::Normal(_) | Component::CurDir));
    if !inside || relative.file_name().is_none() {
        return None;
    }
    let file = vault.join(relative);
    if file.extension().is_some() {
        return Some(file);
    }
    let mut note = file.into_os_string();
    note.push(".md");
    Some(note.into())
}

/// `obsidian://open` for a vault file, optionally at a heading; `None` outside the vault.
pub fn open_url(
    vault_name: &str,
    vault: &Path,
    file: &Path,
    heading: Option<&str>,
) -> Option<String> {
    let relative = file
        .strip_prefix(vault)
        .ok()?
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    let target = match heading {
        Some(heading) => format!("{relative}#{heading}"),
        None => relative,
    };
    Some(format!(
        "obsidian://open?vault={}&file={}",
        encode_component(vault_name),
        encode_component(&target)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_link_encodes_vault_and_file_like_encode_uri_component() {
        let vault = Path::new("/v");
        for (vault_name, file, expected) in [
            (
                "Vault",
                "/v/Note.md",
                "obsidian://open?vault=Vault&file=Note.md",
            ),
            (
                "My Vault",
                "/v/1-Journal/2026-10-24.md",
                "obsidian://open?vault=My%20Vault&file=1-Journal%2F2026-10-24.md",
            ),
            (
                "Vault",
                "/v/Ideen/Über uns #1.md",
                "obsidian://open?vault=Vault&file=Ideen%2F%C3%9Cber%20uns%20%231.md",
            ),
            (
                "Vault",
                "/v/a-b_c.d!~*'()?&=+.md",
                "obsidian://open?vault=Vault&file=a-b_c.d!~*'()%3F%26%3D%2B.md",
            ),
        ] {
            assert_eq!(
                open_url(vault_name, vault, Path::new(file), None).as_deref(),
                Some(expected),
                "{file}"
            );
        }
    }

    #[test]
    fn the_heading_is_appended_after_the_file_and_encoded_with_it() {
        let url = open_url(
            "Vault",
            Path::new("/v"),
            Path::new("/v/Note.md"),
            Some("Plan for Today"),
        );

        assert_eq!(
            url.as_deref(),
            Some("obsidian://open?vault=Vault&file=Note.md%23Plan%20for%20Today")
        );
    }

    #[test]
    fn only_web_and_obsidian_schemes_are_urls() {
        for (target, expected) in [
            ("https://x", true),
            ("http://x", true),
            ("obsidian://open?vault=V&file=N", true),
            ("HTTPS://x", true),
            ("Obsidian://open", true),
            ("3-Personal/Work-Log", false),
            ("Notes: draft", false),
            ("mailto:x", false),
            ("", false),
        ] {
            assert_eq!(is_url(target), expected, "{target}");
        }
    }

    #[test]
    fn a_vault_target_names_a_note_or_keeps_its_extension() {
        let vault = Path::new("/v");
        for (target, expected) in [
            ("3-Personal/Work-Log", "/v/3-Personal/Work-Log.md"),
            ("a/b.pdf", "/v/a/b.pdf"),
            ("  Note  ", "/v/Note.md"),
        ] {
            assert_eq!(
                vault_file(vault, target),
                Some(PathBuf::from(expected)),
                "{target}"
            );
        }
    }

    #[test]
    fn a_target_leaving_the_vault_names_no_file() {
        for target in ["../x", "/etc/passwd", "a/../../x", "", "   ", "."] {
            assert_eq!(vault_file(Path::new("/v"), target), None, "{target}");
        }
    }

    #[test]
    fn a_file_outside_the_vault_has_no_link() {
        assert_eq!(
            open_url(
                "Vault",
                Path::new("/v"),
                Path::new("/elsewhere/Note.md"),
                None
            ),
            None
        );
    }
}
