//! The Activity tab's view of a day, built from TimeTracker's replies. Pure: Daybook shows TimeTracker's
//! numbers and counts nothing of its own.

use chrono::{DateTime, FixedOffset, NaiveDate};
use serde::Serialize;
use ts_rs::TS;

use super::projects::by_project;
use super::records::{Recording, Suggestion};
use super::timeline::{timeline, timeline_items};
use crate::timetracker::reply::TtProject;

/// Time between rows from this long on shows as a gap.
pub const MIN_GAP_MS: i64 = 5 * 60_000;
pub(super) const GAP_TEXT: &str = "Nothing recorded";

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum DayActivityRowKind {
    Work,
    /// The timer's open span, up to now.
    Running,
    /// Ghost row; not in the total.
    Suggestion,
    /// Nothing recorded for `MIN_GAP_MS` or more.
    Gap,
}

/// One row of the Timeline: a span of a recording or suggestion, or a gap.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct DayActivityRow {
    pub kind: DayActivityRowKind,
    /// `HH:MM`.
    pub start: String,
    /// `HH:MM`, or `now` for the running span.
    pub end: String,
    /// "1h 05m".
    pub duration: String,
    pub duration_ms: i64,
    /// "Apollo · QRS-7061: Data Sync", the project alone without a note, "Nothing recorded"
    /// for a gap.
    pub title: String,
    pub project: Option<String>,
    pub client: Option<String>,
    /// Verbatim; empty without one.
    pub note: String,
    pub tag: Option<String>,
    /// An accent name; `None` for a gap or a project TimeTracker didn't list.
    pub color: Option<String>,
    /// 0 to 8, darkest first.
    pub shade: Option<u8>,
    /// "Suggested by Slack: Oscorp" on a suggestion.
    pub suggested_by: Option<String>,
    /// What a click copies: the note, else the project; `None` for a gap.
    pub copy_text: Option<String>,
    pub recording_id: Option<String>,
}

/// One recording in By Project: every span on one row.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct DayActivityRecording {
    pub recording_id: String,
    /// Verbatim; empty without one.
    pub note: String,
    pub tag: Option<String>,
    /// "10:30–11:30, 12:00–12:45"; the running span ends with `now`.
    pub spans: String,
    pub duration: String,
    pub duration_ms: i64,
    pub running: bool,
}

/// A project in By Project with its recordings.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct DayActivityProject {
    pub project_id: String,
    pub project: String,
    pub client: Option<String>,
    pub color: Option<String>,
    pub shade: Option<u8>,
    pub total: String,
    pub total_ms: i64,
    pub recordings: Vec<DayActivityRecording>,
}

/// TimeTracker's day as the Activity tab shows it.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct DayActivity {
    /// `YYYY-MM-DD`.
    pub day: String,
    pub is_today: bool,
    /// In time order.
    pub rows: Vec<DayActivityRow>,
    /// In first-appearance order; suggestions left out.
    pub projects: Vec<DayActivityProject>,
    /// The recordings' sum as TimeTracker counts it, suggestions left out: "3h 00m".
    pub total: String,
    pub total_ms: i64,
    /// Under the last row: its end, `now` while it runs; `None` without rows.
    pub end_label: Option<String>,
    /// "Nothing recorded yet" today, "Nothing recorded" on another day; `None` once there are rows.
    pub empty: Option<String>,
    /// A timer runs on the day.
    pub running: bool,
}

/// TimeTracker's duration format: whole minutes, "0m", "45m", "1h 05m", "12h 30m".
pub fn format_duration(ms: i64) -> String {
    let minutes = ms.max(0) / 60_000;
    match (minutes / 60, minutes % 60) {
        (0, m) => format!("{m}m"),
        (h, m) => format!("{h}h {m:02}m"),
    }
}

/// The Activity tab for `day` at `now` (in the local offset): the Timeline, By Project and the total.
pub fn day_view(
    recordings: &[Recording],
    suggestions: &[Suggestion],
    projects: &[TtProject],
    now: DateTime<FixedOffset>,
    day: NaiveDate,
) -> DayActivity {
    let is_today = day == now.date_naive();
    let items = timeline_items(recordings, suggestions, &now);
    let (rows, until) = timeline(&items, projects);
    let total_ms = recordings.iter().map(|r| r.duration_ms).sum();
    let empty = rows.is_empty().then(|| {
        if is_today {
            "Nothing recorded yet".to_string()
        } else {
            GAP_TEXT.to_string()
        }
    });
    DayActivity {
        day: day.format("%Y-%m-%d").to_string(),
        is_today,
        running: items
            .iter()
            .any(|item| item.kind == DayActivityRowKind::Running),
        projects: by_project(recordings, projects, &now),
        rows,
        total: format_duration(total_ms),
        total_ms,
        end_label: until.map(|point| point.label),
        empty,
    }
}

