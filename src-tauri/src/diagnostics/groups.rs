//! Builds the Settings → Status diagnostic groups from feature facts. Pure: no I/O.

use serde::Serialize;
use ts_rs::TS;

use crate::calendar::CalendarAccess;
use crate::slack::api::SlackIdentity;
use crate::timetracker::{TtState, WatchLink};

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Diagnostics {
    pub groups: Vec<DiagnosticGroup>,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticGroup {
    pub title: String,
    pub rows: Vec<DiagnosticRow>,
}

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticTone {
    Ok,
    Warn,
    Error,
    Info,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticRow {
    pub label: String,
    pub value: String,
    pub tone: DiagnosticTone,
    /// A bus command the page offers as a button.
    pub command: Option<String>,
    pub command_label: Option<String>,
}

fn row(label: &str, value: impl Into<String>, tone: DiagnosticTone) -> DiagnosticRow {
    DiagnosticRow {
        label: label.into(),
        value: value.into(),
        tone,
        command: None,
        command_label: None,
    }
}

impl DiagnosticRow {
    fn offering(mut self, command: &str, label: &str) -> Self {
        self.command = Some(command.into());
        self.command_label = Some(label.into());
        self
    }
}

fn group(title: &str, rows: Vec<DiagnosticRow>) -> DiagnosticGroup {
    DiagnosticGroup {
        title: title.into(),
        rows,
    }
}

pub struct VaultFacts {
    pub path: Option<String>,
    pub today_note: Option<String>,
    pub today_exists: bool,
    pub waiting: u32,
}

pub fn vault_group(facts: &VaultFacts) -> DiagnosticGroup {
    let Some(path) = &facts.path else {
        return group(
            "Vault",
            vec![
                row("Vault", "Not chosen", DiagnosticTone::Error)
                    .offering("settings:vault", "Choose Vault…"),
            ],
        );
    };
    let today = match (&facts.today_note, facts.today_exists) {
        (Some(note), true) => row("Today’s note", format!("Found: {note}"), DiagnosticTone::Ok),
        (Some(note), false) => row(
            "Today’s note",
            format!("Missing: {note}"),
            DiagnosticTone::Warn,
        )
        .offering("daily:create", "Create Today’s Note"),
        (None, _) => row("Today’s note", "Unknown", DiagnosticTone::Warn),
    };
    let waiting = match facts.waiting {
        0 => row("Waiting note changes", "None", DiagnosticTone::Ok),
        1 => row("Waiting note changes", "1 change", DiagnosticTone::Warn),
        count => row(
            "Waiting note changes",
            format!("{count} changes"),
            DiagnosticTone::Warn,
        ),
    };
    group(
        "Vault",
        vec![row("Vault", path, DiagnosticTone::Info), today, waiting],
    )
}

/// What Slack reports; `last_error` is `HH:MM` and the error.
pub struct SlackFacts {
    pub connected: bool,
    pub identity: Option<SlackIdentity>,
    pub last_error: Option<(String, String)>,
}

pub fn slack_group(facts: &SlackFacts) -> DiagnosticGroup {
    let connection = match (facts.connected, &facts.identity) {
        (false, _) => row("Connection", "Not connected", DiagnosticTone::Warn)
            .offering("settings:slack", "Connect…"),
        (true, Some(identity)) => row(
            "Connection",
            format!("{} in {}", identity.user, identity.team),
            DiagnosticTone::Ok,
        ),
        (true, None) => row("Connection", "Connected", DiagnosticTone::Ok),
    };
    let error = match &facts.last_error {
        Some((at, error)) => row(
            "Last error",
            format!("{at} · {error}"),
            DiagnosticTone::Error,
        ),
        None => row("Last error", "None", DiagnosticTone::Ok),
    };
    group("Slack", vec![connection, error])
}

/// What TimeTracker reports; `unlinked` are the active projects without TimeTracker projects.
pub struct TimeTrackerFacts {
    pub state: TtState,
    pub version: Option<String>,
    pub watch: WatchLink,
    pub linked: usize,
    pub unlinked: Vec<String>,
}

pub fn timetracker_group(facts: &TimeTrackerFacts) -> DiagnosticGroup {
    let running = match facts.state {
        TtState::Available => row("TimeTracker", "Running", DiagnosticTone::Ok),
        TtState::NotRunning => row("TimeTracker", "Not running", DiagnosticTone::Warn)
            .offering("timetracker:show", "Open TimeTracker"),
        TtState::Missing => row("TimeTracker", "Not installed", DiagnosticTone::Error)
            .offering("settings:timetracker", "Settings…"),
    };
    let version = match &facts.version {
        Some(version) => row("Version", version, DiagnosticTone::Info),
        None => row("Version", "Unknown", DiagnosticTone::Info),
    };
    let watch = match facts.watch {
        WatchLink::Connected => row("Live updates", "Connected", DiagnosticTone::Ok),
        WatchLink::Polling => row(
            "Live updates",
            "Read every 30 s (TimeTracker has no watch)",
            DiagnosticTone::Info,
        ),
        WatchLink::Waiting => row("Live updates", "Not connected", DiagnosticTone::Warn),
    };
    let total = facts.linked + facts.unlinked.len();
    let linked = row(
        "Linked projects",
        format!("{} of {total}", facts.linked),
        if facts.unlinked.is_empty() {
            DiagnosticTone::Ok
        } else {
            DiagnosticTone::Warn
        },
    );
    let mut rows = vec![running, version, watch, linked];
    if !facts.unlinked.is_empty() {
        rows.push(
            row(
                "Not linked",
                facts.unlinked.join(", "),
                DiagnosticTone::Warn,
            )
            .offering("settings:projects", "Projects…"),
        );
    }
    group("TimeTracker", rows)
}

pub fn permissions_group(calendar: CalendarAccess, accessibility: bool) -> DiagnosticGroup {
    let not_ok = |value: &str, tone| {
        row("Calendar", value, tone).offering("settings:calendar", "Calendar Settings…")
    };
    let calendar = match calendar {
        CalendarAccess::FullAccess => row("Calendar", "Full access", DiagnosticTone::Ok),
        CalendarAccess::NotDetermined => not_ok("Not asked yet", DiagnosticTone::Warn),
        CalendarAccess::WriteOnly => not_ok("Can't read events", DiagnosticTone::Warn),
        CalendarAccess::Denied => not_ok("Denied", DiagnosticTone::Error),
        CalendarAccess::Restricted => not_ok("Restricted", DiagnosticTone::Error),
    };
    let accessibility = if accessibility {
        row("Accessibility", "Allowed", DiagnosticTone::Ok)
    } else {
        row("Accessibility", "Not allowed", DiagnosticTone::Warn)
            .offering("settings:calls", "App Triggers…")
    };
    group("Permissions", vec![calendar, accessibility])
}

pub struct AutomationFacts {
    pub cli_path: String,
    pub cli_linked: bool,
    pub api_running: bool,
    pub api_error: Option<String>,
    pub api_port: u16,
    /// URL scheme without "://", e.g. daybook.
    pub scheme: String,
}

pub fn automation_group(facts: &AutomationFacts) -> DiagnosticGroup {
    let cli = if facts.cli_linked {
        row(
            "Command line",
            format!("Linked: {}", facts.cli_path),
            DiagnosticTone::Ok,
        )
    } else {
        row(
            "Command line",
            format!("{} doesn't run this app", facts.cli_path),
            DiagnosticTone::Warn,
        )
    };
    let api = match (&facts.api_error, facts.api_running) {
        (Some(error), _) => row("HTTP API", error, DiagnosticTone::Error)
            .offering("settings:automation", "Automation Settings…"),
        (None, true) => row(
            "HTTP API",
            format!("Listening on 127.0.0.1:{}", facts.api_port),
            DiagnosticTone::Ok,
        ),
        (None, false) => row("HTTP API", "Off", DiagnosticTone::Info),
    };
    let scheme = row(
        "URL scheme",
        format!("{}://", facts.scheme),
        DiagnosticTone::Info,
    );
    group("Automation", vec![cli, api, scheme])
}

#[cfg(test)]
mod tests {
    use super::*;

    use DiagnosticTone::{Error, Info, Ok, Warn};

    /// `(label, value, tone, command)` of each row.
    fn shown(group: &DiagnosticGroup) -> Vec<(&str, &str, DiagnosticTone, Option<&str>)> {
        group
            .rows
            .iter()
            .map(|row| {
                (
                    row.label.as_str(),
                    row.value.as_str(),
                    row.tone,
                    row.command.as_deref(),
                )
            })
            .collect()
    }

    fn vault(today_note: Option<&str>, today_exists: bool, waiting: u32) -> VaultFacts {
        VaultFacts {
            path: Some("/Vault".into()),
            today_note: today_note.map(String::from),
            today_exists,
            waiting,
        }
    }

    #[test]
    fn without_a_vault_the_group_offers_to_choose_one() {
        let group = vault_group(&VaultFacts {
            path: None,
            ..vault(None, false, 0)
        });
        assert_eq!(group.title, "Vault");
        assert_eq!(
            shown(&group),
            [("Vault", "Not chosen", Error, Some("settings:vault"))]
        );
        assert_eq!(
            group.rows[0].command_label.as_deref(),
            Some("Choose Vault…")
        );
    }

    #[test]
    fn a_vault_with_todays_note_and_no_waiting_changes_is_all_good() {
        let group = vault_group(&vault(Some("2026-10-27.md"), true, 0));
        assert_eq!(
            shown(&group),
            [
                ("Vault", "/Vault", Info, None),
                ("Today’s note", "Found: 2026-10-27.md", Ok, None),
                ("Waiting note changes", "None", Ok, None),
            ]
        );
    }

    #[test]
    fn a_missing_todays_note_offers_to_create_it() {
        let group = vault_group(&vault(Some("2026-10-27.md"), false, 0));
        assert_eq!(
            shown(&group)[1],
            (
                "Today’s note",
                "Missing: 2026-10-27.md",
                Warn,
                Some("daily:create")
            )
        );
        assert_eq!(
            group.rows[1].command_label.as_deref(),
            Some("Create Today’s Note")
        );
    }

    #[test]
    fn an_unknown_todays_note_is_a_warning_without_a_button() {
        for exists in [true, false] {
            let group = vault_group(&vault(None, exists, 0));
            assert_eq!(
                shown(&group)[1],
                ("Today’s note", "Unknown", Warn, None),
                "{exists}"
            );
        }
    }

    #[test]
    fn waiting_note_changes_are_counted() {
        for (waiting, value, tone) in [
            (0, "None", Ok),
            (1, "1 change", Warn),
            (3, "3 changes", Warn),
        ] {
            let group = vault_group(&vault(Some("n.md"), true, waiting));
            assert_eq!(
                shown(&group)[2],
                ("Waiting note changes", value, tone, None),
                "{waiting}"
            );
        }
    }

    fn slack(connected: bool, identity: bool, last_error: Option<(&str, &str)>) -> SlackFacts {
        SlackFacts {
            connected,
            identity: identity.then(|| SlackIdentity {
                team: "Nexo".into(),
                user: "alex".into(),
            }),
            last_error: last_error.map(|(at, error)| (at.into(), error.into())),
        }
    }

    #[test]
    fn slack_not_connected_offers_to_connect() {
        let group = slack_group(&slack(false, true, None));
        assert_eq!(group.title, "Slack");
        assert_eq!(
            shown(&group),
            [
                ("Connection", "Not connected", Warn, Some("settings:slack")),
                ("Last error", "None", Ok, None),
            ]
        );
        assert_eq!(group.rows[0].command_label.as_deref(), Some("Connect…"));
    }

    #[test]
    fn slack_connected_names_the_user_and_team_when_known() {
        assert_eq!(
            shown(&slack_group(&slack(true, true, None)))[0],
            ("Connection", "alex in Nexo", Ok, None)
        );
        assert_eq!(
            shown(&slack_group(&slack(true, false, None)))[0],
            ("Connection", "Connected", Ok, None)
        );
    }

    #[test]
    fn slacks_last_error_shows_its_time_and_text() {
        let group = slack_group(&slack(true, true, Some(("09:41", "invalid_auth"))));
        assert_eq!(
            shown(&group)[1],
            ("Last error", "09:41 · invalid_auth", Error, None)
        );
    }

    fn timetracker(
        state: TtState,
        watch: WatchLink,
        linked: usize,
        unlinked: &[&str],
    ) -> TimeTrackerFacts {
        TimeTrackerFacts {
            state,
            version: Some("1.4.0".into()),
            watch,
            linked,
            unlinked: unlinked.iter().map(|name| name.to_string()).collect(),
        }
    }

    #[test]
    fn a_running_timetracker_with_everything_linked_is_all_good() {
        let group = timetracker_group(&timetracker(
            TtState::Available,
            WatchLink::Connected,
            3,
            &[],
        ));
        assert_eq!(group.title, "TimeTracker");
        assert_eq!(
            shown(&group),
            [
                ("TimeTracker", "Running", Ok, None),
                ("Version", "1.4.0", Info, None),
                ("Live updates", "Connected", Ok, None),
                ("Linked projects", "3 of 3", Ok, None),
            ]
        );
    }

    #[test]
    fn timetracker_not_running_offers_to_open_it() {
        let group = timetracker_group(&timetracker(
            TtState::NotRunning,
            WatchLink::Waiting,
            0,
            &[],
        ));
        assert_eq!(
            shown(&group)[0],
            ("TimeTracker", "Not running", Warn, Some("timetracker:show"))
        );
        assert_eq!(
            group.rows[0].command_label.as_deref(),
            Some("Open TimeTracker")
        );
    }

    #[test]
    fn a_missing_timetracker_is_an_error_pointing_to_its_settings() {
        let group = timetracker_group(&timetracker(TtState::Missing, WatchLink::Waiting, 0, &[]));
        assert_eq!(
            shown(&group)[0],
            (
                "TimeTracker",
                "Not installed",
                Error,
                Some("settings:timetracker")
            )
        );
    }

    #[test]
    fn the_live_updates_row_tells_how_timetracker_is_followed() {
        for (watch, value, tone) in [
            (WatchLink::Connected, "Connected", Ok),
            (
                WatchLink::Polling,
                "Read every 30 s (TimeTracker has no watch)",
                Info,
            ),
            (WatchLink::Waiting, "Not connected", Warn),
        ] {
            let group = timetracker_group(&timetracker(TtState::Available, watch, 1, &[]));
            assert_eq!(
                shown(&group)[2],
                ("Live updates", value, tone, None),
                "{watch:?}"
            );
        }
    }

    #[test]
    fn an_unknown_timetracker_version_reads_unknown() {
        let facts = TimeTrackerFacts {
            version: None,
            ..timetracker(TtState::Available, WatchLink::Connected, 1, &[])
        };
        assert_eq!(
            shown(&timetracker_group(&facts))[1],
            ("Version", "Unknown", Info, None)
        );
    }

    #[test]
    fn unlinked_projects_are_counted_and_listed_with_a_button() {
        let group = timetracker_group(&timetracker(
            TtState::Available,
            WatchLink::Connected,
            2,
            &["Apollo", "Oscorp"],
        ));
        assert_eq!(
            shown(&group)[3..],
            [
                ("Linked projects", "2 of 4", Warn, None),
                (
                    "Not linked",
                    "Apollo, Oscorp",
                    Warn,
                    Some("settings:projects")
                ),
            ]
        );
        assert_eq!(group.rows[4].command_label.as_deref(), Some("Projects…"));
    }

    #[test]
    fn calendar_access_is_a_row_with_a_button_unless_it_is_full() {
        for (access, value, tone, command) in [
            (CalendarAccess::FullAccess, "Full access", Ok, None),
            (
                CalendarAccess::NotDetermined,
                "Not asked yet",
                Warn,
                Some("settings:calendar"),
            ),
            (
                CalendarAccess::WriteOnly,
                "Can't read events",
                Warn,
                Some("settings:calendar"),
            ),
            (
                CalendarAccess::Denied,
                "Denied",
                Error,
                Some("settings:calendar"),
            ),
            (
                CalendarAccess::Restricted,
                "Restricted",
                Error,
                Some("settings:calendar"),
            ),
        ] {
            let group = permissions_group(access, true);
            assert_eq!(group.title, "Permissions");
            assert_eq!(
                shown(&group)[0],
                ("Calendar", value, tone, command),
                "{access:?}"
            );
        }
    }

    #[test]
    fn accessibility_not_allowed_points_to_the_app_triggers_settings() {
        let group = permissions_group(CalendarAccess::FullAccess, false);
        assert_eq!(
            shown(&group)[1],
            ("Accessibility", "Not allowed", Warn, Some("settings:calls"))
        );
        assert_eq!(
            shown(&permissions_group(CalendarAccess::FullAccess, true))[1],
            ("Accessibility", "Allowed", Ok, None)
        );
    }

    fn automation(cli_linked: bool, api_running: bool, api_error: Option<&str>) -> AutomationFacts {
        AutomationFacts {
            cli_path: "/usr/local/bin/daybook".into(),
            cli_linked,
            api_running,
            api_error: api_error.map(String::from),
            api_port: 4747,
            scheme: "daybook".into(),
        }
    }

    #[test]
    fn a_linked_command_line_and_a_running_api_are_good() {
        let group = automation_group(&automation(true, true, None));
        assert_eq!(group.title, "Automation");
        assert_eq!(
            shown(&group),
            [
                ("Command line", "Linked: /usr/local/bin/daybook", Ok, None),
                ("HTTP API", "Listening on 127.0.0.1:4747", Ok, None),
                ("URL scheme", "daybook://", Info, None),
            ]
        );
    }

    #[test]
    fn a_command_line_that_runs_another_app_is_a_warning() {
        let group = automation_group(&automation(false, false, None));
        assert_eq!(
            shown(&group)[..2],
            [
                (
                    "Command line",
                    "/usr/local/bin/daybook doesn't run this app",
                    Warn,
                    None
                ),
                ("HTTP API", "Off", Info, None),
            ]
        );
    }

    #[test]
    fn an_api_error_wins_over_running_and_offers_the_automation_settings() {
        for running in [true, false] {
            let group = automation_group(&automation(true, running, Some("Port 4747 is taken")));
            assert_eq!(
                shown(&group)[1],
                (
                    "HTTP API",
                    "Port 4747 is taken",
                    Error,
                    Some("settings:automation")
                ),
                "{running}"
            );
        }
    }

    #[test]
    fn diagnostics_serialize_with_camel_case_fields() {
        let diagnostics = Diagnostics {
            groups: vec![DiagnosticGroup {
                title: "Vault".into(),
                rows: vec![
                    row("Vault", "Not chosen", Error).offering("settings:vault", "Choose Vault…"),
                    row("Version", "1.0", Info),
                ],
            }],
        };
        assert_eq!(
            serde_json::to_value(&diagnostics).unwrap(),
            serde_json::json!({
                "groups": [{
                    "title": "Vault",
                    "rows": [
                        {
                            "label": "Vault",
                            "value": "Not chosen",
                            "tone": "error",
                            "command": "settings:vault",
                            "commandLabel": "Choose Vault…",
                        },
                        {
                            "label": "Version",
                            "value": "1.0",
                            "tone": "info",
                            "command": null,
                            "commandLabel": null,
                        },
                    ],
                }],
            })
        );
    }
}
