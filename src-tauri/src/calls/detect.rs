//! Which app triggers are in a call, over a `Probe` that asks the Mac. Pure apart from the probe.
//!
//! Every rule gets its own answer: one failing detector leaves only its rule unknown.

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
    /// Whether a process of the app, its helpers included, records from a microphone.
    fn uses_microphone(&self, bundle_id: &str) -> Result<bool, String>;
    /// Teams shows its floating call window, on any Space.
    fn teams_call_window(&self) -> bool;
    /// The URLs and titles of every Chrome tab, in the same order.
    fn chrome_tabs(&self) -> Result<(Vec<String>, Vec<String>), ProbeError>;
    fn window_titles(&self, bundle_id: &str) -> Result<Vec<String>, ProbeError>;
}

/// What one rule's check found.
#[derive(Clone, Debug, PartialEq)]
pub enum Found {
    /// In a call, with its title.
    Call(String),
    NotInCall,
    /// The check failed; why.
    Unknown(String),
}

/// One enabled rule's answer.
#[derive(Clone, Debug, PartialEq)]
pub struct RuleCheck {
    pub bundle_id: String,
    pub app: String,
    pub found: Found,
}

impl RuleCheck {
    fn new(rule: &MeetingRule, found: Found) -> Self {
        Self {
            bundle_id: rule.bundle_id.clone(),
            app: rule.app.clone(),
            found,
        }
    }
}

/// Every enabled rule unknown, for a check that couldn't run.
pub fn unknown_checks(rules: &[MeetingRule], error: &str) -> Vec<RuleCheck> {
    rules
        .iter()
        .filter(|rule| rule.enabled)
        .map(|rule| RuleCheck::new(rule, Found::Unknown(error.to_string())))
        .collect()
}

/// One round of checks: each rule's answer in rule order, and what the checks learned about permissions.
#[derive(Debug, Default, PartialEq)]
pub struct Detection {
    pub checks: Vec<RuleCheck>,
    /// The permissions a check was refused this round.
    pub missing: Vec<Missing>,
    /// Permissions a check used successfully this round.
    pub granted: Vec<Missing>,
}

