//! The `timetracker` command line tool behind the `TimeTracker` trait (the only seam: tests use a fake),
//! what its exit codes mean, and the calls Daybook makes.

use std::io::{BufRead, BufReader, Lines};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use chrono::NaiveDate;

use super::reply::{
    TimerState, TtProject, TtStatus, parse_projects, parse_refusal, parse_status, parse_timer,
};
use super::watch::TtState;
use crate::error::{AppError, AppResult, ErrorKind};
use crate::lock::MutexExt;
use crate::process::{self, RunError};

/// Above TimeTracker's own 5 s socket timeout, so its own answer (not running) wins.
pub const TIMEOUT: Duration = Duration::from_secs(8);
pub const DEFAULT_PATH: &str = "~/.local/bin/timetracker";

pub const NOT_RUNNING: &str = "TimeTracker isn't running.";
pub const NO_ANSWER: &str = "TimeTracker didn't answer.";
pub const MISSING: &str = "TimeTracker isn't installed (Settings → TimeTracker).";

/// How a run of the tool ended.
#[derive(Clone, Debug, PartialEq)]
pub struct RawReply {
    /// `None` when a signal ended it.
    pub code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

/// The tool didn't give a reply.
#[derive(Clone, Debug, PartialEq)]
pub enum RunFailure {
    /// Not there or not executable.
    Missing,
    /// No reply within `TIMEOUT`; it was killed.
    TimedOut,
}

/// A running `watch --json`.
pub trait Watch: Send {
    /// The next line on stdout; `None` once it ended.
    fn next_line(&mut self) -> Option<String>;
    /// Waits for the exit after the last line.
    fn end(self: Box<Self>) -> RawReply;
}

pub trait TimeTracker: Send + Sync {
    /// Runs `timetracker <args>` and waits up to `TIMEOUT`.
    fn run(&self, args: &[&str]) -> Result<RawReply, RunFailure>;
    /// Starts `timetracker watch --json`.
    fn watch(&self) -> Result<Box<dyn Watch>, RunFailure>;
}

/// Commands that change no data; a debug build runs only these.
const READS: &[&[&str]] = &[
    &["status"],
    &["projects"],
    &["clients"],
    &["tags"],
    &["watch"],
    &["show"],
    &["focus"],
    &["timer", "status"],
    &["timer", "recent"],
];

/// Whether `args` would change TimeTracker's data.
pub fn changes_data(args: &[&str]) -> bool {
    let words: Vec<&str> = args
        .iter()
        .copied()
        .filter(|arg| !arg.starts_with('-'))
        .collect();
    !READS.iter().any(|read| words.starts_with(read))
}

fn mentions_not_running(stderr: &str) -> bool {
    stderr.contains("is not running") || stderr.contains("Could not reach")
}

/// The first line of stderr without clap's `error: `.
fn stderr_message(stderr: &str) -> Option<String> {
    let line = stderr
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())?;
    Some(
        line.strip_prefix("error:")
            .unwrap_or(line)
            .trim()
            .to_string(),
    )
}

/// Stdout of a run that worked, or the error it means.
pub fn reply_result(reply: Result<RawReply, RunFailure>) -> AppResult<String> {
    let reply = match reply {
        Ok(reply) => reply,
        Err(RunFailure::Missing) => return Err(AppError::TimeTrackerMissing(MISSING.into())),
        Err(RunFailure::TimedOut) => {
            return Err(AppError::TimeTrackerNotRunning(NO_ANSWER.into()));
        }
    };
    match reply.code {
        Some(0) => Ok(reply.stdout),
        Some(2) if mentions_not_running(&reply.stderr) => {
            Err(AppError::TimeTrackerNotRunning(NOT_RUNNING.into()))
        }
        None => Err(AppError::TimeTrackerNotRunning(NO_ANSWER.into())),
        Some(code) => Err(AppError::TimeTrackerRefused(
            parse_refusal(&reply.stdout)
                .or_else(|| stderr_message(&reply.stderr))
                .unwrap_or_else(|| format!("timetracker failed with exit code {code}")),
        )),
    }
}

/// The state a failed call leaves.
pub fn state_of(error: &AppError) -> TtState {
    if error.kind() == ErrorKind::TimetrackerMissing {
        TtState::Missing
    } else {
        TtState::NotRunning
    }
}

fn call(tt: &dyn TimeTracker, args: &[&str]) -> AppResult<String> {
    let mut args = args.to_vec();
    args.push("--json");
    reply_result(tt.run(&args))
}

