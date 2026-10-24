//! A one-time import of the Glaze prototype's settings and state files, so Daybook starts with the
//! owner's configuration. Owner-specific: deleting this module and its calls in `lib.rs` and `app.rs`
//! removes it.
//!
//! It runs while Daybook has no `settings.json` and the prototype has one. It only reads the
//! prototype's folder. Settings are written last, so an import that fails part-way runs again.

mod settings;
mod state;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::store::json_file::JsonFile;
use crate::store::settings::SETTINGS_FILE;
use settings::DROPPED;
use state::STATE_FILES;

const PROTOTYPE_DIR: &str = "Library/Application Support/app.glaze.macos.2mykv1oc-local";

/// Encrypted with Electron's safeStorage, which only the prototype can read.
const SLACK_TOKEN_FILE: &str = "slack-token.bin";

pub fn prototype_dir() -> Option<PathBuf> {
    std::env::home_dir().map(|home| home.join(PROTOTYPE_DIR))
}

#[derive(Debug, PartialEq)]
pub enum Import {
    Imported(ImportReport),
    Failed(String),
}

#[derive(Debug, PartialEq)]
pub struct ImportReport {
    pub contexts: usize,
    pub projects: usize,
    pub links: usize,
    pub routines: usize,
    pub journal_groups: usize,
    /// The Daybook state files written.
    pub state_files: Vec<String>,
    /// State files not copied, with the reason.
    pub skipped: Vec<String>,
    /// The prototype had a Slack token, which has to be entered again.
    pub slack_token: bool,
}

impl ImportReport {
    /// For the log: everything that was and wasn't imported.
    pub fn log_text(&self) -> String {
        let mut lines = vec![format!(
            "Imported the prototype's settings: {}",
            self.counts()
        )];
        if !self.state_files.is_empty() {
            lines.push(format!("State files: {}", self.state_files.join(", ")));
        }
        for skipped in &self.skipped {
            lines.push(format!("Not imported: {skipped}"));
        }
        lines.push(format!(
            "Left behind with time tracking: {}",
            DROPPED.join("; ")
        ));
        if self.slack_token {
            lines.push("The Slack token is encrypted for the prototype only".into());
        }
        lines.join("\n")
    }

    /// For the page's notice: what came over and what has to be done again.
    pub fn notice(&self) -> String {
        let mut notice = format!("Imported the prototype's settings: {}.", self.counts());
        if self.slack_token {
            notice.push_str(" Enter the Slack token again; it can't be copied.");
        }
        notice
    }

    fn counts(&self) -> String {
        let count =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        [
            count(self.contexts, "context", "contexts"),
            count(self.projects, "project", "projects"),
            count(self.links, "link", "links"),
            count(self.routines, "routine", "routines"),
            count(self.journal_groups, "journal group", "journal groups"),
        ]
        .join(", ")
    }
}

/// Imports from `prototype` into `daybook` unless there is nothing to do.
pub fn run(prototype: &Path, daybook: &Path) -> Option<Import> {
    let target = daybook.join(SETTINGS_FILE);
    let source = prototype.join(SETTINGS_FILE);
    if target.exists() || !source.is_file() {
        return None;
    }
    let raw = match read_json(&source) {
        Ok(raw) => raw,
        Err(e) => return Some(Import::Failed(e)),
    };
    let (state_files, skipped) = copy_state(prototype, daybook);
    let settings = settings::from_prototype(&raw);
    if let Err(e) = JsonFile::new(&target).save(&settings) {
        return Some(Import::Failed(format!(
            "Saving the imported settings failed: {e}"
        )));
    }
    Some(Import::Imported(ImportReport {
        contexts: settings.contexts.len(),
        projects: settings.projects.len(),
        links: settings.links.len(),
        routines: settings.routines.len(),
        journal_groups: settings.journal.len(),
        state_files,
        skipped,
        slack_token: prototype.join(SLACK_TOKEN_FILE).exists(),
    }))
}

/// Copies each state file Daybook doesn't have yet.
fn copy_state(prototype: &Path, daybook: &Path) -> (Vec<String>, Vec<String>) {
    let mut written = Vec::new();
    let mut skipped = Vec::new();
    for (from, to, convert) in STATE_FILES {
        let source = prototype.join(from);
        let target = daybook.join(to);
        if !source.is_file() {
            continue;
        }
        if target.exists() {
            skipped.push(format!("{from}: Daybook already has {to}"));
            continue;
        }
        let saved = read_json(&source).and_then(|raw| {
            JsonFile::new(&target)
                .save(&convert(&raw))
                .map_err(|e| e.to_string())
        });
        match saved {
            Ok(()) => written.push((*to).to_string()),
            Err(e) => skipped.push(format!("{from}: {e}")),
        }
    }
    (written, skipped)
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Reading {} failed: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{} is not valid JSON: {e}", path.display()))
}
