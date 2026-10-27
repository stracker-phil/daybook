//! The Timeline: every span of the day's recordings and suggestions in time order, with gaps.

use chrono::{DateTime, FixedOffset};

use super::projects::project_of;
use super::records::{Recording, Span, Suggestion};
use super::view::{DayActivityRow, DayActivityRowKind, GAP_TEXT, MIN_GAP_MS, format_duration};
use crate::timetracker::reply::TtProject;

/// A moment and how it reads.
#[derive(Clone, Debug)]
pub(super) struct Point {
    pub ms: i64,
    pub label: String,
}

/// `HH:MM` of the wall clock TimeTracker gave, else of `ms` in `now`'s offset.
fn clock(iso: Option<&str>, ms: i64, now: &DateTime<FixedOffset>) -> String {
    iso.and_then(|iso| DateTime::parse_from_rfc3339(iso).ok())
        .or_else(|| DateTime::from_timestamp_millis(ms).map(|at| at.with_timezone(now.offset())))
        .map_or_else(String::new, |at| at.format("%H:%M").to_string())
}

pub(super) fn span_points(span: &Span, now: &DateTime<FixedOffset>) -> (Point, Point) {
    let start = Point {
        ms: span.start,
        label: clock(Some(&span.start_iso), span.start, now),
    };
    let end = match span.end {
        Some(ms) => Point {
            ms,
            label: clock(span.end_iso.as_deref(), ms, now),
        },
        None => Point {
            ms: now.timestamp_millis(),
            label: "now".into(),
        },
    };
    (start, end)
}

fn has_note(note: &str) -> bool {
    !note.trim().is_empty()
}

fn title(project: &str, note: &str) -> String {
    if has_note(note) {
        format!("{project} · {note}")
    } else {
        project.to_string()
    }
}

/// A span waiting for its place in the Timeline.
pub(super) struct Item<'a> {
    start: Point,
    end: Point,
    pub kind: DayActivityRowKind,
    what: What<'a>,
}

enum What<'a> {
    Recording(&'a Recording),
    Suggestion(&'a Suggestion),
}

/// The fields a row takes from its recording or suggestion.
struct Source<'a> {
    project_id: &'a str,
    project: &'a str,
    client: &'a Option<String>,
    note: &'a str,
    tag: &'a Option<String>,
    recording_id: Option<String>,
    suggested_by: Option<String>,
}

fn source<'a>(what: &What<'a>) -> Source<'a> {
    match *what {
        What::Recording(r) => Source {
            project_id: &r.project_id,
            project: &r.project,
            client: &r.client,
            note: &r.note,
            tag: &r.tag,
            recording_id: Some(r.id.clone()),
            suggested_by: None,
        },
        What::Suggestion(s) => {
            let by = if s.by.trim().is_empty() {
                "TimeTracker"
            } else {
                &s.by
            };
            Source {
                project_id: &s.project_id,
                project: &s.project,
                client: &s.client,
                note: &s.note,
                tag: &s.tag,
                recording_id: None,
                suggested_by: Some(format!("Suggested by {by}")),
            }
        }
    }
}

pub(super) fn timeline_items<'a>(
    recordings: &'a [Recording],
    suggestions: &'a [Suggestion],
    now: &DateTime<FixedOffset>,
) -> Vec<Item<'a>> {
    let worked = recordings.iter().flat_map(|recording| {
        recording.spans.iter().map(move |span| {
            let (start, end) = span_points(span, now);
            let kind = if span.is_open() {
                DayActivityRowKind::Running
            } else {
                DayActivityRowKind::Work
            };
            Item {
                start,
                end,
                kind,
                what: What::Recording(recording),
            }
        })
    });
    let suggested = suggestions.iter().flat_map(|suggestion| {
        suggestion.spans.iter().map(move |span| {
            let (start, end) = span_points(span, now);
            Item {
                start,
                end,
                kind: DayActivityRowKind::Suggestion,
                what: What::Suggestion(suggestion),
            }
        })
    });
    let mut items: Vec<Item> = worked.chain(suggested).collect();
    // Ties: recordings before suggestions, then the shorter first.
    items.sort_by_key(|item| {
        (
            item.start.ms,
            item.kind == DayActivityRowKind::Suggestion,
            item.end.ms,
        )
    });
    items
}

fn gap_row(from: &Point, to: &Point) -> DayActivityRow {
    let ms = to.ms - from.ms;
    DayActivityRow {
        kind: DayActivityRowKind::Gap,
        start: from.label.clone(),
        end: to.label.clone(),
        duration: format_duration(ms),
        duration_ms: ms,
        title: GAP_TEXT.into(),
        project: None,
        client: None,
        note: String::new(),
        tag: None,
        color: None,
        shade: None,
        suggested_by: None,
        copy_text: None,
        recording_id: None,
    }
}

fn item_row(item: &Item, projects: &[TtProject]) -> DayActivityRow {
    let source = source(&item.what);
    let project = project_of(projects, source.project_id);
    let ms = item.end.ms - item.start.ms;
    DayActivityRow {
        kind: item.kind,
        start: item.start.label.clone(),
        end: item.end.label.clone(),
        duration: format_duration(ms),
        duration_ms: ms,
        title: title(source.project, source.note),
        project: Some(source.project.to_string()),
        client: source.client.clone(),
        note: source.note.to_string(),
        tag: source.tag.clone(),
        color: project.map(|p| p.color.clone()),
        shade: project.map(|p| p.shade),
        suggested_by: source.suggested_by,
        copy_text: Some(if has_note(source.note) {
            source.note.to_string()
        } else {
            source.project.to_string()
        }),
        recording_id: source.recording_id,
    }
}

/// The Timeline's rows and where the last one ends; suggestions count as covered time for gaps.
pub(super) fn timeline(
    items: &[Item],
    projects: &[TtProject],
) -> (Vec<DayActivityRow>, Option<Point>) {
    let mut rows = Vec::new();
    let mut until: Option<Point> = None;
    for item in items {
        if let Some(from) = &until
            && item.start.ms - from.ms >= MIN_GAP_MS
        {
            rows.push(gap_row(from, &item.start));
        }
        rows.push(item_row(item, projects));
        if until.as_ref().is_none_or(|from| item.end.ms > from.ms) {
            until = Some(item.end.clone());
        }
    }
    (rows, until)
}
