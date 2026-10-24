//! Which app trigger is in a call, over a `Probe` that asks the Mac. Pure apart from the probe.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::checks::{
    CHROME_BUNDLE, GENERIC_TITLE, MEET_BUNDLE, TEAMS_BUNDLE, ZOOM_BUNDLE, built_in_check,
    clean_title, meet_call_title,
};
use crate::store::settings::MeetingRule;

/// A permission a detector found missing.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "kebab-case")]
pub enum Missing {
    /// Window titles of other apps.
    Accessibility,
    /// Chrome's tabs (Privacy → Automation).
    Automation,
}

#[derive(Debug, PartialEq)]
pub enum ProbeError {
    Missing(Missing),
    Failed(String),
}

/// What the detectors ask the Mac.
pub trait Probe {
    fn running(&self) -> Result<HashSet<String>, String>;
    fn zoom_in_call(&self) -> Result<bool, String>;
    fn teams_in_call(&self) -> Result<bool, String>;
    /// The URLs and titles of every Chrome tab, in the same order.
    fn chrome_tabs(&self) -> Result<(Vec<String>, Vec<String>), ProbeError>;
    fn window_titles(&self, bundle_id: &str) -> Result<Vec<String>, ProbeError>;
}

/// An app trigger in a call.
#[derive(Clone, Debug, PartialEq)]
pub struct Detected {
    pub app: String,
    pub title: String,
}

/// One round of checks: the call found, and what the checks learned about permissions.
#[derive(Debug, Default, PartialEq)]
pub struct Detection {
    pub found: Option<Detected>,
    /// The permissions a check was refused this round.
    pub missing: Vec<Missing>,
    /// Permissions a check used successfully this round.
    pub granted: Vec<Missing>,
}

impl Detection {
    /// A refused permission leaves the check without an answer.
    fn answer<T>(
        &mut self,
        result: Result<T, ProbeError>,
        uses: Missing,
    ) -> Result<Option<T>, String> {
        match result {
            Ok(value) => {
                self.granted.push(uses);
                Ok(Some(value))
            }
            Err(ProbeError::Missing(missing)) => {
                self.missing.push(missing);
                Ok(None)
            }
            Err(ProbeError::Failed(message)) => Err(message),
        }
    }
}

fn generic(in_call: bool) -> Option<String> {
    in_call.then(|| GENERIC_TITLE.to_string())
}

/// The call title from the app's built-in check; `None` while it isn't in a call.
fn built_in(
    rule: &MeetingRule,
    running: &HashSet<String>,
    probe: &impl Probe,
    detection: &mut Detection,
) -> Result<Option<String>, String> {
    match rule.bundle_id.as_str() {
        ZOOM_BUNDLE if running.contains(ZOOM_BUNDLE) => probe.zoom_in_call().map(generic),
        TEAMS_BUNDLE if running.contains(TEAMS_BUNDLE) => probe.teams_in_call().map(generic),
        MEET_BUNDLE if running.contains(CHROME_BUNDLE) => {
            let tabs = detection.answer(probe.chrome_tabs(), Missing::Automation)?;
            Ok(tabs.and_then(|(urls, titles)| meet_call_title(&urls, &titles)))
        }
        _ => Ok(None),
    }
}

/// The first enabled rule in a call: Zoom, Teams and Meet by their own check, other apps while running
/// (with a window title set, showing a window whose title contains it).
pub fn detect(rules: &[MeetingRule], probe: &impl Probe) -> Result<Detection, String> {
    let running = probe.running()?;
    let mut detection = Detection::default();
    for rule in rules.iter().filter(|rule| rule.enabled) {
        let title = if built_in_check(&rule.bundle_id).is_some() {
            built_in(rule, &running, probe, &mut detection)?
        } else {
            custom(rule, &running, probe, &mut detection)?
        };
        if let Some(title) = title {
            detection.found = Some(Detected {
                app: rule.app.clone(),
                title,
            });
            break;
        }
    }
    Ok(detection)
}

