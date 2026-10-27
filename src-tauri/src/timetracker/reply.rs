//! TimeTracker's `--json` replies and `watch` lines as typed values. Pure.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The running timer; TimeTracker has none while `running` is false.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct TimerState {
    pub recording_id: String,
    /// TimeTracker's project id.
    pub project_id: String,
    /// TimeTracker's project name.
    pub project: String,
    pub client: Option<String>,
    pub tag_id: Option<String>,
    pub tag: Option<String>,
    /// `None` when the timer has no note.
    pub note: Option<String>,
    /// Ms since the epoch: the open span's start.
    pub started_at: i64,
}

/// `timer status --json` as TimeTracker prints it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimerReply {
    running: bool,
    recording_id: Option<String>,
    project_id: Option<String>,
    project: Option<String>,
    client: Option<String>,
    tag_id: Option<String>,
    tag: Option<String>,
    note: Option<String>,
    started_at: Option<i64>,
}

impl TimerReply {
    fn into_timer(self) -> Option<TimerState> {
        if !self.running {
            return None;
        }
        Some(TimerState {
            recording_id: self.recording_id.unwrap_or_default(),
            project_id: self.project_id?,
            project: self.project.unwrap_or_default(),
            client: self.client.filter(|client| !client.is_empty()),
            tag_id: self.tag_id,
            tag: self.tag,
            note: self.note.filter(|note| !note.trim().is_empty()),
            started_at: self.started_at.unwrap_or_default(),
        })
    }
}

/// A TimeTracker project from `projects --archived --json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct TtProject {
    pub id: String,
    pub name: String,
    pub client_id: Option<String>,
    pub client: Option<String>,
    /// An accent name, e.g. `blue`.
    pub color: String,
    /// 0 to 8, darkest first.
    pub shade: u8,
    #[serde(default)]
    pub billable: bool,
    #[serde(default)]
    pub archived: bool,
}

/// A timer used lately, from `timer recent --json`, newest first.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RecentTimer {
    pub project_id: String,
    pub project: String,
    pub tag: Option<String>,
    /// Empty without a note.
    #[serde(default)]
    pub note: String,
}

/// The part of `status --json` Daybook reads.
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct TtStatus {
    pub version: String,
}

/// The part of `record --json`'s reply Daybook keeps.
#[derive(Deserialize)]
struct Recorded {
    id: String,
}

/// A refusal printed on stdout with exit code 1.
#[derive(Deserialize)]
pub struct Refusal {
    pub message: String,
}

/// What changed in TimeTracker's data, from a `data` line.
#[derive(Clone, Debug, PartialEq)]
pub enum DataKind {
    Projects,
    Recordings,
    Other,
}

/// A line of `watch --json` Daybook acts on.
#[derive(Clone, Debug, PartialEq)]
pub enum WatchLine {
    /// The timer now; `None` when none runs.
    Timer(Option<TimerState>),
    /// The activity tracker changed; Daybook ignores it.
    Tracker,
    Data(DataKind),
}

#[derive(Deserialize)]
struct Line {
    event: String,
    #[serde(default)]
    data: serde_json::Value,
}

#[derive(Deserialize)]
struct DataChange {
    kind: String,
}

/// `null` prints nothing, so an empty reply is `None`.
pub fn parse_timer(stdout: &str) -> Result<Option<TimerState>, String> {
    if stdout.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str::<TimerReply>(stdout)
        .map(TimerReply::into_timer)
        .map_err(|e| format!("TimeTracker's timer couldn't be read: {e}"))
}

pub fn parse_projects(stdout: &str) -> Result<Vec<TtProject>, String> {
    serde_json::from_str(stdout)
        .map_err(|e| format!("TimeTracker's projects couldn't be read: {e}"))
}

pub fn parse_recent(stdout: &str) -> Result<Vec<RecentTimer>, String> {
    if stdout.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(stdout)
        .map_err(|e| format!("TimeTracker's recent timers couldn't be read: {e}"))
}

pub fn parse_status(stdout: &str) -> Result<TtStatus, String> {
    serde_json::from_str(stdout).map_err(|e| format!("TimeTracker's status couldn't be read: {e}"))
}

/// The stored suggestion's id; `None` for an empty reply (a debug build's dry run).
pub fn parse_recorded(stdout: &str) -> Result<Option<String>, String> {
    if stdout.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str::<Recorded>(stdout)
        .map(|recorded| Some(recorded.id))
        .map_err(|e| format!("TimeTracker's suggestion couldn't be read: {e}"))
}

