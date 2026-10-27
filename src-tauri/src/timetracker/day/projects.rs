//! By Project: the day's recordings grouped by project.

use chrono::{DateTime, FixedOffset};

use super::records::Recording;
use super::timeline::span_points;
use super::view::{DayActivityProject, DayActivityRecording, format_duration};
use crate::timetracker::reply::TtProject;

pub(super) fn project_of<'a>(projects: &'a [TtProject], id: &str) -> Option<&'a TtProject> {
    projects.iter().find(|project| project.id == id)
}

fn recording_row(recording: &Recording, now: &DateTime<FixedOffset>) -> DayActivityRecording {
    let spans = recording
        .spans
        .iter()
        .map(|span| {
            let (start, end) = span_points(span, now);
            format!("{}–{}", start.label, end.label)
        })
        .collect::<Vec<_>>()
        .join(", ");
    DayActivityRecording {
        recording_id: recording.id.clone(),
        note: recording.note.clone(),
        tag: recording.tag.clone(),
        spans,
        duration: format_duration(recording.duration_ms),
        duration_ms: recording.duration_ms,
        running: recording.running || recording.spans.iter().any(|span| span.is_open()),
    }
}

fn new_group(recording: &Recording, projects: &[TtProject]) -> DayActivityProject {
    let project = project_of(projects, &recording.project_id);
    DayActivityProject {
        project_id: recording.project_id.clone(),
        project: recording.project.clone(),
        client: recording.client.clone(),
        color: project.map(|p| p.color.clone()),
        shade: project.map(|p| p.shade),
        total: String::new(),
        total_ms: 0,
        recordings: Vec::new(),
    }
}

/// Projects in the order their first span starts, each with its recordings in that order.
pub(super) fn by_project(
    recordings: &[Recording],
    projects: &[TtProject],
    now: &DateTime<FixedOffset>,
) -> Vec<DayActivityProject> {
    let mut ordered: Vec<&Recording> = recordings.iter().collect();
    ordered.sort_by_key(|r| {
        r.spans
            .iter()
            .map(|span| span.start)
            .min()
            .unwrap_or(i64::MAX)
    });
    let mut groups: Vec<DayActivityProject> = Vec::new();
    for recording in ordered {
        let index = match groups
            .iter()
            .position(|group| group.project_id == recording.project_id)
        {
            Some(index) => index,
            None => {
                groups.push(new_group(recording, projects));
                groups.len() - 1
            }
        };
        let group = &mut groups[index];
        group.total_ms += recording.duration_ms;
        group.recordings.push(recording_row(recording, now));
    }
    for group in &mut groups {
        group.total = format_duration(group.total_ms);
    }
    groups
}
