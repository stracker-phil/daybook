//! The menu-bar item's title: the context's label and the status. TimeTracker's own item tells what is
//! timed, so Daybook adds text only when TimeTracker has nothing to say. Pure.

use chrono::NaiveDateTime;

use crate::clock::minutes_since;
use crate::store::settings::Feature;
use crate::tools::{Status, ToolState, TtState};

/// `12m`, `1h 5m`
fn duration(minutes: u32) -> String {
    if minutes < 60 {
        format!("{minutes}m")
    } else {
        format!("{}h {}m", minutes / 60, minutes % 60)
    }
}

/// What Working adds: TimeTracker's own item names a running timer, so only its absence is said.
fn working_title(state: &ToolState) -> &'static str {
    match (state.tt, &state.timer) {
        (TtState::Available, Some(_)) => "",
        (TtState::Available, None) => "No timer",
        (TtState::NotRunning | TtState::Missing, _) => "Working",
    }
}

/// What the status adds to the title: `No timer`, `Break · 12m`; nothing while Off or before the
/// day's first status.
fn status_title(state: &ToolState, now: NaiveDateTime) -> String {
    if !state.has_feature(Feature::Status) {
        return String::new();
    }
    match state.status {
        Some(Status::Working) => working_title(state).into(),
        Some(Status::Break) => minutes_since(state.status_since.as_deref(), now).map_or_else(
            || "Break".into(),
            |minutes| format!("Break · {}", duration(minutes)),
        ),
        Some(Status::Off) | None => String::new(),
    }
}

/// The context's label and the status (`[W] Break · 12m`); empty shows the icon alone.
pub fn menu_bar_title(state: &ToolState, now: NaiveDateTime) -> String {
    if !state.has_vault {
        return String::new();
    }
    let label = state
        .current_context()
        .map(|context| context.label.trim().to_string())
        .unwrap_or_default();
    [label, status_title(state, now)]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::tools::{ContextItem, TimerView, bare_state};

    fn relabelled(label: &str) -> ToolState {
        ToolState {
            contexts: vec![ContextItem {
                id: "work".into(),
                label: label.into(),
                name: "Work".into(),
            }],
            ..bare_state()
        }
    }

    fn with_status(status: Option<Status>, since: Option<&str>) -> ToolState {
        ToolState {
            status,
            status_since: since.map(String::from),
            ..bare_state()
        }
    }

    fn at(hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 24)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    #[test]
    fn the_menu_bar_title_is_the_trimmed_label_of_the_current_context() {
        assert_eq!(
            menu_bar_title(
                &ToolState {
                    status: None,
                    ..bare_state()
                },
                at(10, 0)
            ),
            "[W]"
        );
        assert_eq!(
            menu_bar_title(
                &ToolState {
                    status: None,
                    ..relabelled("  🐈 ")
                },
                at(10, 0)
            ),
            "🐈"
        );
        assert_eq!(
            menu_bar_title(
                &ToolState {
                    status: None,
                    ..relabelled("")
                },
                at(10, 0)
            ),
            ""
        );
    }

    #[test]
    fn the_menu_bar_title_adds_the_status_after_the_label() {
        for (status, since, now, expected) in [
            (
                Some(Status::Working),
                Some("09:00"),
                at(10, 12),
                "[W] Working",
            ),
            (Some(Status::Working), None, at(10, 12), "[W] Working"),
            (
                Some(Status::Break),
                Some("10:00"),
                at(10, 12),
                "[W] Break · 12m",
            ),
            (
                Some(Status::Break),
                Some("10:00"),
                at(10, 0),
                "[W] Break · 0m",
            ),
            (
                Some(Status::Break),
                Some("10:00"),
                at(10, 59),
                "[W] Break · 59m",
            ),
            (
                Some(Status::Break),
                Some("10:00"),
                at(11, 0),
                "[W] Break · 1h 0m",
            ),
            (
                Some(Status::Break),
                Some("10:00"),
                at(11, 5),
                "[W] Break · 1h 5m",
            ),
            (Some(Status::Break), None, at(10, 12), "[W] Break"),
            (Some(Status::Off), Some("17:00"), at(18, 0), "[W]"),
            (None, None, at(10, 12), "[W]"),
        ] {
            assert_eq!(
                menu_bar_title(&with_status(status, since), now),
                expected,
                "{status:?} since {since:?}"
            );
        }
    }

    fn timed(tt: TtState, timer: Option<&str>) -> ToolState {
        ToolState {
            tt,
            timer: timer.map(|project| TimerView {
                project: project.into(),
                note: String::new(),
            }),
            ..bare_state()
        }
    }

    #[test]
    fn working_tells_about_the_timer_only_when_timetracker_has_nothing_to_say() {
        for (tt, timer, expected) in [
            (TtState::Available, Some("Oscorp"), "[W]"),
            (TtState::Available, None, "[W] No timer"),
            (TtState::NotRunning, None, "[W] Working"),
            (TtState::Missing, None, "[W] Working"),
            (TtState::NotRunning, Some("Oscorp"), "[W] Working"),
            (TtState::Missing, Some("Oscorp"), "[W] Working"),
        ] {
            assert_eq!(
                menu_bar_title(&timed(tt, timer), at(10, 0)),
                expected,
                "{tt:?} {timer:?}"
            );
        }
    }

    #[test]
    fn break_and_off_ignore_the_timer() {
        for (status, expected) in [(Status::Break, "[W] Break"), (Status::Off, "[W]")] {
            for timer in [None, Some("Oscorp")] {
                let state = ToolState {
                    status: Some(status),
                    ..timed(TtState::Available, timer)
                };
                assert_eq!(
                    menu_bar_title(&state, at(10, 0)),
                    expected,
                    "{status:?} {timer:?}"
                );
            }
        }
    }

    #[test]
    fn no_timer_stands_alone_when_the_label_is_empty() {
        let state = ToolState {
            tt: TtState::Available,
            ..relabelled(" ")
        };
        assert_eq!(menu_bar_title(&state, at(10, 0)), "No timer");
    }

    #[test]
    fn the_menu_bar_title_leaves_the_status_out_without_the_status_feature() {
        for status in [
            Some(Status::Working),
            Some(Status::Break),
            Some(Status::Off),
            None,
        ] {
            let state = ToolState {
                features: vec![Feature::Jira],
                ..with_status(status, Some("10:00"))
            };
            assert_eq!(menu_bar_title(&state, at(10, 12)), "[W]", "{status:?}");
        }
    }

    #[test]
    fn the_menu_bar_title_shows_the_status_alone_when_the_label_is_empty() {
        let state = ToolState {
            status: Some(Status::Working),
            ..relabelled(" ")
        };
        assert_eq!(menu_bar_title(&state, at(10, 0)), "Working");
    }

    #[test]
    fn the_menu_bar_title_is_empty_without_a_vault_or_a_current_context() {
        let no_vault = ToolState {
            has_vault: false,
            ..bare_state()
        };
        let unknown_context = ToolState {
            context: Some("gone".into()),
            status: None,
            ..bare_state()
        };
        let none = ToolState {
            context: None,
            status: None,
            ..bare_state()
        };
        for (name, state) in [
            ("no vault", no_vault),
            ("unknown", unknown_context),
            ("none", none),
        ] {
            assert_eq!(menu_bar_title(&state, at(10, 12)), "", "{name}");
        }
    }
}