pub fn timer_status(tt: &dyn TimeTracker) -> AppResult<Option<TimerState>> {
    parse_timer(&call(tt, &["timer", "status"])?).map_err(AppError::TimeTrackerRefused)
}

/// Every project, the archived ones too.
pub fn projects(tt: &dyn TimeTracker) -> AppResult<Vec<TtProject>> {
    parse_projects(&call(tt, &["projects", "--archived"])?).map_err(AppError::TimeTrackerRefused)
}

pub fn status(tt: &dyn TimeTracker) -> AppResult<TtStatus> {
    parse_status(&call(tt, &["status"])?).map_err(AppError::TimeTrackerRefused)
}

/// Shows TimeTracker's Day view of `day`, else today; starts TimeTracker if needed.
pub fn show(tt: &dyn TimeTracker, day: Option<NaiveDate>) -> AppResult<()> {
    let day = day.map(|day| day.format("%Y-%m-%d").to_string());
    let mut args = vec!["show"];
    args.extend(day.as_deref());
    call(tt, &args).map(|_| ())
}

/// Brings TimeTracker to the front; starts it if needed.
pub fn focus(tt: &dyn TimeTracker) -> AppResult<()> {
    call(tt, &["focus"]).map(|_| ())
}

/// The tool at `path`; empty is the default, `~/` is the home folder.
pub fn resolve_path(path: &str) -> PathBuf {
    let path = path.trim();
    let path = if path.is_empty() { DEFAULT_PATH } else { path };
    match (path.strip_prefix("~/"), std::env::home_dir()) {
        (Some(rest), Some(home)) => home.join(rest),
        _ => PathBuf::from(path),
    }
}

/// The real tool. A debug build only logs commands that change data and answers as if they ran.
pub struct Cli {
    path: PathBuf,
    dry_run: bool,
    /// The running `watch`, so quitting can end it.
    watching: Arc<Mutex<Option<Child>>>,
}

