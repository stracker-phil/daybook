//! The `plaunch` tool: finding it, listing Project Launcher's projects (briefly cached) and showing one.

use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::Serialize;
use ts_rs::TS;

use super::parse::{ListedProject, failure_message, parse_projects};
use crate::error::{AppError, AppResult};
use crate::lock::MutexExt;
use crate::process::{self, is_executable};

const TOOL: &str = "plaunch";
const TIMEOUT: Duration = Duration::from_secs(40);
const CACHE_FOR: Duration = Duration::from_secs(60);
const NOT_INSTALLED: &str = "Project Launcher’s command line tool isn’t installed. Install it in Project Launcher → Settings → Command Line.";

/// What `plaunch list` answered: `found` is false without the tool; `error` when it failed.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct LauncherProjects {
    pub found: bool,
    pub projects: Vec<ListedProject>,
    pub error: Option<String>,
}

pub struct Launcher {
    /// Where `plaunch` may be; an app doesn't get the login `PATH`.
    folders: Vec<PathBuf>,
    /// The last list and when it was made; held while a list runs, so callers share one run.
    listed: Mutex<Option<(Instant, LauncherProjects)>>,
}

impl Default for Launcher {
    fn default() -> Self {
        let home = std::env::home_dir().map(|home| home.join(".local/bin"));
        let folders = home
            .into_iter()
            .chain(["/usr/local/bin", "/opt/homebrew/bin"].map(PathBuf::from))
            .collect();
        Self::new(folders)
    }
}

impl Launcher {
    pub fn new(folders: Vec<PathBuf>) -> Self {
        Self {
            folders,
            listed: Mutex::default(),
        }
    }

    fn tool(&self) -> Option<PathBuf> {
        self.folders
            .iter()
            .map(|folder| folder.join(TOOL))
            .find(|tool| is_executable(tool))
    }

    /// Runs `plaunch` with `args`; its stdout, or why it failed.
    fn run(tool: &Path, args: &[&str]) -> Result<String, String> {
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        let output = process::run(tool, &args, TIMEOUT).map_err(|e| e.to_string())?;
        if output.success {
            Ok(output.stdout)
        } else {
            Err(failure_message(&output.stderr, "plaunch failed"))
        }
    }

    fn list_now(&self) -> LauncherProjects {
        let Some(tool) = self.tool() else {
            return LauncherProjects {
                found: false,
                projects: Vec::new(),
                error: None,
            };
        };
        match Self::run(&tool, &["list", "--json"]).and_then(|stdout| parse_projects(&stdout)) {
            Ok(projects) => LauncherProjects {
                found: true,
                projects,
                error: None,
            },
            Err(error) => LauncherProjects {
                found: true,
                projects: Vec::new(),
                error: Some(error),
            },
        }
    }

    /// The projects Project Launcher lists, kept for `CACHE_FOR` unless `fresh`; a failure isn't kept.
    /// Blocks up to `TIMEOUT`.
    pub fn list(&self, fresh: bool) -> LauncherProjects {
        let mut listed = self.listed.lock_unpoisoned();
        if !fresh
            && let Some((at, projects)) = listed.as_ref()
            && at.elapsed() < CACHE_FOR
        {
            return projects.clone();
        }
        let projects = self.list_now();
        *listed = projects
            .error
            .is_none()
            .then(|| (Instant::now(), projects.clone()));
        projects
    }

    /// Shows project `id` in Project Launcher. Blocks up to `TIMEOUT`.
    pub fn open(&self, id: &str) -> AppResult<()> {
        let tool = self
            .tool()
            .ok_or_else(|| AppError::NotFound(NOT_INSTALLED.into()))?;
        Self::run(&tool, &["open", id])
            .map(|_| ())
            .map_err(AppError::Io)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;

    const LIST_OK: &str = r#"[{"id":"proj-0001","name":"Warehouse","archived":false}]"#;

    fn install(folder: &Path, body: &str, mode: u32) {
        let tool = folder.join(TOOL);
        fs::write(&tool, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&tool, fs::Permissions::from_mode(mode)).unwrap();
    }

    fn launcher_with(body: &str) -> (tempfile::TempDir, Launcher) {
        let dir = tempfile::tempdir().unwrap();
        install(dir.path(), body, 0o755);
        let launcher = Launcher::new(vec![dir.path().to_path_buf()]);
        (dir, launcher)
    }

    fn printing(json: &str) -> String {
        format!("echo '{json}'")
    }

    fn warehouse() -> ListedProject {
        ListedProject {
            id: "proj-0001".into(),
            name: "Warehouse".into(),
            archived: false,
        }
    }

    #[test]
    fn a_missing_tool_is_found_false_without_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let launcher = Launcher::new(vec![dir.path().to_path_buf()]);
        assert_eq!(
            launcher.list(true),
            LauncherProjects {
                found: false,
                projects: vec![],
                error: None
            }
        );
    }

