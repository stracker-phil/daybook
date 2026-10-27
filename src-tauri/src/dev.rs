//! Debug-build conveniences for driving the app from scripts/dev without disturbing the user: window
//! placement, background launches, a check of the vault write path and firing the wizard. Only compiled
//! into debug builds (`#[cfg(debug_assertions)]` on the module and its call sites).

use std::sync::Arc;

use chrono::{NaiveDateTime, NaiveTime};

use tauri::{Manager, State};

use crate::calendar::events::{CalendarEvent, Event};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::vault::ops::NoteOp;
use crate::vault::{WriteOutcome, commands::blocking};
use crate::wizard::prompts::Reason;

/// `DAYBOOK_WINDOW_FRAME="x,y,width,height"` (logical points) places the main window.
pub fn window_frame() -> Option<(f64, f64, f64, f64)> {
    parse_frame(&std::env::var("DAYBOOK_WINDOW_FRAME").ok()?)
}

/// `DAYBOOK_BACKGROUND=1` keeps the window from taking focus.
pub fn background() -> bool {
    std::env::var("DAYBOOK_BACKGROUND").is_ok_and(|v| v == "1")
}

/// Runs JavaScript sent by `scripts/dev/js.sh` in the main window, or in another one named by a first
/// line `@<label>`.
pub fn eval(app: &tauri::AppHandle, js: &str) {
    let (label, js) = match js.strip_prefix('@').and_then(|rest| rest.split_once('\n')) {
        Some((label, rest)) => (label.trim(), rest),
        None => (crate::window::MAIN, js),
    };
    let Some(window) = app.get_webview_window(label) else {
        return;
    };
    if let Err(e) = window.eval(js) {
        log::warn!("Couldn't evaluate script from scripts/dev: {e}");
    }
}

/// Applies a note op to today's note, for checking the write path from `scripts/dev/js.sh`.
#[tauri::command]
pub async fn dev_edit_today_note(
    state: State<'_, AppState>,
    op: NoteOp,
) -> AppResult<WriteOutcome> {
    let notes = Arc::clone(&state.notes);
    let settings = state.settings.get();
    blocking(move || notes.edit_today(&settings, op)).await
}

/// Gives the wizard a chance as if `reason` (`due` or an `OpenEvent` such as `off`) happened today at
/// `at` (`HH:MM`, default now), also while away. Once-a-day triggers stay once a day (`routines.json`).
#[tauri::command]
pub async fn dev_wizard_fire(
    app: tauri::AppHandle,
    reason: String,
    at: Option<String>,
) -> AppResult<()> {
    let reason = Reason::from_name(&reason)
        .ok_or_else(|| AppError::InvalidInput(format!("Not a reason: {reason}")))?;
    let now = chrono::Local::now().naive_local();
    let when = match at.as_deref() {
        Some(at) => chrono::NaiveTime::parse_from_str(at, "%H:%M")
            .map(|time| now.date().and_time(time))
            .map_err(|_| AppError::InvalidInput(format!("Not a time: {at}")))?,
        None => now,
    };
    crate::wizard::fire(&app, reason, when).await;
    Ok(())
}

/// `DAYBOOK_DEMO_EVENTS=<file.json>` replaces the calendar with made-up events for screenshots
/// (`scripts/dev/demo/`): a list of `{ "title", "calendar", "start": "HH:MM:SS", "end": "HH:MM:SS", "url"? }`,
/// repeated on every day from `from` until `to`. `None` without the variable or with an unreadable file.
pub fn demo_events(from: NaiveDateTime, to: NaiveDateTime) -> Option<Vec<CalendarEvent>> {
    let raw = std::fs::read_to_string(std::env::var("DAYBOOK_DEMO_EVENTS").ok()?).ok()?;
    let demo: Vec<DemoEvent> = serde_json::from_str(&raw)
        .inspect_err(|e| log::warn!("DAYBOOK_DEMO_EVENTS couldn't be read: {e}"))
        .ok()?;
    Some(demo_days(&demo, from, to))
}

#[derive(serde::Deserialize)]
struct DemoEvent {
    title: String,
    calendar: String,
    start: NaiveTime,
    end: NaiveTime,
    url: Option<String>,
}

