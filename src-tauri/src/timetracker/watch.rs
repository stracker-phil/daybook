//! Following `timetracker watch --json`: what a connection reports, how it ended and when to try
//! again. An older TimeTracker without `watch` is polled instead.

use std::time::Duration;

use serde::Serialize;
use ts_rs::TS;

use super::client::{RawReply, RunFailure, TimeTracker, state_of, timer_status};
use super::reply::{DataChange, TimerState, WatchLine, parse_watch_line};

/// Whether Daybook can reach TimeTracker.
#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum TtState {
    Available,
    /// Also before the first answer.
    #[default]
    NotRunning,
    /// The `timetracker` tool isn't there.
    Missing,
}

/// TimeTracker's state and its timer; payload of `timer_changed`.
#[derive(Serialize, Clone, Debug, Default, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "TimerChanged")]
#[serde(rename_all = "camelCase")]
pub struct Tracking {
    pub tt: TtState,
    /// Only while TimeTracker is available.
    pub timer: Option<TimerState>,
}

impl Tracking {
    pub fn gone(tt: TtState) -> Self {
        Self { tt, timer: None }
    }
}

/// How Daybook follows TimeTracker now, for Settings → Status.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WatchLink {
    /// No `watch` connected: TimeTracker doesn't run, or Daybook waits to try again.
    #[default]
    Waiting,
    /// A `watch` sends TimeTracker's changes.
    Connected,
    /// An older TimeTracker without `watch`, read every `POLL`.
    Polling,
}

/// Polling an older TimeTracker.
pub const POLL: Duration = Duration::from_secs(30);
const MISSING_RETRY: Duration = Duration::from_secs(60);
const AFTER_LINES: Duration = Duration::from_secs(1);
const MAX_BACKOFF_SECS: u64 = 30;

/// How a `watch` ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WatchEnd {
    Missing,
    /// Not running, quit, or the connection was lost.
    NotRunning,
    /// This TimeTracker has no `watch` (or refused it).
    Unsupported,
}

/// What a `watch` reply means; clap's `error: …` (an unknown subcommand) means no `watch`.
pub fn watch_end(reply: Result<RawReply, RunFailure>) -> WatchEnd {
    match reply {
        Err(RunFailure::Missing) => WatchEnd::Missing,
        Err(RunFailure::TimedOut) => WatchEnd::NotRunning,
        Ok(reply) => match reply.code {
            Some(1) => WatchEnd::Unsupported,
            Some(2) if reply.stderr.trim_start().starts_with("error:") => WatchEnd::Unsupported,
            _ => WatchEnd::NotRunning,
        },
    }
}

