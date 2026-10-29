//! The `taskboard` tool: opens an issue in Taskboard, which starts when it isn't running.

use std::path::PathBuf;
use std::time::Duration;

use crate::process;

const TOOL: &str = "taskboard";
/// Long enough for Taskboard to start.
const TIMEOUT: Duration = Duration::from_secs(10);

/// Why Taskboard didn't open an issue.
#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    Missing,
    Failed(String),
}

/// Something that shows an issue by its key; `Taskboard`, or a fake in tests.
pub trait IssueApp {
    fn open(&self, key: &str) -> Result<(), Failure>;
}

pub struct Taskboard {
    folders: Vec<PathBuf>,
}

impl Default for Taskboard {
    fn default() -> Self {
        Self::new(process::user_tool_folders())
    }
}

impl Taskboard {
    pub fn new(folders: Vec<PathBuf>) -> Self {
        Self { folders }
    }

    pub fn is_installed(&self) -> bool {
        process::find_tool(&self.folders, TOOL).is_some()
    }
}

impl IssueApp for Taskboard {
    fn open(&self, key: &str) -> Result<(), Failure> {
        let tool = process::find_tool(&self.folders, TOOL).ok_or(Failure::Missing)?;
        let args = ["open".to_string(), key.to_string()];
        let output =
            process::run(&tool, &args, TIMEOUT).map_err(|e| Failure::Failed(e.to_string()))?;
        if output.success {
            return Ok(());
        }
        let said = [output.stderr.trim(), output.stdout.trim()]
            .into_iter()
            .find(|text| !text.is_empty());
        Err(Failure::Failed(said.map_or_else(
            || format!("Taskboard couldn't open {key}."),
            str::to_string,
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    fn taskboard_with(script: &str, mode: u32) -> (tempfile::TempDir, Taskboard) {
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("taskboard");
        std::fs::write(&tool, script).unwrap();
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(mode)).unwrap();
        let taskboard = Taskboard::new(vec![dir.path().to_path_buf()]);
        (dir, taskboard)
    }

    fn script(body: &str) -> (tempfile::TempDir, Taskboard) {
        taskboard_with(&format!("#!/bin/sh\n{body}\n"), 0o755)
    }

    #[test]
    fn without_the_tool_taskboard_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let taskboard = Taskboard::new(vec![dir.path().to_path_buf()]);

        assert!(!taskboard.is_installed());
        assert_eq!(taskboard.open("QRS-1"), Err(Failure::Missing));
    }

    #[test]
    fn a_file_without_an_execute_bit_is_not_installed() {
        let (_dir, taskboard) = taskboard_with("#!/bin/sh\nexit 0\n", 0o644);

        assert!(!taskboard.is_installed());
        assert_eq!(taskboard.open("QRS-1"), Err(Failure::Missing));
    }

    #[test]
    fn a_tool_that_exits_cleanly_has_opened_the_issue() {
        let (_dir, taskboard) = script("exit 0");

        assert!(taskboard.is_installed());
        assert_eq!(taskboard.open("QRS-1"), Ok(()));
    }

    #[test]
    fn the_tool_is_called_with_open_and_the_key() {
        let (_dir, taskboard) = script(r#"[ "$1" = open ] && [ "$2" = QRS-1 ] && [ $# -eq 2 ]"#);

        assert_eq!(taskboard.open("QRS-1"), Ok(()));
        assert!(taskboard.open("QRS-2").is_err());
    }

    #[test]
    fn a_failure_is_reported_with_what_the_tool_said_on_stderr() {
        let (_dir, taskboard) = script("echo 'No issue QRS-1' >&2\nexit 1");

        assert_eq!(
            taskboard.open("QRS-1"),
            Err(Failure::Failed("No issue QRS-1".into()))
        );
    }

    #[test]
    fn a_failure_falls_back_to_stdout_when_stderr_is_empty() {
        let (_dir, taskboard) = script("echo 'Not running'\nexit 1");

        assert_eq!(
            taskboard.open("QRS-1"),
            Err(Failure::Failed("Not running".into()))
        );
    }

    #[test]
    fn a_silent_failure_names_the_issue_that_didnt_open() {
        let (_dir, taskboard) = script("exit 1");

        assert_eq!(
            taskboard.open("QRS-1"),
            Err(Failure::Failed("Taskboard couldn't open QRS-1.".into()))
        );
    }
}