fn demo_days(demo: &[DemoEvent], from: NaiveDateTime, to: NaiveDateTime) -> Vec<CalendarEvent> {
    from.date()
        .iter_days()
        .take_while(|day| day.and_time(NaiveTime::MIN) < to)
        .flat_map(|day| {
            demo.iter().map(move |event| CalendarEvent {
                event: Event {
                    title: event.title.clone(),
                    calendar: event.calendar.clone(),
                    start: day.and_time(event.start),
                    end: day.and_time(event.end),
                    url: event.url.clone(),
                    location: None,
                    notes: None,
                },
                all_day: false,
                canceled: false,
                declined: false,
                busy: true,
            })
        })
        .collect()
}

fn parse_frame(raw: &str) -> Option<(f64, f64, f64, f64)> {
    let v: Vec<f64> = raw
        .split(',')
        .map(|p| p.trim().parse().ok())
        .collect::<Option<_>>()?;
    if !v.iter().all(|n| n.is_finite()) {
        return None;
    }
    match v[..] {
        [x, y, w, h] if w > 0.0 && h > 0.0 => Some((x, y, w, h)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frames_and_rejects_garbage() {
        assert_eq!(
            parse_frame("-3000, -537, 1400,900"),
            Some((-3000.0, -537.0, 1400.0, 900.0))
        );
        assert_eq!(parse_frame("1,2,3"), None);
        assert_eq!(parse_frame("1,2,0,4"), None);
        assert_eq!(parse_frame("a,b,c,d"), None);
    }

    #[test]
    fn non_finite_or_negative_sizes_are_rejected() {
        for raw in ["NaN,0,10,10", "0,0,inf,10", "0,0,-5,10"] {
            assert_eq!(parse_frame(raw), None, "{raw}");
        }
    }

    #[test]
    fn whitespace_around_numbers_is_ignored() {
        assert_eq!(parse_frame("  1 , 2 ,  3,4 "), Some((1.0, 2.0, 3.0, 4.0)));
    }

    fn at(day: u32, h: u32, m: u32) -> NaiveDateTime {
        chrono::NaiveDate::from_ymd_opt(2026, 3, day)
            .unwrap()
            .and_hms_opt(h, m, 0)
            .unwrap()
    }

    fn demo_list() -> Vec<DemoEvent> {
        serde_json::from_str(
            r#"[
                { "title": "Standup", "calendar": "Work", "start": "09:30:00", "end": "09:45:00",
                  "url": "https://meet.example.com/standup" },
                { "title": "Lunch", "calendar": "Home", "start": "12:00:00", "end": "13:00:00" }
            ]"#,
        )
        .unwrap()
    }

    fn expected(
        title: &str,
        calendar: &str,
        start: NaiveDateTime,
        end: NaiveDateTime,
        url: Option<&str>,
    ) -> CalendarEvent {
        CalendarEvent {
            event: Event {
                title: title.into(),
                calendar: calendar.into(),
                start,
                end,
                url: url.map(Into::into),
                location: None,
                notes: None,
            },
            all_day: false,
            canceled: false,
            declined: false,
            busy: true,
        }
    }

    #[test]
    fn a_one_day_range_yields_each_demo_event_once_with_its_times() {
        let events = demo_days(&demo_list(), at(10, 0, 0), at(11, 0, 0));
        assert_eq!(
            events,
            vec![
                expected(
                    "Standup",
                    "Work",
                    at(10, 9, 30),
                    at(10, 9, 45),
                    Some("https://meet.example.com/standup")
                ),
                expected("Lunch", "Home", at(10, 12, 0), at(10, 13, 0), None),
            ]
        );
    }

    #[test]
    fn a_three_day_range_repeats_the_events_on_each_day() {
        let events = demo_days(&demo_list(), at(10, 8, 0), at(13, 0, 0));
        let starts: Vec<_> = events.iter().map(|e| e.event.start).collect();
        assert_eq!(
            starts,
            vec![
                at(10, 9, 30),
                at(10, 12, 0),
                at(11, 9, 30),
                at(11, 12, 0),
                at(12, 9, 30),
                at(12, 12, 0),
            ]
        );
    }

    #[test]
    fn a_range_ending_at_the_first_midnight_yields_no_events() {
        assert!(demo_days(&demo_list(), at(10, 0, 0), at(10, 0, 0)).is_empty());
        assert!(demo_days(&demo_list(), at(10, 8, 0), at(10, 0, 0)).is_empty());
    }
}