/// What a connection's end leaves Daybook with; an unsupported `watch` is polled instead.
pub fn state_after(end: WatchEnd) -> Option<Tracking> {
    match end {
        WatchEnd::Missing => Some(Tracking::gone(TtState::Missing)),
        WatchEnd::NotRunning => Some(Tracking::gone(TtState::NotRunning)),
        WatchEnd::Unsupported => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Next {
    /// Start `watch` again after this long.
    Retry(Duration),
    /// Poll `timer status` every `POLL`.
    Poll,
}

/// Backoff while TimeTracker isn't running: 2, 4, 8, 16, then every 30 s.
#[derive(Debug, Default)]
pub struct Reconnect {
    failures: u32,
}

impl Reconnect {
    /// When to try again after a connection that ended with `end`; `had_lines` resets the backoff.
    pub fn after(&mut self, end: WatchEnd, had_lines: bool) -> Next {
        if had_lines {
            self.failures = 0;
            return Next::Retry(AFTER_LINES);
        }
        match end {
            WatchEnd::Unsupported => Next::Poll,
            WatchEnd::Missing => {
                self.failures = 0;
                Next::Retry(MISSING_RETRY)
            }
            WatchEnd::NotRunning => {
                self.failures = self.failures.saturating_add(1);
                let secs = 2u64.saturating_pow(self.failures).min(MAX_BACKOFF_SECS);
                Next::Retry(Duration::from_secs(secs))
            }
        }
    }
}

/// Where a connection's news goes.
pub trait Sink {
    /// The first line arrived.
    fn connected(&self);
    fn tracking(&self, tracking: Tracking);
    fn data_changed(&self, change: DataChange);
}

/// Follows one `watch` until it ends; whether any line came.
pub fn follow(tt: &dyn TimeTracker, sink: &dyn Sink) -> (WatchEnd, bool) {
    let mut watch = match tt.watch() {
        Ok(watch) => watch,
        Err(failure) => return (watch_end(Err(failure)), false),
    };
    let mut had_lines = false;
    while let Some(line) = watch.next_line() {
        let Some(line) = parse_watch_line(&line) else {
            continue;
        };
        if !had_lines {
            had_lines = true;
            sink.connected();
        }
        match line {
            WatchLine::Timer(timer) => sink.tracking(Tracking {
                tt: TtState::Available,
                timer,
            }),
            WatchLine::Data(change) => sink.data_changed(change),
            WatchLine::Tracker => {}
        }
    }
    (watch_end(Ok(watch.end())), had_lines)
}

/// One look at the timer, for polling and after a project change.
pub fn poll(tt: &dyn TimeTracker) -> Tracking {
    match timer_status(tt) {
        Ok(timer) => Tracking {
            tt: TtState::Available,
            timer,
        },
        Err(e) => Tracking::gone(state_of(&e)),
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::sync::Mutex;

    use super::super::client::Watch;
    use super::super::reply::DataKind;
    use super::*;

    const IDLE: &str = r#"{"running":false,"recordingId":null,"projectId":null,"project":null,"client":null,"tagId":null,"tag":null,"note":null,"startedAt":null,"startedAtIso":null,"elapsedMs":null}"#;

    const RUNNING: &str = r#"{"running":true,"recordingId":"r1","projectId":"p1","project":"SDK v2","client":"Oscorp","tagId":null,"tag":null,"note":"review","startedAt":1793091300000,"startedAtIso":"2026-10-27T09:55:00+01:00","elapsedMs":1500000}"#;

    fn running_timer() -> TimerState {
        TimerState {
            recording_id: "r1".into(),
            project_id: "p1".into(),
            project: "SDK v2".into(),
            client: Some("Oscorp".into()),
            tag_id: None,
            tag: None,
            note: Some("review".into()),
            started_at: 1_793_091_300_000,
        }
    }

    fn reply(code: Option<i32>, stderr: &str) -> RawReply {
        RawReply {
            code,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }

    fn line(event: &str, data: &str) -> String {
        format!(r#"{{"event":"{event}","data":{data}}}"#)
    }

    struct FakeWatch {
        lines: VecDeque<String>,
        end: RawReply,
    }

    impl Watch for FakeWatch {
        fn next_line(&mut self) -> Option<String> {
            self.lines.pop_front()
        }

        fn end(self: Box<Self>) -> RawReply {
            self.end
        }
    }

    struct FakeTt {
        watch: Mutex<Option<Result<Box<dyn Watch>, RunFailure>>>,
        run: Result<RawReply, RunFailure>,
    }

    impl FakeTt {
        fn watching(lines: &[String], end: RawReply) -> Self {
            let watch = FakeWatch {
                lines: lines.iter().cloned().collect(),
                end,
            };
            Self {
                watch: Mutex::new(Some(Ok(Box::new(watch)))),
                run: Err(RunFailure::Missing),
            }
        }

        fn failing_to_watch(failure: RunFailure) -> Self {
            Self {
                watch: Mutex::new(Some(Err(failure))),
                run: Err(RunFailure::Missing),
            }
        }

        fn answering(run: Result<RawReply, RunFailure>) -> Self {
            Self {
                watch: Mutex::new(None),
                run,
            }
        }
    }

    impl TimeTracker for FakeTt {
        fn run(&self, _args: &[&str]) -> Result<RawReply, RunFailure> {
            self.run.clone()
        }

        fn watch(&self) -> Result<Box<dyn Watch>, RunFailure> {
            self.watch.lock().unwrap().take().expect("watched once")
        }
    }

    #[derive(Debug, PartialEq)]
    enum Heard {
        Connected,
        Tracking(Tracking),
        Data(DataChange),
    }

    #[derive(Default)]
    struct FakeSink(RefCell<Vec<Heard>>);

    impl Sink for FakeSink {
        fn connected(&self) {
            self.0.borrow_mut().push(Heard::Connected);
        }

        fn tracking(&self, tracking: Tracking) {
            self.0.borrow_mut().push(Heard::Tracking(tracking));
        }

        fn data_changed(&self, change: DataChange) {
            self.0.borrow_mut().push(Heard::Data(change));
        }
    }

    #[test]
    fn a_watch_end_is_told_from_the_reply() {
        for (name, result, expected) in [
            ("missing", Err(RunFailure::Missing), WatchEnd::Missing),
            ("timeout", Err(RunFailure::TimedOut), WatchEnd::NotRunning),
            (
                "not running",
                Ok(reply(Some(2), "TimeTracker is not running\n")),
                WatchEnd::NotRunning,
            ),
            (
                "quit",
                Ok(reply(Some(2), "TimeTracker quit\n")),
                WatchEnd::NotRunning,
            ),
            (
                "connection lost",
                Ok(reply(Some(2), "Lost the connection to TimeTracker: eof\n")),
                WatchEnd::NotRunning,
            ),
            (
                "unknown subcommand",
                Ok(reply(Some(2), "error: unrecognized subcommand 'watch'\n")),
                WatchEnd::Unsupported,
            ),
            ("refused", Ok(reply(Some(1), "")), WatchEnd::Unsupported),
            ("signal", Ok(reply(None, "")), WatchEnd::NotRunning),
        ] {
            assert_eq!(watch_end(result), expected, "{name}");
        }
    }

    #[test]
    fn a_missing_tool_and_a_quit_leave_their_states_and_an_unsupported_watch_is_polled() {
        assert_eq!(
            state_after(WatchEnd::Missing),
            Some(Tracking::gone(TtState::Missing))
        );
        assert_eq!(
            state_after(WatchEnd::NotRunning),
            Some(Tracking::gone(TtState::NotRunning))
        );
        assert_eq!(state_after(WatchEnd::Unsupported), None);
    }

    #[test]
    fn the_wait_while_timetracker_is_not_running_doubles_up_to_30_seconds() {
        let mut reconnect = Reconnect::default();
        let waits: Vec<Next> = (0..6)
            .map(|_| reconnect.after(WatchEnd::NotRunning, false))
            .collect();
        assert_eq!(
            waits,
            [2, 4, 8, 16, 30, 30].map(|secs| Next::Retry(Duration::from_secs(secs)))
        );
    }

    #[test]
    fn a_connection_that_had_lines_is_retried_after_a_second_and_the_backoff_starts_over() {
        let mut reconnect = Reconnect::default();
        for _ in 0..3 {
            reconnect.after(WatchEnd::NotRunning, false);
        }
        assert_eq!(
            reconnect.after(WatchEnd::NotRunning, true),
            Next::Retry(Duration::from_secs(1))
        );
        assert_eq!(
            reconnect.after(WatchEnd::NotRunning, false),
            Next::Retry(Duration::from_secs(2))
        );
    }

    #[test]
    fn a_missing_tool_is_retried_after_a_minute_and_resets_the_backoff() {
        let mut reconnect = Reconnect::default();
        reconnect.after(WatchEnd::NotRunning, false);
        reconnect.after(WatchEnd::NotRunning, false);
        assert_eq!(
            reconnect.after(WatchEnd::Missing, false),
            Next::Retry(Duration::from_secs(60))
        );
        assert_eq!(
            reconnect.after(WatchEnd::NotRunning, false),
            Next::Retry(Duration::from_secs(2))
        );
    }

    #[test]
    fn an_unsupported_watch_switches_to_polling() {
        assert_eq!(
            Reconnect::default().after(WatchEnd::Unsupported, false),
            Next::Poll
        );
    }

    #[test]
    fn following_reports_the_first_line_once_then_the_timer_and_data_changes() {
        let lines = [
            line("timer", IDLE),
            line(
                "tracker",
                r#"{"state":{"kind":"running"},"text":"Tracking"}"#,
            ),
            line("data", r#"{"kind":"projects","day":null}"#),
            line("timer", RUNNING),
            line("data", r#"{"kind":"recordings","day":"2026-10-27"}"#),
        ];
        let tt = FakeTt::watching(&lines, reply(Some(2), "TimeTracker quit\n"));
        let sink = FakeSink::default();

        let ended = follow(&tt, &sink);

        assert_eq!(ended, (WatchEnd::NotRunning, true));
        assert_eq!(
            sink.0.into_inner(),
            vec![
                Heard::Connected,
                Heard::Tracking(Tracking {
                    tt: TtState::Available,
                    timer: None
                }),
                Heard::Data(DataChange {
                    kind: DataKind::Projects,
                    day: None
                }),
                Heard::Tracking(Tracking {
                    tt: TtState::Available,
                    timer: Some(running_timer())
                }),
                Heard::Data(DataChange {
                    kind: DataKind::Recordings,
                    day: chrono::NaiveDate::from_ymd_opt(2026, 10, 27)
                }),
            ]
        );
    }

    #[test]
    fn garbage_and_unknown_lines_are_skipped_and_do_not_count_as_a_connection() {
        let lines = ["garbage".to_string(), line("newthing", "{}"), String::new()];
        let tt = FakeTt::watching(&lines, reply(Some(2), "TimeTracker is not running\n"));
        let sink = FakeSink::default();

        assert_eq!(follow(&tt, &sink), (WatchEnd::NotRunning, false));
        assert!(sink.0.into_inner().is_empty());
    }

    #[test]
    fn following_an_older_timetracker_ends_unsupported_without_lines() {
        let tt = FakeTt::watching(
            &[],
            reply(Some(2), "error: unrecognized subcommand 'watch'\n"),
        );
        assert_eq!(
            follow(&tt, &FakeSink::default()),
            (WatchEnd::Unsupported, false)
        );
    }

    #[test]
    fn a_watch_that_cannot_start_ends_without_lines() {
        for (failure, expected) in [
            (RunFailure::Missing, WatchEnd::Missing),
            (RunFailure::TimedOut, WatchEnd::NotRunning),
        ] {
            let tt = FakeTt::failing_to_watch(failure.clone());
            let sink = FakeSink::default();
            assert_eq!(follow(&tt, &sink), (expected, false), "{failure:?}");
            assert!(sink.0.into_inner().is_empty());
        }
    }

    #[test]
    fn polling_an_available_timetracker_gives_its_timer() {
        let running = FakeTt::answering(Ok(RawReply {
            code: Some(0),
            stdout: RUNNING.into(),
            stderr: String::new(),
        }));
        assert_eq!(
            poll(&running),
            Tracking {
                tt: TtState::Available,
                timer: Some(running_timer())
            }
        );

        let idle = FakeTt::answering(Ok(RawReply {
            code: Some(0),
            stdout: IDLE.into(),
            stderr: String::new(),
        }));
        assert_eq!(
            poll(&idle),
            Tracking {
                tt: TtState::Available,
                timer: None
            }
        );
    }

    #[test]
    fn polling_when_timetracker_is_not_running_or_missing_gives_no_timer() {
        let not_running = FakeTt::answering(Ok(reply(Some(2), "TimeTracker is not running\n")));
        assert_eq!(poll(&not_running), Tracking::gone(TtState::NotRunning));

        let hung = FakeTt::answering(Err(RunFailure::TimedOut));
        assert_eq!(poll(&hung), Tracking::gone(TtState::NotRunning));

        let missing = FakeTt::answering(Err(RunFailure::Missing));
        assert_eq!(poll(&missing), Tracking::gone(TtState::Missing));
    }

    #[test]
    fn the_tracking_serializes_with_camel_case_names() {
        assert_eq!(
            serde_json::to_value(Tracking::gone(TtState::NotRunning)).unwrap(),
            serde_json::json!({ "tt": "notRunning", "timer": null })
        );
    }
}
