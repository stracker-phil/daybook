//! The activity tool's part of the menu-bar menu. Pure.

use super::choices::with_choices;
use super::entry::{MenuEntry, command};
use super::model::MenuInput;
use crate::store::settings::{Feature, ToolId};
use crate::tools::TtState;

/// What the timer runs on (or the activity picked without one), Log Activity (with the recent
/// timers) and, with check-ins, their pause.
pub(super) fn activity_entries(input: &MenuInput) -> Vec<MenuEntry> {
    let state = input.state;
    if !state.has_feature(Feature::Status) {
        return Vec::new();
    }
    let timer_label = match (state.tt, &state.timer, &state.activity) {
        (TtState::Available, Some(timer), _) => timer.text(),
        (TtState::Available, None, Some(activity)) => format!("{} (no timer)", activity.text()),
        (TtState::Available, None, None) => "No timer running".into(),
        (TtState::NotRunning, ..) => "TimeTracker isn't running".into(),
        (TtState::Missing, ..) => "TimeTracker isn't installed".into(),
    };
    let log = command(
        "Log Activity…",
        "activity:switch",
        &input.hotkeys.switch_activity,
    );
    let mut entries = vec![MenuEntry::Info(timer_label)];
    entries.extend(with_choices("Log Activity", log, ToolId::Activity, input));
    if state.check_in_minutes > 0 {
        entries.push(MenuEntry::Checkbox {
            label: "Pause Check-ins Today".into(),
            command: "checkin:pause".into(),
            checked: state.check_ins_paused,
        });
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::Hotkeys;
    use crate::tools::{RecentActivity, TimerView, ToolState, bare_state};

    fn entries_of(state: &ToolState, hotkeys: &Hotkeys) -> Vec<MenuEntry> {
        activity_entries(&MenuInput {
            state,
            hotkeys,
            tools: &[],
        })
    }

    fn entries(state: &ToolState) -> Vec<MenuEntry> {
        entries_of(state, &Hotkeys::none())
    }

    fn log_activity(shortcut: Option<&str>) -> MenuEntry {
        MenuEntry::Command {
            label: "Log Activity…".into(),
            command: "activity:switch".into(),
            shortcut: shortcut.map(String::from),
            enabled: true,
        }
    }

    fn state_with(tt: TtState, timer: Option<TimerView>) -> ToolState {
        ToolState {
            tt,
            timer,
            ..bare_state()
        }
    }

    #[test]
    fn the_info_line_tells_what_the_timer_runs_on_or_why_there_is_none() {
        let running = TimerView {
            project: "Oscorp".into(),
            note: "QRS-6805 review".into(),
        };
        for (tt, timer, line) in [
            (
                TtState::Available,
                Some(running),
                "Oscorp · QRS-6805 review",
            ),
            (TtState::Available, None, "No timer running"),
            (TtState::NotRunning, None, "TimeTracker isn't running"),
            (TtState::Missing, None, "TimeTracker isn't installed"),
        ] {
            assert_eq!(
                entries(&state_with(tt, timer.clone()))[0],
                MenuEntry::Info(line.into()),
                "{tt:?} {timer:?}"
            );
        }
    }

    #[test]
    fn log_activity_follows_the_info_line_with_its_shortcut() {
        let hotkeys = Hotkeys {
            switch_activity: "cmd+f2".into(),
            ..Hotkeys::none()
        };
        assert_eq!(
            entries_of(&bare_state(), &hotkeys),
            vec![
                MenuEntry::Info("TimeTracker isn't running".into()),
                log_activity(Some("cmd+f2")),
            ]
        );
    }

    #[test]
    fn recent_timers_turn_log_activity_into_a_submenu() {
        let state = ToolState {
            recent_activities: vec![RecentActivity {
                project: "Oscorp".into(),
                note: "QRS-6805 review".into(),
                tt_project_id: "tt1".into(),
                tag: None,
            }],
            ..state_with(TtState::Available, None)
        };
        assert_eq!(
            entries(&state),
            vec![
                MenuEntry::Info("No timer running".into()),
                MenuEntry::Submenu {
                    label: "Log Activity".into(),
                    items: vec![
                        log_activity(None),
                        MenuEntry::Separator,
                        MenuEntry::Command {
                            label: "Oscorp · QRS-6805 review".into(),
                            command: "activity:prefill:Oscorp:QRS-6805%20review:tt1".into(),
                            shortcut: None,
                            enabled: true,
                        },
                    ],
                },
            ]
        );
    }

    #[test]
    fn pausing_check_ins_is_offered_only_with_a_check_in_interval() {
        for (minutes, paused, expected) in [
            (0, false, None),
            (0, true, None),
            (15, false, Some(false)),
            (15, true, Some(true)),
        ] {
            let state = ToolState {
                check_in_minutes: minutes,
                check_ins_paused: paused,
                ..bare_state()
            };
            let checkbox = entries(&state).into_iter().find_map(|entry| match entry {
                MenuEntry::Checkbox {
                    label,
                    command,
                    checked,
                } => Some((label, command, checked)),
                _ => None,
            });
            assert_eq!(
                checkbox,
                expected.map(|checked| (
                    "Pause Check-ins Today".to_string(),
                    "checkin:pause".to_string(),
                    checked
                )),
                "{minutes} {paused}"
            );
        }
    }

    #[test]
    fn the_pause_checkbox_comes_last() {
        let state = ToolState {
            check_in_minutes: 15,
            ..bare_state()
        };
        assert_eq!(
            entries(&state).last(),
            Some(&MenuEntry::Checkbox {
                label: "Pause Check-ins Today".into(),
                command: "checkin:pause".into(),
                checked: false,
            })
        );
    }

    #[test]
    fn nothing_is_offered_without_the_status_feature() {
        let state = ToolState {
            features: vec![Feature::Jira],
            check_in_minutes: 15,
            ..bare_state()
        };
        assert_eq!(entries(&state), vec![]);
    }
}