    #[test]
    fn the_listed_projects_are_parsed_from_the_tools_answer() {
        let (_dir, launcher) = launcher_with(&printing(LIST_OK));
        assert_eq!(
            launcher.list(true),
            LauncherProjects {
                found: true,
                projects: vec![warehouse()],
                error: None
            }
        );
    }

    #[test]
    fn a_failing_tool_reports_its_message_without_the_prefix() {
        let (_dir, launcher) = launcher_with("echo 'plaunch: boom' >&2\nexit 1");
        assert_eq!(
            launcher.list(true),
            LauncherProjects {
                found: true,
                projects: vec![],
                error: Some("boom".into())
            }
        );
    }

    #[test]
    fn a_failing_tool_without_a_message_reports_a_generic_failure() {
        let (_dir, launcher) = launcher_with("exit 1");
        assert_eq!(launcher.list(true).error, Some("plaunch failed".into()));
    }

    #[test]
    fn an_answer_that_is_not_json_is_an_error_result() {
        let (_dir, launcher) = launcher_with("echo 'not json'");
        let result = launcher.list(true);
        assert!(result.found);
        assert!(result.projects.is_empty());
        assert!(result.error.is_some());
    }

    #[test]
    fn the_list_is_kept_until_a_fresh_one_is_asked_for() {
        let (dir, launcher) = launcher_with(&printing(LIST_OK));
        assert_eq!(launcher.list(false).projects, vec![warehouse()]);

        install(dir.path(), "echo '[]'", 0o755);
        assert_eq!(launcher.list(false).projects, vec![warehouse()]);
        assert_eq!(launcher.list(true).projects, vec![]);
        assert_eq!(launcher.list(false).projects, vec![]);
    }

    #[test]
    fn a_failure_is_not_kept() {
        let (dir, launcher) = launcher_with("echo 'plaunch: boom' >&2\nexit 1");
        assert!(launcher.list(false).error.is_some());

        install(dir.path(), &printing(LIST_OK), 0o755);
        let result = launcher.list(false);
        assert_eq!(result.error, None);
        assert_eq!(result.projects, vec![warehouse()]);
    }

    #[test]
    fn the_first_folder_with_the_tool_wins() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        install(first.path(), &printing(LIST_OK), 0o755);
        install(second.path(), "echo '[]'", 0o755);
        let launcher = Launcher::new(vec![first.path().into(), second.path().into()]);
        assert_eq!(launcher.list(true).projects, vec![warehouse()]);
    }

    #[test]
    fn a_file_without_the_execute_bit_is_skipped() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        install(first.path(), "echo '[]'", 0o644);
        install(second.path(), &printing(LIST_OK), 0o755);
        let launcher = Launcher::new(vec![first.path().into(), second.path().into()]);
        assert_eq!(launcher.list(true).projects, vec![warehouse()]);

        let only_plain = Launcher::new(vec![first.path().into()]);
        assert!(!only_plain.list(true).found);
    }

    #[test]
    fn opening_without_the_tool_is_not_found_with_an_install_hint() {
        let dir = tempfile::tempdir().unwrap();
        let launcher = Launcher::new(vec![dir.path().to_path_buf()]);
        match launcher.open("proj-0001") {
            Err(AppError::NotFound(message)) => assert_eq!(
                message,
                "Project Launcher’s command line tool isn’t installed. Install it in Project Launcher → Settings → Command Line."
            ),
            other => panic!("expected NotFound, got {other:?}"),
        }
    }

    #[test]
    fn opening_runs_the_tool_with_open_and_the_id() {
        let dir = tempfile::tempdir().unwrap();
        let args = dir.path().join("args.txt");
        install(
            dir.path(),
            &format!("echo \"$@\" > '{}'", args.display()),
            0o755,
        );
        let launcher = Launcher::new(vec![dir.path().to_path_buf()]);
        launcher.open("proj-0001").unwrap();
        assert_eq!(fs::read_to_string(args).unwrap().trim(), "open proj-0001");
    }

    #[test]
    fn a_failed_open_is_a_tool_failure_with_the_tools_message() {
        let (_dir, launcher) = launcher_with("echo 'plaunch: no such project' >&2\nexit 2");
        match launcher.open("nope") {
            Err(AppError::Io(message)) => assert_eq!(message, "no such project"),
            other => panic!("expected Io, got {other:?}"),
        }
    }
}