impl Cli {
    pub fn new(path: PathBuf, watching: Arc<Mutex<Option<Child>>>) -> Self {
        Self {
            path,
            dry_run: cfg!(debug_assertions),
            watching,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl TimeTracker for Cli {
    fn run(&self, args: &[&str]) -> Result<RawReply, RunFailure> {
        if self.dry_run && changes_data(args) {
            log::info!("debug build: would run timetracker {}", args.join(" "));
            return Ok(RawReply {
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            });
        }
        let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
        match process::run(&self.path, &args, TIMEOUT) {
            Ok(output) => Ok(RawReply {
                code: output.code,
                stdout: output.stdout,
                stderr: output.stderr,
            }),
            Err(RunError::Spawn { .. }) => Err(RunFailure::Missing),
            Err(RunError::Wait { .. } | RunError::TimedOut { .. }) => Err(RunFailure::TimedOut),
        }
    }

    fn watch(&self) -> Result<Box<dyn Watch>, RunFailure> {
        let mut child = Command::new(&self.path)
            .args(["watch", "--json"])
            .env("PATH", process::path_with_tools())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| RunFailure::Missing)?;
        let stdout = child.stdout.take().ok_or(RunFailure::Missing)?;
        let stderr = child.stderr.take();
        let stderr = thread::spawn(move || {
            let mut text = String::new();
            if let Some(mut pipe) = stderr {
                let _ = std::io::Read::read_to_string(&mut pipe, &mut text);
            }
            text
        });
        *self.watching.lock_unpoisoned() = Some(child);
        Ok(Box::new(CliWatch {
            lines: BufReader::new(stdout).lines(),
            stderr,
            watching: Arc::clone(&self.watching),
        }))
    }
}

struct CliWatch {
    lines: Lines<BufReader<ChildStdout>>,
    stderr: thread::JoinHandle<String>,
    watching: Arc<Mutex<Option<Child>>>,
}

impl Watch for CliWatch {
    fn next_line(&mut self) -> Option<String> {
        self.lines.next()?.ok()
    }

    fn end(self: Box<Self>) -> RawReply {
        let child = self.watching.lock_unpoisoned().take();
        let code = child
            .and_then(|mut child| child.wait().ok())
            .and_then(|status| status.code());
        RawReply {
            code,
            stdout: String::new(),
            stderr: self.stderr.join().unwrap_or_default(),
        }
    }
}

/// Ends a running `watch` (Daybook quits).
pub fn stop_watching(watching: &Mutex<Option<Child>>) {
    if let Some(mut child) = watching.lock_unpoisoned().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorKind;

    const IDLE: &str = r#"{"running":false,"recordingId":null,"projectId":null,"project":null,"client":null,"tagId":null,"tag":null,"note":null,"startedAt":null,"startedAtIso":null,"elapsedMs":null}"#;

    fn reply(code: Option<i32>, stdout: &str, stderr: &str) -> Result<RawReply, RunFailure> {
        Ok(RawReply {
            code,
            stdout: stdout.into(),
            stderr: stderr.into(),
        })
    }

    struct Fake {
        answer: Result<RawReply, RunFailure>,
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl Fake {
        fn answering(answer: Result<RawReply, RunFailure>) -> Self {
            Self {
                answer,
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl TimeTracker for Fake {
        fn run(&self, args: &[&str]) -> Result<RawReply, RunFailure> {
            self.calls
                .lock()
                .unwrap()
                .push(args.iter().map(|arg| arg.to_string()).collect());
            self.answer.clone()
        }

        fn watch(&self) -> Result<Box<dyn Watch>, RunFailure> {
            Err(RunFailure::Missing)
        }
    }

    fn error_of(result: AppResult<String>) -> (ErrorKind, String) {
        let err = result.unwrap_err();
        (err.kind(), err.to_string())
    }

    #[test]
    fn a_reply_with_exit_code_0_gives_its_stdout() {
        assert_eq!(
            reply_result(reply(Some(0), "{\"a\":1}\n", "")),
            Ok("{\"a\":1}\n".to_string())
        );
    }

    #[test]
    fn a_refusal_on_stdout_with_exit_code_1_is_refused_with_timetrackers_message() {
        let stdout =
            r#"{"kind":"invalid_input","message":"`note`: must be at most 500 characters"}"#;
        assert_eq!(
            error_of(reply_result(reply(Some(1), stdout, ""))),
            (
                ErrorKind::TimetrackerRefused,
                "`note`: must be at most 500 characters".to_string()
            )
        );
    }

    #[test]
    fn exit_code_1_without_a_refusal_falls_back_to_stderr_then_to_the_code() {
        assert_eq!(
            error_of(reply_result(reply(Some(1), "", "disk on fire\n"))),
            (ErrorKind::TimetrackerRefused, "disk on fire".to_string())
        );
        assert_eq!(
            error_of(reply_result(reply(Some(1), "", ""))),
            (
                ErrorKind::TimetrackerRefused,
                "timetracker failed with exit code 1".to_string()
            )
        );
    }

    #[test]
    fn exit_code_2_with_a_not_running_message_means_timetracker_is_not_running() {
        for stderr in [
            "TimeTracker is not running\n",
            "Could not reach the running TimeTracker: timed out\n",
        ] {
            assert_eq!(
                error_of(reply_result(reply(Some(2), "", stderr))),
                (
                    ErrorKind::TimetrackerNotRunning,
                    "TimeTracker isn't running.".to_string()
                ),
                "{stderr}"
            );
        }
    }

    #[test]
    fn a_clap_error_is_refused_without_the_error_prefix() {
        assert_eq!(
            error_of(reply_result(reply(
                Some(2),
                "",
                "error: unrecognized subcommand 'watch'\n\nUsage: timetracker [OPTIONS]\n"
            ))),
            (
                ErrorKind::TimetrackerRefused,
                "unrecognized subcommand 'watch'".to_string()
            )
        );
    }

    #[test]
    fn a_run_ended_by_a_signal_or_the_timeout_means_it_didnt_answer() {
        for result in [reply(None, "", ""), Err(RunFailure::TimedOut)] {
            assert_eq!(
                error_of(reply_result(result)),
                (
                    ErrorKind::TimetrackerNotRunning,
                    "TimeTracker didn't answer.".to_string()
                )
            );
        }
    }

    #[test]
    fn a_missing_tool_is_reported_as_missing() {
        assert_eq!(
            error_of(reply_result(Err(RunFailure::Missing))),
            (
                ErrorKind::TimetrackerMissing,
                "TimeTracker isn't installed (Settings → TimeTracker).".to_string()
            )
        );
    }

    #[test]
    fn data_changing_commands_are_told_from_reads_whatever_the_flags() {
        for (args, expected) in [
            (vec!["timer", "start", "p1", "--note=x", "--json"], true),
            (vec!["timer", "stop"], true),
            (vec!["--json", "timer", "stop"], true),
            (vec!["record", "p1", "--json"], true),
            (vec!["status", "--json"], false),
            (vec!["projects", "--archived", "--json"], false),
            (vec!["clients"], false),
            (vec!["tags"], false),
            (vec!["watch", "--json"], false),
            (vec!["show"], false),
            (vec!["show", "2026-10-26", "--json"], false),
            (vec!["focus"], false),
            (vec!["timer", "status", "--json"], false),
            (vec!["--json", "timer", "recent", "--limit", "8"], false),
        ] {
            assert_eq!(changes_data(&args), expected, "{args:?}");
        }
    }

    #[test]
    fn the_timer_is_read_with_timer_status() {
        let tt = Fake::answering(reply(Some(0), IDLE, ""));
        assert_eq!(timer_status(&tt), Ok(None));
        assert_eq!(tt.calls(), vec![vec!["timer", "status", "--json"]]);
    }

    #[test]
    fn projects_are_listed_with_the_archived_ones() {
        let tt = Fake::answering(reply(Some(0), "[]", ""));
        assert_eq!(projects(&tt), Ok(vec![]));
        assert_eq!(tt.calls(), vec![vec!["projects", "--archived", "--json"]]);
    }

    #[test]
    fn the_version_comes_from_status() {
        let tt = Fake::answering(reply(Some(0), r#"{"version":"0.1.0"}"#, ""));
        assert_eq!(status(&tt).unwrap().version, "0.1.0");
        assert_eq!(tt.calls(), vec![vec!["status", "--json"]]);
    }

    #[test]
    fn unreadable_output_is_refused_naming_what_failed() {
        let tt = Fake::answering(reply(Some(0), "oops", ""));
        let err = timer_status(&tt).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::TimetrackerRefused);
        assert!(
            err.to_string()
                .starts_with("TimeTracker's timer couldn't be read")
        );
    }

    #[test]
    fn show_opens_today_or_the_given_day() {
        let tt = Fake::answering(reply(Some(0), "", ""));
        show(&tt, None).unwrap();
        show(&tt, NaiveDate::from_ymd_opt(2026, 10, 26)).unwrap();
        assert_eq!(
            tt.calls(),
            vec![vec!["show", "--json"], vec!["show", "2026-10-26", "--json"]]
        );
    }

    #[test]
    fn focus_brings_timetracker_to_the_front() {
        let tt = Fake::answering(reply(Some(0), "", ""));
        focus(&tt).unwrap();
        assert_eq!(tt.calls(), vec![vec!["focus", "--json"]]);
    }

    #[test]
    fn a_failing_call_gives_the_error_of_its_reply() {
        let tt = Fake::answering(reply(Some(2), "", "TimeTracker is not running\n"));
        assert_eq!(
            timer_status(&tt).unwrap_err().kind(),
            ErrorKind::TimetrackerNotRunning
        );
        assert_eq!(
            show(&tt, None).unwrap_err().kind(),
            ErrorKind::TimetrackerNotRunning
        );
    }

    #[test]
    fn an_empty_path_is_the_default_tool() {
        let home = std::env::home_dir().unwrap();
        for path in ["", "   "] {
            assert_eq!(
                resolve_path(path),
                home.join(".local/bin/timetracker"),
                "{path:?}"
            );
        }
    }

    #[test]
    fn a_path_starting_with_tilde_slash_is_under_the_home_folder() {
        let home = std::env::home_dir().unwrap();
        assert_eq!(resolve_path(" ~/bin/tt "), home.join("bin/tt"));
    }

    #[test]
    fn an_absolute_path_is_kept() {
        assert_eq!(
            resolve_path("/opt/tt/timetracker"),
            PathBuf::from("/opt/tt/timetracker")
        );
    }

    fn cli_at_nothing() -> Cli {
        Cli::new(
            PathBuf::from("/nonexistent/daybook-test/timetracker"),
            Arc::new(Mutex::new(None)),
        )
    }

    #[test]
    fn a_tool_that_is_not_there_is_missing_for_reads_and_watch() {
        let cli = cli_at_nothing();
        assert_eq!(cli.run(&["status", "--json"]), Err(RunFailure::Missing));
        assert!(matches!(cli.watch(), Err(RunFailure::Missing)));
    }

    #[cfg(debug_assertions)]
    #[test]
    fn a_debug_build_answers_writes_without_running_the_tool() {
        let cli = cli_at_nothing();
        assert_eq!(
            cli.run(&["timer", "start", "p1", "--note=x", "--json"]),
            Ok(RawReply {
                code: Some(0),
                stdout: String::new(),
                stderr: String::new(),
            })
        );
    }
}