#[cfg(test)]
mod tests {
    use chrono::TimeZone;

    use super::*;
    use crate::timetracker::day::records::Span;

    const MIN: i64 = 60_000;

    fn offset() -> FixedOffset {
        FixedOffset::east_opt(2 * 3600).unwrap()
    }

    fn at(hour: u32, minute: u32) -> DateTime<FixedOffset> {
        offset()
            .with_ymd_and_hms(2026, 10, 23, hour, minute, 0)
            .unwrap()
    }

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 23).unwrap()
    }

    fn span(from: (u32, u32), to: Option<(u32, u32)>) -> Span {
        let start = at(from.0, from.1);
        let end = to.map(|(h, m)| at(h, m));
        Span {
            start: start.timestamp_millis(),
            end: end.map(|e| e.timestamp_millis()),
            start_iso: start.to_rfc3339(),
            end_iso: end.map(|e| e.to_rfc3339()),
        }
    }

    fn recording(id: &str, project_id: &str, note: &str, spans: Vec<Span>) -> Recording {
        let duration_ms = spans
            .iter()
            .map(|s| s.end.unwrap_or(at(10, 40).timestamp_millis()) - s.start)
            .sum();
        Recording {
            id: id.into(),
            running: spans.iter().any(|s| s.end.is_none()),
            duration_ms,
            spans,
            project_id: project_id.into(),
            project: format!("Project {project_id}"),
            client: None,
            tag: None,
            note: note.into(),
        }
    }

    fn suggestion(id: &str, by: &str, spans: Vec<Span>) -> Suggestion {
        Suggestion {
            id: id.into(),
            by: by.into(),
            spans,
            project_id: "p1".into(),
            project: "Project p1".into(),
            client: None,
            tag: None,
            note: String::new(),
        }
    }

    fn project(id: &str, color: &str, shade: u8) -> TtProject {
        TtProject {
            id: id.into(),
            name: format!("Project {id}"),
            client_id: None,
            client: None,
            color: color.into(),
            shade,
            billable: false,
            archived: false,
        }
    }

    fn view(
        recordings: &[Recording],
        suggestions: &[Suggestion],
        now: DateTime<FixedOffset>,
    ) -> DayActivity {
        day_view(
            recordings,
            suggestions,
            &[project("p1", "blue", 2)],
            now,
            today(),
        )
    }

    fn kinds(view: &DayActivity) -> Vec<DayActivityRowKind> {
        view.rows.iter().map(|row| row.kind).collect()
    }

    fn times(view: &DayActivity) -> Vec<(String, String)> {
        view.rows
            .iter()
            .map(|row| (row.start.clone(), row.end.clone()))
            .collect()
    }

    #[test]
    fn durations_read_in_whole_minutes() {
        for (ms, expected) in [
            (0, "0m"),
            (45 * MIN, "45m"),
            (60 * MIN, "1h 00m"),
            (65 * MIN, "1h 05m"),
            (12 * 60 * MIN + 30 * MIN, "12h 30m"),
            (59_999, "0m"),
            (-5 * MIN, "0m"),
        ] {
            assert_eq!(format_duration(ms), expected, "{ms}");
        }
    }

    #[test]
    fn rows_are_sorted_by_start_one_per_span() {
        let recordings = [
            recording("r2", "p1", "late", vec![span((14, 0), Some((15, 0)))]),
            recording(
                "r1",
                "p1",
                "early",
                vec![span((9, 0), Some((10, 0))), span((11, 0), Some((12, 0)))],
            ),
        ];
        let v = view(&recordings, &[], at(16, 0));
        assert_eq!(
            v.rows
                .iter()
                .filter(|r| r.kind != DayActivityRowKind::Gap)
                .map(|r| (r.start.as_str(), r.recording_id.as_deref().unwrap()))
                .collect::<Vec<_>>(),
            vec![("09:00", "r1"), ("11:00", "r1"), ("14:00", "r2")]
        );
    }

    #[test]
    fn on_equal_starts_a_recording_comes_before_a_suggestion() {
        let recordings = [recording(
            "r1",
            "p1",
            "",
            vec![span((10, 0), Some((11, 0)))],
        )];
        let suggestions = [suggestion("s1", "", vec![span((10, 0), Some((10, 30)))])];
        let v = view(&recordings, &suggestions, at(12, 0));
        assert_eq!(
            kinds(&v),
            vec![DayActivityRowKind::Work, DayActivityRowKind::Suggestion]
        );
    }

    #[test]
    fn on_equal_starts_the_earlier_end_comes_first() {
        let recordings = [
            recording("long", "p1", "", vec![span((10, 0), Some((11, 0)))]),
            recording("short", "p1", "", vec![span((10, 0), Some((10, 30)))]),
        ];
        let v = view(&recordings, &[], at(12, 0));
        assert_eq!(
            v.rows
                .iter()
                .map(|r| r.recording_id.as_deref().unwrap())
                .collect::<Vec<_>>(),
            vec!["short", "long"]
        );
    }

    #[test]
    fn a_recording_with_two_spans_has_two_timeline_rows_and_one_by_project_recording() {
        let mut r = recording(
            "r1",
            "p1",
            "Sync",
            vec![
                span((10, 30), Some((11, 30))),
                span((12, 0), Some((12, 45))),
            ],
        );
        r.duration_ms = 105 * MIN;
        let v = view(&[r], &[], at(13, 0));
        assert_eq!(
            kinds(&v),
            vec![
                DayActivityRowKind::Work,
                DayActivityRowKind::Gap,
                DayActivityRowKind::Work
            ]
        );
        assert_eq!(v.projects.len(), 1);
        assert_eq!(
            v.projects[0].recordings,
            vec![DayActivityRecording {
                recording_id: "r1".into(),
                note: "Sync".into(),
                tag: None,
                spans: "10:30–11:30, 12:00–12:45".into(),
                duration: "1h 45m".into(),
                duration_ms: 105 * MIN,
                running: false,
            }]
        );
    }

    #[test]
    fn a_gap_shows_from_five_minutes_after_the_latest_end() {
        let four = [
            recording("a", "p1", "", vec![span((10, 0), Some((11, 0)))]),
            recording("b", "p1", "", vec![span((11, 4), Some((12, 0)))]),
        ];
        assert_eq!(
            kinds(&view(&four, &[], at(13, 0))),
            vec![DayActivityRowKind::Work, DayActivityRowKind::Work]
        );

        let five = [
            recording("a", "p1", "", vec![span((10, 0), Some((11, 0)))]),
            recording("b", "p1", "", vec![span((11, 5), Some((12, 0)))]),
        ];
        let v = view(&five, &[], at(13, 0));
        assert_eq!(
            kinds(&v),
            vec![
                DayActivityRowKind::Work,
                DayActivityRowKind::Gap,
                DayActivityRowKind::Work
            ]
        );
        let gap = &v.rows[1];
        assert_eq!(gap.start, "11:00");
        assert_eq!(gap.end, "11:05");
        assert_eq!(gap.duration, "5m");
        assert_eq!(gap.duration_ms, 5 * MIN);
        assert_eq!(gap.title, "Nothing recorded");
        assert_eq!(gap.copy_text, None);
        assert_eq!(gap.recording_id, None);
        assert_eq!(gap.color, None);
    }

    #[test]
    fn a_gap_is_measured_from_the_latest_end_so_far() {
        let recordings = [
            recording("a", "p1", "", vec![span((10, 0), Some((12, 0)))]),
            recording("b", "p1", "", vec![span((10, 30), Some((11, 0)))]),
            recording("c", "p1", "", vec![span((12, 2), Some((13, 0)))]),
        ];
        assert_eq!(
            kinds(&view(&recordings, &[], at(14, 0))),
            vec![DayActivityRowKind::Work; 3]
        );
    }

    #[test]
    fn there_is_no_gap_before_the_first_or_after_the_last_row() {
        let recordings = [recording("a", "p1", "", vec![span((10, 0), Some((11, 0)))])];
        let v = view(&recordings, &[], at(18, 0));
        assert_eq!(kinds(&v), vec![DayActivityRowKind::Work]);
        assert_eq!(v.end_label.as_deref(), Some("11:00"));
    }

    #[test]
    fn suggestions_count_as_covered_time_for_gaps() {
        let recordings = [
            recording("a", "p1", "", vec![span((9, 0), Some((10, 0)))]),
            recording("b", "p1", "", vec![span((10, 30), Some((11, 0)))]),
        ];
        let suggestions = [suggestion("s", "", vec![span((10, 0), Some((10, 30)))])];
        assert_eq!(
            kinds(&view(&recordings, &suggestions, at(12, 0))),
            vec![
                DayActivityRowKind::Work,
                DayActivityRowKind::Suggestion,
                DayActivityRowKind::Work
            ]
        );
    }

    #[test]
    fn an_open_span_is_a_running_row_ending_now() {
        let recordings = [recording("r1", "p1", "Deep", vec![span((10, 0), None)])];
        let v = view(&recordings, &[], at(10, 40));
        assert_eq!(kinds(&v), vec![DayActivityRowKind::Running]);
        let row = &v.rows[0];
        assert_eq!(row.start, "10:00");
        assert_eq!(row.end, "now");
        assert_eq!(row.duration, "40m");
        assert_eq!(row.duration_ms, 40 * MIN);
        assert!(v.running);
        assert_eq!(v.end_label.as_deref(), Some("now"));
        assert_eq!(v.projects[0].recordings[0].spans, "10:00–now");
        assert!(v.projects[0].recordings[0].running);
    }

    #[test]
    fn a_day_without_an_open_span_is_not_running() {
        let recordings = [recording(
            "r1",
            "p1",
            "",
            vec![span((10, 0), Some((11, 0)))],
        )];
        assert!(!view(&recordings, &[], at(12, 0)).running);
    }

    #[test]
    fn a_suggestion_names_who_suggests_it_and_defaults_to_timetracker() {
        let suggestions = [
            suggestion("s1", "Slack: Oscorp", vec![span((9, 0), Some((9, 30)))]),
            suggestion("s2", "", vec![span((10, 0), Some((10, 30)))]),
        ];
        let v = view(&[], &suggestions, at(12, 0));
        assert_eq!(
            v.rows
                .iter()
                .map(|r| (r.kind, r.suggested_by.as_deref(), r.recording_id.clone()))
                .collect::<Vec<_>>(),
            vec![
                (
                    DayActivityRowKind::Suggestion,
                    Some("Suggested by Slack: Oscorp"),
                    None
                ),
                (DayActivityRowKind::Gap, None, None),
                (
                    DayActivityRowKind::Suggestion,
                    Some("Suggested by TimeTracker"),
                    None
                ),
            ]
        );
    }

    #[test]
    fn suggestions_are_left_out_of_the_total_and_the_projects() {
        let recordings = [recording("r1", "p1", "", vec![span((9, 0), Some((10, 0)))])];
        let suggestions = [suggestion("s", "", vec![span((10, 0), Some((11, 0)))])];
        let v = view(&recordings, &suggestions, at(12, 0));
        assert_eq!(v.total, "1h 00m");
        assert_eq!(v.total_ms, 60 * MIN);
        assert_eq!(v.projects.len(), 1);
        assert_eq!(v.projects[0].recordings.len(), 1);
        assert_eq!(v.rows.len(), 2);
    }

    #[test]
    fn a_day_with_only_suggestions_has_rows_but_no_projects_and_no_empty_text() {
        let suggestions = [suggestion("s", "", vec![span((10, 0), Some((11, 0)))])];
        let v = view(&[], &suggestions, at(12, 0));
        assert_eq!(v.rows.len(), 1);
        assert!(v.projects.is_empty());
        assert_eq!(v.empty, None);
        assert_eq!(v.total, "0m");
        assert_eq!(v.end_label.as_deref(), Some("11:00"));
    }

    #[test]
    fn by_project_groups_in_first_appearance_order_and_sums_the_recordings() {
        let recordings = [
            recording("a", "p2", "one", vec![span((9, 0), Some((10, 0)))]),
            recording("b", "p1", "two", vec![span((10, 0), Some((10, 30)))]),
            recording("c", "p2", "three", vec![span((11, 0), Some((11, 15)))]),
        ];
        let v = view(&recordings, &[], at(12, 0));
        assert_eq!(
            v.projects
                .iter()
                .map(|p| {
                    (
                        p.project_id.as_str(),
                        p.total.as_str(),
                        p.total_ms,
                        p.recordings.len(),
                    )
                })
                .collect::<Vec<_>>(),
            vec![("p2", "1h 15m", 75 * MIN, 2), ("p1", "30m", 30 * MIN, 1)]
        );
        assert_eq!(v.total, "1h 45m");
    }

    #[test]
    fn a_known_project_gives_its_colour_to_rows_and_groups() {
        let recordings = [recording("a", "p1", "", vec![span((9, 0), Some((10, 0)))])];
        let v = view(&recordings, &[], at(12, 0));
        assert_eq!(v.rows[0].color.as_deref(), Some("blue"));
        assert_eq!(v.rows[0].shade, Some(2));
        assert_eq!(v.projects[0].color.as_deref(), Some("blue"));
        assert_eq!(v.projects[0].shade, Some(2));
    }

    #[test]
    fn an_unknown_project_has_no_colour_on_row_and_group() {
        let recordings = [recording(
            "a",
            "gone",
            "",
            vec![span((9, 0), Some((10, 0)))],
        )];
        let v = view(&recordings, &[], at(12, 0));
        assert_eq!((v.rows[0].color.clone(), v.rows[0].shade), (None, None));
        assert_eq!(
            (v.projects[0].color.clone(), v.projects[0].shade),
            (None, None)
        );
    }

    #[test]
    fn an_empty_day_today_says_nothing_recorded_yet() {
        let v = view(&[], &[], at(12, 0));
        assert!(v.rows.is_empty());
        assert!(v.projects.is_empty());
        assert_eq!(v.total, "0m");
        assert_eq!(v.total_ms, 0);
        assert_eq!(v.end_label, None);
        assert_eq!(v.empty.as_deref(), Some("Nothing recorded yet"));
        assert!(v.is_today);
        assert!(!v.running);
    }

    #[test]
    fn an_empty_past_day_says_nothing_recorded() {
        let yesterday = NaiveDate::from_ymd_opt(2026, 10, 22).unwrap();
        let v = day_view(&[], &[], &[], at(12, 0), yesterday);
        assert_eq!(v.day, "2026-10-22");
        assert!(!v.is_today);
        assert_eq!(v.empty.as_deref(), Some("Nothing recorded"));
    }

    #[test]
    fn the_title_is_project_and_note_or_the_project_alone() {
        let recordings = [
            recording("a", "p1", "QRS-7061", vec![span((9, 0), Some((10, 0)))]),
            recording("b", "p1", "  ", vec![span((10, 0), Some((11, 0)))]),
        ];
        let v = view(&recordings, &[], at(12, 0));
        assert_eq!(v.rows[0].title, "Project p1 · QRS-7061");
        assert_eq!(v.rows[0].copy_text.as_deref(), Some("QRS-7061"));
        assert_eq!(v.rows[1].title, "Project p1");
        assert_eq!(v.rows[1].copy_text.as_deref(), Some("Project p1"));
    }

    #[test]
    fn each_span_keeps_the_clock_and_offset_timetracker_gave() {
        let start = at(10, 0);
        let end = at(11, 0);
        let recordings = [Recording {
            spans: vec![Span {
                start: start.timestamp_millis(),
                end: Some(end.timestamp_millis()),
                start_iso: "2026-10-23T10:00:00+02:00".into(),
                end_iso: Some("2026-10-23T05:00:00-03:00".into()),
            }],
            ..recording("a", "p1", "", vec![])
        }];
        let v = view(&recordings, &[], at(12, 0));
        assert_eq!(times(&v), vec![("10:00".to_string(), "05:00".to_string())]);
        assert_eq!(v.projects[0].recordings[0].spans, "10:00–05:00");
    }

    #[test]
    fn the_day_serializes_with_camel_case_names() {
        let recordings = [recording("r1", "p1", "", vec![span((10, 0), None)])];
        let v = view(&recordings, &[], at(10, 40));
        let value = serde_json::to_value(&v).unwrap();
        assert_eq!(value["day"], "2026-10-23");
        assert_eq!(value["isToday"], true);
        assert_eq!(value["endLabel"], "now");
        assert_eq!(value["rows"][0]["kind"], "running");
        assert_eq!(value["rows"][0]["durationMs"], 40 * MIN);
        assert_eq!(value["rows"][0]["recordingId"], "r1");
        assert_eq!(value["projects"][0]["totalMs"], 40 * MIN);
        assert_eq!(value["projects"][0]["recordings"][0]["running"], true);
    }
}
