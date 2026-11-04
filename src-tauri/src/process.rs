//! Runs other programs with a time limit and the usual tool folders on the `PATH`.

use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

/// An app started from Finder has a bare `PATH`; tools installed by Homebrew live here.
const EXTRA_PATH: &[&str] = &[
    "/opt/homebrew/bin",
    "/usr/local/bin",
    "/usr/bin",
    "/bin",
    "/usr/sbin",
    "/sbin",
];
/// `lsappinfo list` and `pmset -g assertions` grow with every running app.
const MAX_OUTPUT: u64 = 4 * 1024 * 1024;
const POLL: Duration = Duration::from_millis(20);

#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub success: bool,
    /// `None` when a signal ended it.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, thiserror::Error)]
pub enum RunError {
    #[error("Couldn't start {program}: {source}")]
    Spawn { program: String, source: io::Error },
    #[error("Couldn't wait for {program}: {source}")]
    Wait { program: String, source: io::Error },
    #[error("{program} didn't finish within {} seconds", .timeout.as_secs())]
    TimedOut { program: String, timeout: Duration },
}

pub(crate) fn path_with_tools() -> String {
    let mut path = EXTRA_PATH.join(":");
    if let Ok(existing) = std::env::var("PATH") {
        path.push(':');
        path.push_str(&existing);
    }
    path
}

fn read_capped(pipe: Option<impl Read + Send + 'static>) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut pipe) = pipe {
            let _ = (&mut pipe).take(MAX_OUTPUT).read_to_end(&mut bytes);
            // The rest is drained, so a chatty program never blocks on a full pipe.
            let _ = io::copy(&mut pipe, &mut io::sink());
        }
        // Not all output is UTF-8: pmset prints device names in Mac Roman.
        String::from_utf8_lossy(&bytes).into_owned()
    })
}

/// The exit status, or `None` once `deadline` passed.
fn wait_until(child: &mut Child, deadline: Instant) -> io::Result<Option<ExitStatus>> {
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Some(status));
        }
        if Instant::now() >= deadline {
            return Ok(None);
        }
        thread::sleep(POLL);
    }
}

/// Runs `program` without a shell and kills it after `timeout`.
pub fn run(program: &Path, args: &[String], timeout: Duration) -> Result<Output, RunError> {
    let name = program.display().to_string();
    let mut child = Command::new(program)
        .args(args)
        .env("PATH", path_with_tools())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| RunError::Spawn {
            program: name.clone(),
            source,
        })?;
    let stdout = read_capped(child.stdout.take());
    let stderr = read_capped(child.stderr.take());
    let status = match wait_until(&mut child, Instant::now() + timeout) {
        Ok(Some(status)) => status,
        finished => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(match finished {
                Err(source) => RunError::Wait {
                    program: name,
                    source,
                },
                _ => RunError::TimedOut {
                    program: name,
                    timeout,
                },
            });
        }
    };
    Ok(Output {
        success: status.success(),
        code: status.code(),
        stdout: stdout.join().unwrap_or_default(),
        stderr: stderr.join().unwrap_or_default(),
    })
}

/// Runs `command` in a login zsh, so the owner's own `PATH` and shell setup apply.
pub fn run_in_login_shell(command: &str, timeout: Duration) -> Result<Output, RunError> {
    let args = ["-l".to_string(), "-c".to_string(), command.to_string()];
    run(Path::new("/bin/zsh"), &args, timeout)
}

/// Where command line tools of the owner's own apps live; an app doesn't get the login `PATH`.
pub fn user_tool_folders() -> Vec<PathBuf> {
    std::env::home_dir()
        .map(|home| home.join(".local/bin"))
        .into_iter()
        .chain(["/usr/local/bin", "/opt/homebrew/bin"].map(PathBuf::from))
        .collect()
}

pub fn find_tool(folders: &[PathBuf], name: &str) -> Option<PathBuf> {
    folders
        .iter()
        .map(|folder| folder.join(name))
        .find(|tool| is_executable(tool))
}

/// A regular file with an execute bit.
pub fn is_executable(file: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(file)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sh(script: &str, timeout: Duration) -> Result<Output, RunError> {
        run(
            Path::new("/bin/sh"),
            &["-c".to_string(), script.to_string()],
            timeout,
        )
    }

    #[test]
    fn a_command_that_succeeds_returns_its_stdout() {
        let output = run(
            Path::new("/bin/echo"),
            &["hello".to_string(), "there".to_string()],
            Duration::from_secs(5),
        )
        .unwrap();

        assert_eq!(
            output,
            Output {
                success: true,
                code: Some(0),
                stdout: "hello there\n".into(),
                stderr: String::new(),
            }
        );
    }

    #[test]
    fn a_non_zero_exit_is_reported_with_its_stderr() {
        let output = sh("echo out; echo oops >&2; exit 3", Duration::from_secs(5)).unwrap();

        assert_eq!(
            output,
            Output {
                success: false,
                code: Some(3),
                stdout: "out\n".into(),
                stderr: "oops\n".into(),
            }
        );
    }

    #[test]
    fn output_that_is_not_valid_utf8_keeps_its_valid_text() {
        let output = sh(
            r"printf 'before\325after\n'; printf 'err\325or\n' >&2",
            Duration::from_secs(5),
        )
        .unwrap();

        assert_eq!(output.stdout, "before\u{FFFD}after\n");
        assert_eq!(output.stderr, "err\u{FFFD}or\n");
    }

    #[test]
    fn a_command_past_its_timeout_is_killed_quickly() {
        let started = Instant::now();

        let result = run(
            Path::new("/bin/sleep"),
            &["30".to_string()],
            Duration::from_millis(100),
        );

        assert!(matches!(
            result,
            Err(RunError::TimedOut { timeout, .. }) if timeout == Duration::from_millis(100)
        ));
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn a_program_that_does_not_exist_fails_to_start() {
        let result = run(Path::new("/nonexistent/tool"), &[], Duration::from_secs(5));

        assert!(
            matches!(result, Err(RunError::Spawn { program, .. }) if program == "/nonexistent/tool")
        );
    }

    #[test]
    fn the_usual_tool_folders_are_on_the_path() {
        let output = sh("echo \"$PATH\"", Duration::from_secs(5)).unwrap();

        assert!(
            output
                .stdout
                .starts_with("/opt/homebrew/bin:/usr/local/bin:")
        );
    }

    #[test]
    fn only_regular_files_with_an_execute_bit_are_executable() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let tool = dir.path().join("tool");
        std::fs::write(&tool, "#!/bin/sh\n").unwrap();

        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!is_executable(&tool));
        std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(is_executable(&tool));
        assert!(!is_executable(dir.path()));
        assert!(!is_executable(&dir.path().join("missing")));
    }

    #[test]
    fn a_tool_is_found_in_the_first_folder_that_has_it_executable() {
        use std::os::unix::fs::PermissionsExt;
        let (first, second, third) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        for (dir, mode) in [(&first, 0o644), (&second, 0o755), (&third, 0o755)] {
            let tool = dir.path().join("tool");
            std::fs::write(&tool, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&tool, std::fs::Permissions::from_mode(mode)).unwrap();
        }
        let folders = vec![
            PathBuf::from("/nonexistent"),
            first.path().into(),
            second.path().into(),
            third.path().into(),
        ];

        assert_eq!(
            find_tool(&folders, "tool"),
            Some(second.path().join("tool"))
        );
        assert_eq!(find_tool(&folders, "other"), None);
        assert_eq!(find_tool(&[], "tool"), None);
    }
}