impl Detection {
    /// The first failure this round, for Settings.
    pub fn error(&self) -> Option<&str> {
        self.checks.iter().find_map(|check| match &check.found {
            Found::Unknown(error) => Some(error.as_str()),
            _ => None,
        })
    }

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

/// Teams records from the microphone during a call, or shows its call window (also while muted). A
/// failed microphone check counts only when the window doesn't answer it.
fn teams(probe: &impl Probe) -> Result<Option<String>, String> {
    if probe.teams_call_window() {
        return Ok(generic(true));
    }
    probe.uses_microphone(TEAMS_BUNDLE).map(generic)
}

/// A Meet meeting tab counts while Chrome records from the microphone: the tab stays open on the page
/// after leaving.
fn meet(probe: &impl Probe, detection: &mut Detection) -> Result<Option<String>, String> {
    let tabs = detection.answer(probe.chrome_tabs(), Missing::Automation)?;
    let Some(title) = tabs.and_then(|(urls, titles)| meet_call_title(&urls, &titles)) else {
        return Ok(None);
    };
    Ok(probe.uses_microphone(CHROME_BUNDLE)?.then_some(title))
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
        TEAMS_BUNDLE if running.contains(TEAMS_BUNDLE) => teams(probe),
        MEET_BUNDLE if running.contains(CHROME_BUNDLE) => meet(probe, detection),
        _ => Ok(None),
    }
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
    if rule.needs_microphone && !probe.uses_microphone(&rule.bundle_id)? {
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

fn check_rule(
    rule: &MeetingRule,
    running: &HashSet<String>,
    probe: &impl Probe,
    detection: &mut Detection,
) -> Found {
    let title = if built_in_check(&rule.bundle_id).is_some() {
        built_in(rule, running, probe, detection)
    } else {
        custom(rule, running, probe, detection)
    };
    match title {
        Ok(Some(title)) => Found::Call(title),
        Ok(None) => Found::NotInCall,
        Err(error) => Found::Unknown(error),
    }
}

/// Checks enabled rules in order until one is in a call and the active call's rule (if any) has answered.
pub fn detect(rules: &[MeetingRule], probe: &impl Probe, active: Option<&str>) -> Detection {
    let mut detection = Detection::default();
    let running = match probe.running() {
        Ok(running) => running,
        Err(error) => {
            detection.checks = unknown_checks(rules, &error);
            return detection;
        }
    };
    let mut found_one = false;
    let mut active_answered = active.is_none();
    for rule in rules.iter().filter(|rule| rule.enabled) {
        if found_one && active_answered {
            break;
        }
        let found = check_rule(rule, &running, probe, &mut detection);
        found_one |= matches!(found, Found::Call(_));
        active_answered |= active == Some(rule.bundle_id.as_str());
        detection.checks.push(RuleCheck::new(rule, found));
    }
    detection
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::store::settings::built_in_call_apps;

    const STANDUP: &str = "com.example.standup";

    type Tabs = Result<(Vec<String>, Vec<String>), ProbeError>;

    struct Fake {
        running: Result<HashSet<String>, String>,
        zoom: Result<bool, String>,
        /// Per bundle id; an app not listed records nothing.
        microphone: HashMap<String, Result<bool, String>>,
        teams_window: bool,
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
        fn uses_microphone(&self, bundle_id: &str) -> Result<bool, String> {
            self.microphone.get(bundle_id).cloned().unwrap_or(Ok(false))
        }
        fn teams_call_window(&self) -> bool {
            self.teams_window
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
            microphone: HashMap::new(),
            teams_window: false,
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

    fn check(app: &str, bundle_id: &str, found: Found) -> RuleCheck {
        RuleCheck {
            bundle_id: bundle_id.into(),
            app: app.into(),
            found,
        }
    }

    fn call(title: &str) -> Found {
        Found::Call(title.into())
    }

    fn unknown(message: &str) -> Found {
        Found::Unknown(message.into())
    }

    fn answers(detection: &Detection) -> Vec<(&str, &Found)> {
        detection
            .checks
            .iter()
            .map(|check| (check.app.as_str(), &check.found))
            .collect()
    }

    fn weekly_tabs() -> Tabs {
        Ok((
            strings(&[
                "https://example.com",
                "https://meet.google.com/abc-defg-hij",
            ]),
            strings(&["Example", "Meet – Weekly"]),
        ))
    }

    #[test]
    fn rules_whose_apps_are_not_in_a_call_are_all_answered_as_not_in_a_call() {
        let probe = fake(&[ZOOM_BUNDLE, TEAMS_BUNDLE]);
        let rules = vec![
            built_in_rule(ZOOM_BUNDLE),
            built_in_rule(TEAMS_BUNDLE),
            built_in_rule(MEET_BUNDLE),
        ];
        assert_eq!(
            detect(&rules, &probe, None),
            Detection {
                checks: vec![
                    check("Zoom", ZOOM_BUNDLE, Found::NotInCall),
                    check("Teams", TEAMS_BUNDLE, Found::NotInCall),
                    check("Meet", MEET_BUNDLE, Found::NotInCall),
                ],
                missing: vec![],
                granted: vec![],
            }
        );
    }

    #[test]
    fn without_rules_nothing_is_checked() {
        assert_eq!(detect(&[], &fake(&[]), None), Detection::default());
    }

    #[test]
    fn checks_come_in_rule_order_and_stop_after_the_first_call_without_an_active_call() {
        let mut probe = fake(&[ZOOM_BUNDLE, TEAMS_BUNDLE]);
        probe.zoom = Ok(true);
        probe.teams_window = true;
        let rules = vec![built_in_rule(TEAMS_BUNDLE), built_in_rule(ZOOM_BUNDLE)];
        let detection = detect(&rules, &probe, None);
        assert_eq!(
            detection.checks,
            vec![check("Teams", TEAMS_BUNDLE, call("Meeting"))]
        );
    }

    #[test]
    fn checks_go_on_past_the_first_call_until_the_active_rule_has_answered() {
        let mut probe = fake(&[ZOOM_BUNDLE, TEAMS_BUNDLE]);
        probe.zoom = Ok(true);
        probe.teams_window = true;
        let rules = vec![
            built_in_rule(TEAMS_BUNDLE),
            built_in_rule(ZOOM_BUNDLE),
            built_in_rule(MEET_BUNDLE),
        ];
        let detection = detect(&rules, &probe, Some(ZOOM_BUNDLE));
        assert_eq!(
            detection.checks,
            vec![
                check("Teams", TEAMS_BUNDLE, call("Meeting")),
                check("Zoom", ZOOM_BUNDLE, call("Meeting")),
            ]
        );
    }

    #[test]
    fn checks_stop_at_the_active_rule_when_it_is_the_first_call() {
        let mut probe = fake(&[ZOOM_BUNDLE, TEAMS_BUNDLE]);
        probe.zoom = Ok(true);
        probe.teams_window = true;
        let rules = vec![built_in_rule(ZOOM_BUNDLE), built_in_rule(TEAMS_BUNDLE)];
        let detection = detect(&rules, &probe, Some(ZOOM_BUNDLE));
        assert_eq!(
            detection.checks,
            vec![check("Zoom", ZOOM_BUNDLE, call("Meeting"))]
        );
    }

    #[test]
    fn every_rule_is_checked_while_none_is_in_a_call() {
        let probe = fake(&[ZOOM_BUNDLE, TEAMS_BUNDLE]);
        let rules = vec![built_in_rule(ZOOM_BUNDLE), built_in_rule(TEAMS_BUNDLE)];
        let detection = detect(&rules, &probe, Some(TEAMS_BUNDLE));
        assert_eq!(
            answers(&detection),
            vec![("Zoom", &Found::NotInCall), ("Teams", &Found::NotInCall)]
        );
    }

    #[test]
    fn disabled_rules_are_skipped() {
        let mut probe = fake(&[ZOOM_BUNDLE, TEAMS_BUNDLE]);
        probe.zoom = Ok(true);
        probe.teams_window = true;
        let mut zoom = built_in_rule(ZOOM_BUNDLE);
        zoom.enabled = false;
        let rules = vec![zoom, built_in_rule(TEAMS_BUNDLE)];
        let detection = detect(&rules, &probe, None);
        assert_eq!(
            detection.checks,
            vec![check("Teams", TEAMS_BUNDLE, call("Meeting"))]
        );
    }

    #[test]
    fn a_failing_list_of_running_apps_leaves_every_enabled_rule_unknown() {
        let mut probe = fake(&[]);
        probe.running = Err("lsappinfo failed".into());
        let mut meet = built_in_rule(MEET_BUNDLE);
        meet.enabled = false;
        let rules = vec![built_in_rule(ZOOM_BUNDLE), meet, rule(STANDUP)];
        let detection = detect(&rules, &probe, Some(ZOOM_BUNDLE));
        assert_eq!(
            detection,
            Detection {
                checks: vec![
                    check("Zoom", ZOOM_BUNDLE, unknown("lsappinfo failed")),
                    check("Standup", STANDUP, unknown("lsappinfo failed")),
                ],
                missing: vec![],
                granted: vec![],
            }
        );
    }

    #[test]
    fn zoom_counts_only_while_its_app_runs_and_has_a_meeting() {
        let rules = vec![built_in_rule(ZOOM_BUNDLE)];

        let mut in_call = fake(&[ZOOM_BUNDLE]);
        in_call.zoom = Ok(true);
        assert_eq!(
            detect(&rules, &in_call, None).checks,
            vec![check("Zoom", ZOOM_BUNDLE, call("Meeting"))]
        );

        let mut not_running = fake(&[]);
        not_running.zoom = Ok(true);
        assert_eq!(
            detect(&rules, &not_running, None).checks,
            vec![check("Zoom", ZOOM_BUNDLE, Found::NotInCall)],
            "not running"
        );

        assert_eq!(
            detect(&rules, &fake(&[ZOOM_BUNDLE]), None).checks,
            vec![check("Zoom", ZOOM_BUNDLE, Found::NotInCall)],
            "idle"
        );
    }

    #[test]
    fn a_failing_zoom_process_check_leaves_only_zoom_unknown() {
        let mut probe = fake(&[ZOOM_BUNDLE, TEAMS_BUNDLE]);
        probe.zoom = Err("pgrep failed: boom".into());
        probe.teams_window = true;
        let rules = vec![built_in_rule(ZOOM_BUNDLE), built_in_rule(TEAMS_BUNDLE)];
        let detection = detect(&rules, &probe, None);
        assert_eq!(
            detection.checks,
            vec![
                check("Zoom", ZOOM_BUNDLE, unknown("pgrep failed: boom")),
                check("Teams", TEAMS_BUNDLE, call("Meeting")),
            ]
        );
    }

    #[test]
    fn teams_counts_with_its_call_window_the_microphone_or_neither() {
        let rules = vec![built_in_rule(TEAMS_BUNDLE)];
        let mic =
            |answer: Result<bool, String>| HashMap::from([(TEAMS_BUNDLE.to_string(), answer)]);
        let cases = [
            ("window", true, mic(Ok(false)), call("Meeting")),
            (
                "window, microphone error",
                true,
                mic(Err("pmset failed".into())),
                call("Meeting"),
            ),
            ("microphone", false, mic(Ok(true)), call("Meeting")),
            ("idle", false, mic(Ok(false)), Found::NotInCall),
            (
                "microphone error",
                false,
                mic(Err("pmset failed".into())),
                unknown("pmset failed"),
            ),
        ];
        for (name, window, microphone, expected) in cases {
            let mut probe = fake(&[TEAMS_BUNDLE]);
            probe.teams_window = window;
            probe.microphone = microphone;
            assert_eq!(
                detect(&rules, &probe, None).checks,
                vec![check("Teams", TEAMS_BUNDLE, expected)],
                "{name}"
            );
        }
    }

    #[test]
    fn teams_that_is_not_running_is_not_in_a_call() {
        let mut probe = fake(&[]);
        probe.teams_window = true;
        probe.microphone.insert(TEAMS_BUNDLE.into(), Ok(true));
        assert_eq!(
            detect(&[built_in_rule(TEAMS_BUNDLE)], &probe, None).checks,
            vec![check("Teams", TEAMS_BUNDLE, Found::NotInCall)]
        );
    }

    #[test]
    fn meet_needs_a_meeting_link_tab_and_chrome_on_the_microphone() {
        let rules = vec![built_in_rule(MEET_BUNDLE)];

        let mut in_call = fake(&[CHROME_BUNDLE]);
        in_call.tabs = weekly_tabs();
        in_call.microphone.insert(CHROME_BUNDLE.into(), Ok(true));
        assert_eq!(
            detect(&rules, &in_call, None).checks,
            vec![check("Meet", MEET_BUNDLE, call("Weekly"))]
        );

        let mut left = fake(&[CHROME_BUNDLE]);
        left.tabs = weekly_tabs();
        assert_eq!(
            detect(&rules, &left, None).checks,
            vec![check("Meet", MEET_BUNDLE, Found::NotInCall)],
            "tab without microphone"
        );

        let mut no_chrome = fake(&[]);
        no_chrome.tabs = weekly_tabs();
        no_chrome.microphone.insert(CHROME_BUNDLE.into(), Ok(true));
        assert_eq!(
            detect(&rules, &no_chrome, None).checks,
            vec![check("Meet", MEET_BUNDLE, Found::NotInCall)],
            "chrome not running"
        );

        let mut no_link = fake(&[CHROME_BUNDLE]);
        no_link.tabs = Ok((strings(&["https://example.com"]), strings(&["Example"])));
        no_link.microphone.insert(CHROME_BUNDLE.into(), Ok(true));
        assert_eq!(
            detect(&rules, &no_link, None).checks,
            vec![check("Meet", MEET_BUNDLE, Found::NotInCall)],
            "no meeting link"
        );
    }

    #[test]
    fn a_failing_microphone_check_leaves_meet_unknown_only_with_a_meeting_tab() {
        let rules = vec![built_in_rule(MEET_BUNDLE)];

        let mut with_tab = fake(&[CHROME_BUNDLE]);
        with_tab.tabs = weekly_tabs();
        with_tab
            .microphone
            .insert(CHROME_BUNDLE.into(), Err("pmset failed".into()));
        assert_eq!(
            detect(&rules, &with_tab, None).checks,
            vec![check("Meet", MEET_BUNDLE, unknown("pmset failed"))]
        );

        let mut without_tab = fake(&[CHROME_BUNDLE]);
        without_tab
            .microphone
            .insert(CHROME_BUNDLE.into(), Err("pmset failed".into()));
        assert_eq!(
            detect(&rules, &without_tab, None).checks,
            vec![check("Meet", MEET_BUNDLE, Found::NotInCall)]
        );
    }

    #[test]
    fn refused_chrome_tabs_find_no_meet_call_and_report_automation_missing() {
        let mut probe = fake(&[CHROME_BUNDLE]);
        probe.tabs = Err(ProbeError::Missing(Missing::Automation));
        let detection = detect(&[built_in_rule(MEET_BUNDLE)], &probe, None);
        assert_eq!(
            detection,
            Detection {
                checks: vec![check("Meet", MEET_BUNDLE, Found::NotInCall)],
                missing: vec![Missing::Automation],
                granted: vec![],
            }
        );
    }

    #[test]
    fn readable_chrome_tabs_report_automation_granted() {
        let probe = fake(&[CHROME_BUNDLE]);
        let detection = detect(&[built_in_rule(MEET_BUNDLE)], &probe, None);
        assert_eq!(
            detection,
            Detection {
                checks: vec![check("Meet", MEET_BUNDLE, Found::NotInCall)],
                missing: vec![],
                granted: vec![Missing::Automation],
            }
        );
    }

    #[test]
    fn failing_chrome_tabs_leave_meet_unknown() {
        let mut probe = fake(&[CHROME_BUNDLE]);
        probe.tabs = Err(ProbeError::Failed("script failed".into()));
        let detection = detect(&[built_in_rule(MEET_BUNDLE)], &probe, None);
        assert_eq!(
            detection.checks,
            vec![check("Meet", MEET_BUNDLE, unknown("script failed"))]
        );
    }

    #[test]
    fn a_custom_app_counts_while_it_runs_titled_by_its_first_cleaned_window() {
        let mut probe = fake(&[STANDUP]);
        probe.windows = Ok(strings(&["Standup - Zoom", "Other"]));
        let detection = detect(&[rule(STANDUP)], &probe, None);
        assert_eq!(
            detection.checks,
            vec![check("Standup", STANDUP, call("Standup"))]
        );
        assert_eq!(detection.granted, vec![Missing::Accessibility]);
    }

    #[test]
    fn a_custom_app_without_windows_is_titled_meeting() {
        let probe = fake(&[STANDUP]);
        assert_eq!(
            detect(&[rule(STANDUP)], &probe, None).checks,
            vec![check("Standup", STANDUP, call("Meeting"))]
        );
    }

    #[test]
    fn a_custom_app_that_is_not_running_does_not_count() {
        let probe = fake(&[]);
        assert_eq!(
            detect(&[rule(STANDUP)], &probe, None).checks,
            vec![check("Standup", STANDUP, Found::NotInCall)]
        );
    }

    #[test]
    fn a_custom_app_that_needs_the_microphone_counts_only_while_it_records() {
        let mut needing = rule(STANDUP);
        needing.needs_microphone = true;
        let cases = [
            (Some(Ok(true)), call("Meeting")),
            (Some(Ok(false)), Found::NotInCall),
            (None, Found::NotInCall),
            (
                Some(Err("pmset failed".to_string())),
                unknown("pmset failed"),
            ),
        ];
        for (microphone, expected) in cases {
            let mut probe = fake(&[STANDUP]);
            if let Some(answer) = microphone.clone() {
                probe.microphone.insert(STANDUP.into(), answer);
            }
            assert_eq!(
                detect(std::slice::from_ref(&needing), &probe, None).checks,
                vec![check("Standup", STANDUP, expected)],
                "{microphone:?}"
            );
        }
    }

    #[test]
    fn a_window_title_rule_needs_a_window_containing_it_ignoring_case() {
        let mut with_title = rule(STANDUP);
        with_title.window_title = Some("weekly".into());

        let mut hit = fake(&[STANDUP]);
        hit.windows = Ok(strings(&["Notes", "Team WEEKLY sync"]));
        assert_eq!(
            detect(std::slice::from_ref(&with_title), &hit, None).checks,
            vec![check("Standup", STANDUP, call("Team WEEKLY sync"))]
        );

        let mut miss = fake(&[STANDUP]);
        miss.windows = Ok(strings(&["Notes"]));
        assert_eq!(
            detect(&[with_title], &miss, None).checks,
            vec![check("Standup", STANDUP, Found::NotInCall)]
        );
    }

    #[test]
    fn a_window_title_that_only_repeats_the_rule_is_titled_meeting() {
        let mut with_title = rule(STANDUP);
        with_title.window_title = Some("Call".into());
        let mut probe = fake(&[STANDUP]);
        probe.windows = Ok(strings(&["call"]));
        assert_eq!(
            detect(&[with_title], &probe, None).checks,
            vec![check("Standup", STANDUP, call("Meeting"))]
        );
    }

    #[test]
    fn refused_window_titles_still_match_a_rule_without_a_window_title() {
        let mut probe = fake(&[STANDUP]);
        probe.windows = Err(ProbeError::Missing(Missing::Accessibility));
        let detection = detect(&[rule(STANDUP)], &probe, None);
        assert_eq!(
            detection,
            Detection {
                checks: vec![check("Standup", STANDUP, call("Meeting"))],
                missing: vec![Missing::Accessibility],
                granted: vec![],
            }
        );
    }

    #[test]
    fn refused_window_titles_never_match_a_rule_with_a_window_title() {
        let mut with_title = rule(STANDUP);
        with_title.window_title = Some("Weekly".into());
        let mut probe = fake(&[STANDUP]);
        probe.windows = Err(ProbeError::Missing(Missing::Accessibility));
        let detection = detect(&[with_title], &probe, None);
        assert_eq!(
            detection.checks,
            vec![check("Standup", STANDUP, Found::NotInCall)]
        );
        assert_eq!(detection.missing, vec![Missing::Accessibility]);
    }

    #[test]
    fn failing_window_titles_leave_a_custom_rule_unknown() {
        let mut probe = fake(&[STANDUP]);
        probe.windows = Err(ProbeError::Failed("ax failed".into()));
        assert_eq!(
            detect(&[rule(STANDUP)], &probe, None).checks,
            vec![check("Standup", STANDUP, unknown("ax failed"))]
        );
    }

    #[test]
    fn one_failing_detector_leaves_the_other_rules_answered() {
        let mut probe = fake(&[STANDUP, ZOOM_BUNDLE]);
        probe.windows = Err(ProbeError::Failed("ax failed".into()));
        let rules = vec![rule(STANDUP), built_in_rule(ZOOM_BUNDLE)];
        let detection = detect(&rules, &probe, None);
        assert_eq!(
            detection.checks,
            vec![
                check("Standup", STANDUP, unknown("ax failed")),
                check("Zoom", ZOOM_BUNDLE, Found::NotInCall),
            ]
        );
    }

    #[test]
    fn the_detection_error_is_the_first_unknown_answer() {
        let detection = Detection {
            checks: vec![
                check("Zoom", ZOOM_BUNDLE, Found::NotInCall),
                check("Teams", TEAMS_BUNDLE, unknown("pmset failed")),
                check("Meet", MEET_BUNDLE, unknown("script failed")),
            ],
            missing: vec![],
            granted: vec![],
        };
        assert_eq!(detection.error(), Some("pmset failed"));
    }

    #[test]
    fn a_detection_without_unknown_answers_has_no_error() {
        let detection = Detection {
            checks: vec![
                check("Zoom", ZOOM_BUNDLE, call("Meeting")),
                check("Teams", TEAMS_BUNDLE, Found::NotInCall),
            ],
            missing: vec![],
            granted: vec![],
        };
        assert_eq!(detection.error(), None);
        assert_eq!(Detection::default().error(), None);
    }
}