/// Without window titles only a rule that doesn't need them can match.
fn custom(
    rule: &MeetingRule,
    running: &HashSet<String>,
    probe: &impl Probe,
    detection: &mut Detection,
) -> Result<Option<String>, String> {
    if !running.contains(&rule.bundle_id) {
        return Ok(None);
    }
    let titles = detection
        .answer(probe.window_titles(&rule.bundle_id), Missing::Accessibility)?
        .unwrap_or_default();
    let Some(wanted) = rule.window_title.as_deref() else {
        let first = titles.first().map_or("", String::as_str);
        return Ok(Some(clean_title(first, None)));
    };
    let wanted_lower = wanted.to_lowercase();
    Ok(titles
        .iter()
        .find(|title| title.to_lowercase().contains(&wanted_lower))
        .map(|hit| clean_title(hit, Some(wanted))))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::built_in_call_apps;

    const STANDUP: &str = "com.example.standup";

    type Tabs = Result<(Vec<String>, Vec<String>), ProbeError>;

    struct Fake {
        running: Result<HashSet<String>, String>,
        zoom: Result<bool, String>,
        teams: Result<bool, String>,
        tabs: Tabs,
        windows: Result<Vec<String>, ProbeError>,
    }

    fn copy_error(error: &ProbeError) -> ProbeError {
        match error {
            ProbeError::Missing(missing) => ProbeError::Missing(*missing),
            ProbeError::Failed(message) => ProbeError::Failed(message.clone()),
        }
    }

    impl Probe for Fake {
        fn running(&self) -> Result<HashSet<String>, String> {
            self.running.clone()
        }
        fn zoom_in_call(&self) -> Result<bool, String> {
            self.zoom.clone()
        }
        fn teams_in_call(&self) -> Result<bool, String> {
            self.teams.clone()
        }
        fn chrome_tabs(&self) -> Tabs {
            self.tabs.as_ref().map(Clone::clone).map_err(copy_error)
        }
        fn window_titles(&self, _bundle_id: &str) -> Result<Vec<String>, ProbeError> {
            self.windows.as_ref().map(Clone::clone).map_err(copy_error)
        }
    }

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    fn fake(running: &[&str]) -> Fake {
        Fake {
            running: Ok(running.iter().map(|s| s.to_string()).collect()),
            zoom: Ok(false),
            teams: Ok(false),
            tabs: Ok((vec![], vec![])),
            windows: Ok(vec![]),
        }
    }

    fn rule(bundle_id: &str) -> MeetingRule {
        let mut rule = built_in_call_apps().remove(0);
        rule.app = "Standup".into();
        rule.bundle_id = bundle_id.into();
        rule
    }

    fn built_in_rule(bundle_id: &str) -> MeetingRule {
        built_in_call_apps()
            .into_iter()
            .find(|rule| rule.bundle_id == bundle_id)
            .unwrap()
    }

    fn found(detection: &Detection) -> Option<(&str, &str)> {
        detection
            .found
            .as_ref()
            .map(|d| (d.app.as_str(), d.title.as_str()))
    }

    #[test]
    fn nothing_is_found_while_no_rule_app_is_in_a_call() {
        let probe = fake(&[ZOOM_BUNDLE, TEAMS_BUNDLE]);
        let detection = detect(&built_in_call_apps(), &probe).unwrap();
        assert_eq!(detection, Detection::default());
    }

    #[test]
    fn the_first_enabled_rule_in_a_call_wins_in_rule_order() {
        let mut probe = fake(&[ZOOM_BUNDLE, TEAMS_BUNDLE]);
        probe.zoom = Ok(true);
        probe.teams = Ok(true);
        let rules = vec![built_in_rule(TEAMS_BUNDLE), built_in_rule(ZOOM_BUNDLE)];
        assert_eq!(
            found(&detect(&rules, &probe).unwrap()),
            Some(("Teams", "Meeting"))
        );
    }

    #[test]
    fn disabled_rules_are_skipped() {
        let mut probe = fake(&[ZOOM_BUNDLE, TEAMS_BUNDLE]);
        probe.zoom = Ok(true);
        probe.teams = Ok(true);
        let mut zoom = built_in_rule(ZOOM_BUNDLE);
        zoom.enabled = false;
        let rules = vec![zoom, built_in_rule(TEAMS_BUNDLE)];
        assert_eq!(
            found(&detect(&rules, &probe).unwrap()),
            Some(("Teams", "Meeting"))
        );
    }

    #[test]
    fn zoom_and_teams_count_only_while_their_app_runs_and_is_in_a_call() {
        for (bundle, app) in [(ZOOM_BUNDLE, "Zoom"), (TEAMS_BUNDLE, "Teams")] {
            let rules = vec![built_in_rule(bundle)];

            let mut in_call = fake(&[bundle]);
            in_call.zoom = Ok(true);
            in_call.teams = Ok(true);
            assert_eq!(
                found(&detect(&rules, &in_call).unwrap()),
                Some((app, "Meeting")),
                "{app}"
            );

            let mut not_running = fake(&[]);
            not_running.zoom = Ok(true);
            not_running.teams = Ok(true);
            assert_eq!(
                detect(&rules, &not_running).unwrap().found,
                None,
                "{app} not running"
            );

            let idle = fake(&[bundle]);
            assert_eq!(detect(&rules, &idle).unwrap().found, None, "{app} idle");
        }
    }

    #[test]
    fn meet_needs_chrome_running_and_a_meeting_link_tab() {
        let rules = vec![built_in_rule(MEET_BUNDLE)];
        let tabs = (
            strings(&[
                "https://example.com",
                "https://meet.google.com/abc-defg-hij",
            ]),
            strings(&["Example", "Meet – Weekly"]),
        );

        let mut in_call = fake(&[CHROME_BUNDLE]);
        in_call.tabs = Ok(tabs.clone());
        assert_eq!(
            found(&detect(&rules, &in_call).unwrap()),
            Some(("Meet", "Weekly"))
        );

        let mut no_chrome = fake(&[]);
        no_chrome.tabs = Ok(tabs);
        assert_eq!(detect(&rules, &no_chrome).unwrap().found, None);

        let mut no_link = fake(&[CHROME_BUNDLE]);
        no_link.tabs = Ok((strings(&["https://example.com"]), strings(&["Example"])));
        assert_eq!(detect(&rules, &no_link).unwrap().found, None);
    }

    #[test]
    fn refused_chrome_tabs_find_no_meet_call_and_report_automation_missing() {
        let mut probe = fake(&[CHROME_BUNDLE]);
        probe.tabs = Err(ProbeError::Missing(Missing::Automation));
        let detection = detect(&[built_in_rule(MEET_BUNDLE)], &probe).unwrap();
        assert_eq!(
            detection,
            Detection {
                found: None,
                missing: vec![Missing::Automation],
                granted: vec![],
            }
        );
    }

    #[test]
    fn readable_chrome_tabs_report_automation_granted() {
        let probe = fake(&[CHROME_BUNDLE]);
        let detection = detect(&[built_in_rule(MEET_BUNDLE)], &probe).unwrap();
        assert_eq!(
            detection,
            Detection {
                found: None,
                missing: vec![],
                granted: vec![Missing::Automation],
            }
        );
    }

    #[test]
    fn a_custom_app_counts_while_it_runs_titled_by_its_first_cleaned_window() {
        let mut probe = fake(&[STANDUP]);
        probe.windows = Ok(strings(&["Standup - Zoom", "Other"]));
        let detection = detect(&[rule(STANDUP)], &probe).unwrap();
        assert_eq!(found(&detection), Some(("Standup", "Standup")));
        assert_eq!(detection.granted, vec![Missing::Accessibility]);
    }

    #[test]
    fn a_custom_app_without_windows_is_titled_meeting() {
        let probe = fake(&[STANDUP]);
        assert_eq!(
            found(&detect(&[rule(STANDUP)], &probe).unwrap()),
            Some(("Standup", "Meeting"))
        );
    }

    #[test]
    fn a_custom_app_that_is_not_running_does_not_count() {
        let probe = fake(&[]);
        assert_eq!(detect(&[rule(STANDUP)], &probe).unwrap().found, None);
    }

    #[test]
    fn a_window_title_rule_needs_a_window_containing_it_ignoring_case() {
        let mut with_title = rule(STANDUP);
        with_title.window_title = Some("weekly".into());

        let mut hit = fake(&[STANDUP]);
        hit.windows = Ok(strings(&["Notes", "Team WEEKLY sync"]));
        assert_eq!(
            found(&detect(std::slice::from_ref(&with_title), &hit).unwrap()),
            Some(("Standup", "Team WEEKLY sync"))
        );

        let mut miss = fake(&[STANDUP]);
        miss.windows = Ok(strings(&["Notes"]));
        assert_eq!(detect(&[with_title], &miss).unwrap().found, None);
    }

    #[test]
    fn a_window_title_that_only_repeats_the_rule_is_titled_meeting() {
        let mut with_title = rule(STANDUP);
        with_title.window_title = Some("Call".into());
        let mut probe = fake(&[STANDUP]);
        probe.windows = Ok(strings(&["call"]));
        assert_eq!(
            found(&detect(&[with_title], &probe).unwrap()),
            Some(("Standup", "Meeting"))
        );
    }

    #[test]
    fn refused_window_titles_still_match_a_rule_without_a_window_title() {
        let mut probe = fake(&[STANDUP]);
        probe.windows = Err(ProbeError::Missing(Missing::Accessibility));
        let detection = detect(&[rule(STANDUP)], &probe).unwrap();
        assert_eq!(found(&detection), Some(("Standup", "Meeting")));
        assert_eq!(detection.missing, vec![Missing::Accessibility]);
        assert_eq!(detection.granted, vec![]);
    }

    #[test]
    fn refused_window_titles_never_match_a_rule_with_a_window_title() {
        let mut with_title = rule(STANDUP);
        with_title.window_title = Some("Weekly".into());
        let mut probe = fake(&[STANDUP]);
        probe.windows = Err(ProbeError::Missing(Missing::Accessibility));
        let detection = detect(&[with_title], &probe).unwrap();
        assert_eq!(detection.found, None);
        assert_eq!(detection.missing, vec![Missing::Accessibility]);
    }

    #[test]
    fn a_failing_probe_makes_detection_fail() {
        let rules = built_in_call_apps();

        let mut running = fake(&[]);
        running.running = Err("lsappinfo failed".into());
        assert_eq!(detect(&rules, &running).unwrap_err(), "lsappinfo failed");

        let mut zoom = fake(&[ZOOM_BUNDLE]);
        zoom.zoom = Err("ps failed".into());
        assert_eq!(detect(&rules, &zoom).unwrap_err(), "ps failed");

        let mut tabs = fake(&[CHROME_BUNDLE]);
        tabs.tabs = Err(ProbeError::Failed("script failed".into()));
        assert_eq!(detect(&rules, &tabs).unwrap_err(), "script failed");

        let mut windows = fake(&[STANDUP]);
        windows.windows = Err(ProbeError::Failed("ax failed".into()));
        assert_eq!(detect(&[rule(STANDUP)], &windows).unwrap_err(), "ax failed");
    }
}
