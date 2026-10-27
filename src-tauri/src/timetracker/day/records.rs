//! `recordings <day> --json` and `suggestions <day> --json` as typed values. Pure.

use serde::Deserialize;

/// One stretch of a recording or suggestion; times in ms since the epoch.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub start: i64,
    /// `None` while the timer runs on it.
    pub end: Option<i64>,
    /// Local wall clock with its offset, e.g. `2026-10-23T10:00:00+02:00`.
    pub start_iso: String,
    pub end_iso: Option<String>,
}

impl Span {
    /// The timer runs on it.
    pub fn is_open(&self) -> bool {
        self.end.is_none()
    }
}

/// A recording with time on the day; TimeTracker merges the same project, note and tag per day.
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Recording {
    pub id: String,
    #[serde(default)]
    pub running: bool,
    /// TimeTracker's total, the open span counted up to the time of the reply.
    pub duration_ms: i64,
    pub spans: Vec<Span>,
    pub project_id: String,
    pub project: String,
    pub client: Option<String>,
    pub tag: Option<String>,
    /// Empty without a note.
    #[serde(default)]
    pub note: String,
}

/// A suggestion to review: a rule's (`kind` `rule`) or a stored one (`stored`, e.g. from Daybook).
#[derive(Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Suggestion {
    pub id: String,
    /// Who suggests it: `TimeTracker`, `Slack: Oscorp`, `Daybook`.
    #[serde(default)]
    pub by: String,
    pub spans: Vec<Span>,
    /// `None` on a stored suggestion still waiting for its project.
    pub project_id: Option<String>,
    pub project: Option<String>,
    pub client: Option<String>,
    pub tag: Option<String>,
    #[serde(default)]
    pub note: String,
}

fn parse_list<T: for<'de> Deserialize<'de>>(stdout: &str, what: &str) -> Result<Vec<T>, String> {
    if stdout.trim().is_empty() {
        return Ok(Vec::new());
    }
    serde_json::from_str(stdout).map_err(|e| format!("TimeTracker's {what} couldn't be read: {e}"))
}

pub fn parse_recordings(stdout: &str) -> Result<Vec<Recording>, String> {
    parse_list(stdout, "recordings")
}

pub fn parse_suggestions(stdout: &str) -> Result<Vec<Suggestion>, String> {
    parse_list(stdout, "suggestions")
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECORDINGS: &str = r#"[
        {"id":"r1","running":false,"durationMs":6300000,"projectId":"p1","project":"Apollo",
         "client":"Acme","tag":"deep","note":"QRS-7061",
         "spans":[
           {"start":1000,"end":2000,"startIso":"2026-10-23T10:30:00+02:00","endIso":"2026-10-23T11:30:00+02:00"},
           {"start":3000,"end":null,"startIso":"2026-10-23T12:00:00+02:00","endIso":null}
         ]}
    ]"#;

    #[test]
    fn recordings_are_read_with_their_spans_and_an_open_span_has_no_end() {
        let recordings = parse_recordings(RECORDINGS).unwrap();
        assert_eq!(
            recordings,
            vec![Recording {
                id: "r1".into(),
                running: false,
                duration_ms: 6_300_000,
                spans: vec![
                    Span {
                        start: 1000,
                        end: Some(2000),
                        start_iso: "2026-10-23T10:30:00+02:00".into(),
                        end_iso: Some("2026-10-23T11:30:00+02:00".into()),
                    },
                    Span {
                        start: 3000,
                        end: None,
                        start_iso: "2026-10-23T12:00:00+02:00".into(),
                        end_iso: None,
                    },
                ],
                project_id: "p1".into(),
                project: "Apollo".into(),
                client: Some("Acme".into()),
                tag: Some("deep".into()),
                note: "QRS-7061".into(),
            }]
        );
    }

    #[test]
    fn a_recording_without_note_or_running_flag_gets_the_defaults() {
        let json = r#"[{"id":"r2","durationMs":0,"spans":[],"projectId":"p","project":"P","client":null,"tag":null}]"#;
        let recording = &parse_recordings(json).unwrap()[0];
        assert_eq!(recording.note, "");
        assert!(!recording.running);
    }

    #[test]
    fn suggestions_are_read_and_a_missing_by_is_empty() {
        let json = r#"[
            {"id":"s1","by":"Slack: Oscorp","projectId":"p1","project":"Apollo","client":null,"tag":null,"note":"Sync",
             "spans":[{"start":1,"end":2,"startIso":"2026-10-23T09:00:00+02:00","endIso":"2026-10-23T09:30:00+02:00"}]},
            {"id":"s2","projectId":"p1","project":"Apollo","client":null,"tag":null,"spans":[]}
        ]"#;
        let suggestions = parse_suggestions(json).unwrap();
        assert_eq!(suggestions.len(), 2);
        assert_eq!(suggestions[0].by, "Slack: Oscorp");
        assert_eq!(suggestions[0].note, "Sync");
        assert_eq!(suggestions[0].spans.len(), 1);
        assert_eq!(suggestions[1].by, "");
        assert_eq!(suggestions[1].note, "");
    }

    #[test]
    fn a_stored_suggestion_without_a_project_is_read() {
        let json = r#"[
            {"id":"s1","by":"Daybook","projectId":null,"project":null,"client":null,"tag":null,"note":"Zoom - Dentist",
             "spans":[{"start":1,"end":2,"startIso":"2026-10-23T09:00:00+02:00","endIso":"2026-10-23T09:30:00+02:00"}]}
        ]"#;
        let suggestions = parse_suggestions(json).unwrap();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].project_id, None);
        assert_eq!(suggestions[0].project, None);
        assert_eq!(suggestions[0].note, "Zoom - Dentist");
    }

    #[test]
    fn an_empty_reply_is_an_empty_list() {
        for stdout in ["", "  \n", "[]"] {
            assert_eq!(parse_recordings(stdout), Ok(vec![]), "{stdout:?}");
            assert_eq!(parse_suggestions(stdout), Ok(vec![]), "{stdout:?}");
        }
    }

    #[test]
    fn an_unreadable_reply_names_what_couldnt_be_read() {
        let recordings = parse_recordings("{nope").unwrap_err();
        assert!(recordings.starts_with("TimeTracker's recordings couldn't be read: "));
        let suggestions = parse_suggestions("{nope").unwrap_err();
        assert!(suggestions.starts_with("TimeTracker's suggestions couldn't be read: "));
    }
}