/// TimeTracker's message in a refusal, if stdout holds one.
pub fn parse_refusal(stdout: &str) -> Option<String> {
    serde_json::from_str::<Refusal>(stdout.trim())
        .ok()
        .map(|refusal| refusal.message)
        .filter(|message| !message.trim().is_empty())
}

/// `None` for unknown events and lines that don't parse: TimeTracker may add more.
pub fn parse_watch_line(line: &str) -> Option<WatchLine> {
    let line: Line = serde_json::from_str(line.trim()).ok()?;
    match line.event.as_str() {
        "timer" => serde_json::from_value::<TimerReply>(line.data)
            .ok()
            .map(|reply| WatchLine::Timer(reply.into_timer())),
        "tracker" => Some(WatchLine::Tracker),
        "data" => {
            let change: DataChange = serde_json::from_value(line.data).ok()?;
            Some(WatchLine::Data(match change.kind.as_str() {
                "projects" => DataKind::Projects,
                "recordings" => DataKind::Recordings,
                _ => DataKind::Other,
            }))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const IDLE: &str = r#"{"running":false,"recordingId":null,"projectId":null,"project":null,"client":null,"tagId":null,"tag":null,"note":null,"startedAt":null,"startedAtIso":null,"elapsedMs":null}"#;

    const RUNNING: &str = r#"{"running":true,"recordingId":"33333333-3333-4333-8333-333333333333","projectId":"55555555-5555-4555-8555-555555555555","project":"SDK v2","client":"Oscorp","tagId":"22222222-2222-4222-8222-222222222222","tag":"Development","note":"QRS-7064: Contact Form","startedAt":1793091300000,"startedAtIso":"2026-10-27T09:55:00+01:00","elapsedMs":1500000}"#;

    fn running_timer() -> TimerState {
        TimerState {
            recording_id: "33333333-3333-4333-8333-333333333333".into(),
            project_id: "55555555-5555-4555-8555-555555555555".into(),
            project: "SDK v2".into(),
            client: Some("Oscorp".into()),
            tag_id: Some("22222222-2222-4222-8222-222222222222".into()),
            tag: Some("Development".into()),
            note: Some("QRS-7064: Contact Form".into()),
            started_at: 1_793_091_300_000,
        }
    }

    fn watch_line(event: &str, data: &str) -> String {
        format!(r#"{{"event":"{event}","data":{data}}}"#)
    }

    #[test]
    fn an_idle_timer_is_no_timer() {
        assert_eq!(parse_timer(IDLE), Ok(None));
    }

    #[test]
    fn an_empty_reply_is_no_timer() {
        for stdout in ["", "\n", "  "] {
            assert_eq!(parse_timer(stdout), Ok(None), "{stdout:?}");
        }
    }

    #[test]
    fn a_running_timer_carries_its_project_tag_note_and_start() {
        assert_eq!(parse_timer(RUNNING), Ok(Some(running_timer())));
    }

    #[test]
    fn a_blank_note_is_no_note() {
        for note in [r#""""#, r#""  ""#, "null"] {
            let stdout = RUNNING.replace(r#""QRS-7064: Contact Form""#, note);
            let timer = parse_timer(&stdout).unwrap().unwrap();
            assert_eq!(timer.note, None, "{note}");
        }
    }

    #[test]
    fn an_empty_client_is_no_client() {
        let stdout = RUNNING.replace(r#""client":"Oscorp""#, r#""client":"""#);
        assert_eq!(parse_timer(&stdout).unwrap().unwrap().client, None);
    }

    #[test]
    fn a_running_timer_without_a_project_is_no_timer() {
        let stdout = RUNNING.replace(
            r#""projectId":"55555555-5555-4555-8555-555555555555""#,
            r#""projectId":null"#,
        );
        assert_eq!(parse_timer(&stdout), Ok(None));
    }

    #[test]
    fn an_unreadable_timer_is_an_error_naming_the_timer() {
        let err = parse_timer("not json").unwrap_err();
        assert!(
            err.starts_with("TimeTracker's timer couldn't be read"),
            "{err}"
        );
    }

    #[test]
    fn the_timer_serializes_with_camel_case_names() {
        assert_eq!(
            serde_json::to_value(running_timer()).unwrap(),
            serde_json::json!({
                "recordingId": "33333333-3333-4333-8333-333333333333",
                "projectId": "55555555-5555-4555-8555-555555555555",
                "project": "SDK v2",
                "client": "Oscorp",
                "tagId": "22222222-2222-4222-8222-222222222222",
                "tag": "Development",
                "note": "QRS-7064: Contact Form",
                "startedAt": 1_793_091_300_000_i64,
            })
        );
    }

    #[test]
    fn projects_are_read_with_their_client_colour_and_flags() {
        let stdout = r#"[{"id":"b3a1b7a1","name":"Apollo","clientId":"83a9cec6","client":"Oscorp","color":"blue","shade":1,"billable":true,"archived":false},{"id":"fee10958","name":"Solo","clientId":null,"client":null,"color":"teal","shade":4,"billable":false,"archived":true}]"#;
        assert_eq!(
            parse_projects(stdout),
            Ok(vec![
                TtProject {
                    id: "b3a1b7a1".into(),
                    name: "Apollo".into(),
                    client_id: Some("83a9cec6".into()),
                    client: Some("Oscorp".into()),
                    color: "blue".into(),
                    shade: 1,
                    billable: true,
                    archived: false,
                },
                TtProject {
                    id: "fee10958".into(),
                    name: "Solo".into(),
                    client_id: None,
                    client: None,
                    color: "teal".into(),
                    shade: 4,
                    billable: false,
                    archived: true,
                },
            ])
        );
    }

    #[test]
    fn projects_without_flags_are_active_and_not_billable() {
        let stdout =
            r#"[{"id":"a","name":"A","clientId":null,"client":null,"color":"blue","shade":0}]"#;
        let project = parse_projects(stdout).unwrap().remove(0);
        assert!(!project.archived);
        assert!(!project.billable);
    }

    #[test]
    fn no_projects_is_an_empty_list() {
        assert_eq!(parse_projects("[]"), Ok(vec![]));
    }

    #[test]
    fn unreadable_projects_are_an_error_naming_the_projects() {
        let err = parse_projects("").unwrap_err();
        assert!(
            err.starts_with("TimeTracker's projects couldn't be read"),
            "{err}"
        );
    }

    const RECENT: &str = r#"[{"projectId":"11111111-1111-4111-8111-111111111111","project":"Internal","client":"Nexo","tagId":null,"tag":null,"note":"testing","lastUsedAt":1792824000000},{"projectId":"11111111-1111-4111-8111-111111111111","project":"Internal","client":"Nexo","tagId":"44444444-4444-4444-8444-444444444444","tag":"Admin","note":"Time recording","lastUsedAt":1792790100000},{"projectId":"55555555-5555-4555-8555-555555555555","project":"SDK v2","client":"Oscorp","tagId":"22222222-2222-4222-8222-222222222222","tag":"Development","note":"QRS-7064: Contact Form","lastUsedAt":1792766700000}]"#;

    #[test]
    fn recent_timers_are_read_newest_first_with_project_tag_and_note() {
        let timer = |project_id: &str, project: &str, tag: Option<&str>, note: &str| RecentTimer {
            project_id: project_id.into(),
            project: project.into(),
            tag: tag.map(String::from),
            note: note.into(),
        };
        assert_eq!(
            parse_recent(RECENT),
            Ok(vec![
                timer(
                    "11111111-1111-4111-8111-111111111111",
                    "Internal",
                    None,
                    "testing"
                ),
                timer(
                    "11111111-1111-4111-8111-111111111111",
                    "Internal",
                    Some("Admin"),
                    "Time recording"
                ),
                timer(
                    "55555555-5555-4555-8555-555555555555",
                    "SDK v2",
                    Some("Development"),
                    "QRS-7064: Contact Form"
                ),
            ])
        );
    }

    #[test]
    fn no_recent_timers_is_an_empty_list_whether_printed_or_not() {
        for stdout in ["", "\n", "[]"] {
            assert_eq!(parse_recent(stdout), Ok(vec![]), "{stdout:?}");
        }
    }

    #[test]
    fn a_recent_timer_without_a_note_is_read() {
        let stdout = r#"[{"projectId":"p","project":"P","client":null,"tagId":null,"tag":null,"note":"","lastUsedAt":1}]"#;
        let timer = parse_recent(stdout).unwrap().remove(0);
        assert_eq!(timer.note, "");
    }

    #[test]
    fn unreadable_recent_timers_are_an_error_naming_the_recent_timers() {
        let err = parse_recent("oops").unwrap_err();
        assert!(
            err.starts_with("TimeTracker's recent timers couldn't be read"),
            "{err}"
        );
    }

    #[test]
    fn the_status_gives_the_version_and_ignores_the_rest() {
        let stdout = format!(
            r#"{{"version":"0.1.0","tracker":{{"state":{{"kind":"running"}},"text":"Tracking · Slack"}},"timer":{IDLE},"billable":{{"recordedMs":0,"billableMs":0,"percent":null}},"windowVisible":true}}"#
        );
        assert_eq!(
            parse_status(&stdout),
            Ok(TtStatus {
                version: "0.1.0".into()
            })
        );
    }

    #[test]
    fn a_status_without_a_version_is_an_error_naming_the_status() {
        let err = parse_status("{}").unwrap_err();
        assert!(
            err.starts_with("TimeTracker's status couldn't be read"),
            "{err}"
        );
    }

    #[test]
    fn a_recorded_suggestion_gives_its_id_and_ignores_the_rest() {
        let stdout = r#"{"kind":"suggestion","id":"5e1c-77","projectId":"p1","note":"Zoom - Oscorp Weekly","tagId":null,"source":"Daybook","day":"2026-10-27","spans":[],"projectName":"Internal"}"#;
        assert_eq!(parse_recorded(stdout), Ok(Some("5e1c-77".to_string())));
    }

    #[test]
    fn an_empty_record_reply_gives_no_id() {
        for stdout in ["", "\n", "  "] {
            assert_eq!(parse_recorded(stdout), Ok(None), "{stdout:?}");
        }
    }

    #[test]
    fn an_unreadable_record_reply_is_an_error_naming_the_suggestion() {
        for stdout in ["oops", "{}", r#"{"id":5}"#] {
            let err = parse_recorded(stdout).unwrap_err();
            assert!(
                err.starts_with("TimeTracker's suggestion couldn't be read"),
                "{stdout}: {err}"
            );
        }
    }

    #[test]
    fn a_refusal_gives_timetrackers_message_verbatim() {
        let stdout = "{\"kind\":\"not_found\",\"message\":\"No project matches \\\"x\\\"\"}\n";
        assert_eq!(
            parse_refusal(stdout),
            Some("No project matches \"x\"".into())
        );
    }

    #[test]
    fn stdout_that_is_no_refusal_gives_no_message() {
        for stdout in [
            "",
            "plain text",
            "{}",
            r#"{"kind":"io"}"#,
            r#"{"kind":"io","message":"  "}"#,
        ] {
            assert_eq!(parse_refusal(stdout), None, "{stdout:?}");
        }
    }

    #[test]
    fn a_timer_line_gives_the_timer() {
        assert_eq!(
            parse_watch_line(&watch_line("timer", RUNNING)),
            Some(WatchLine::Timer(Some(running_timer())))
        );
    }

    #[test]
    fn an_idle_timer_line_gives_no_timer() {
        assert_eq!(
            parse_watch_line(&watch_line("timer", IDLE)),
            Some(WatchLine::Timer(None))
        );
    }

    #[test]
    fn a_tracker_line_is_noted_without_its_data() {
        let line = watch_line(
            "tracker",
            r#"{"state":{"kind":"running"},"current":null,"permission":"granted","text":"Tracking · Slack"}"#,
        );
        assert_eq!(parse_watch_line(&line), Some(WatchLine::Tracker));
    }

    #[test]
    fn data_lines_tell_what_kind_of_data_changed() {
        for (kind, expected) in [
            ("projects", DataKind::Projects),
            ("recordings", DataKind::Recordings),
            ("activity", DataKind::Other),
            ("tags", DataKind::Other),
            ("rules", DataKind::Other),
            ("settings", DataKind::Other),
            ("something-new", DataKind::Other),
        ] {
            let line = watch_line("data", &format!(r#"{{"kind":"{kind}","day":null}}"#));
            assert_eq!(
                parse_watch_line(&line),
                Some(WatchLine::Data(expected)),
                "{kind}"
            );
        }
    }

    #[test]
    fn a_data_line_with_a_day_is_read() {
        let line = watch_line("data", r#"{"kind":"recordings","day":"2026-10-27"}"#);
        assert_eq!(
            parse_watch_line(&line),
            Some(WatchLine::Data(DataKind::Recordings))
        );
    }

    #[test]
    fn a_line_with_surrounding_whitespace_is_read() {
        let line = format!("  {}\n", watch_line("timer", IDLE));
        assert_eq!(parse_watch_line(&line), Some(WatchLine::Timer(None)));
    }

    #[test]
    fn unknown_events_and_garbage_lines_are_skipped() {
        for line in [
            r#"{"event":"newthing","data":{}}"#.to_string(),
            "garbage".to_string(),
            String::new(),
            r#"{"data":{}}"#.to_string(),
            watch_line("timer", r#""not a timer""#),
            watch_line("data", "{}"),
        ] {
            assert_eq!(parse_watch_line(&line), None, "{line:?}");
        }
    }
}
