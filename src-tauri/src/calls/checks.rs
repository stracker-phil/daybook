//! Reading what the detectors see: call titles, microphone users, running apps. Pure.
//!
//! Zoom runs its CptHost process only during a meeting, Teams uses the microphone during a call (and shows a
//! level-19 window while it isn't in front, on any Space), and a Google Meet meeting has a
//! `xxx-xxxx-xxx` link in a browser tab (the Meet app's windows are Chrome windows too) while that browser
//! holds a WebRTC connection or records from the microphone.

use std::collections::HashSet;
use std::sync::LazyLock;

use regex::Regex;
use serde::Deserialize;

pub use crate::store::settings::{MEET_BUNDLE, TEAMS_BUNDLE, ZOOM_BUNDLE};

pub const CHROME_BUNDLE: &str = "com.google.Chrome";
pub const ARC_BUNDLE: &str = "company.thebrowser.Browser";
/// The browsers a Meet call can run in, in the order they're asked; Chrome includes the Meet app.
pub const MEET_BROWSERS: [&str; 2] = [CHROME_BUNDLE, ARC_BUNDLE];
/// The title of a call whose app doesn't name it.
pub const GENERIC_TITLE: &str = "Meeting";

/// What a built-in check looks for, as Settings describes it; `None` for an app without one.
pub fn built_in_check(bundle_id: &str) -> Option<&'static str> {
    match bundle_id {
        ZOOM_BUNDLE => Some("A call while Zoom’s meeting process (CptHost) runs."),
        TEAMS_BUNDLE => {
            Some("A call while Teams uses the microphone or shows its call window, on any Space.")
        }
        MEET_BUNDLE => Some(
            "A call while a Chrome or Arc tab or the Meet app shows a meeting link (meet.google.com/…/xxx-xxxx-xxx) and that browser has a call connection open or uses the microphone.",
        ),
        _ => None,
    }
}

fn regex(pattern: &str) -> Regex {
    Regex::new(pattern).expect("valid regex")
}

const MEET_CODE_PATTERN: &str = "[a-z]{3}-[a-z]{4}-[a-z]{3}";
/// The code may follow other path segments (`meet.google.com/meet/xxx-xxxx-xxx`).
static MEET_CALL_URL: LazyLock<Regex> = LazyLock::new(|| {
    regex(&format!(
        r"(?:^|[/.])meet\.google\.com/(?:[^/?#\s]+/)*{MEET_CODE_PATTERN}(?:[/?#]|$)"
    ))
});
static MEET_CODE: LazyLock<Regex> = LazyLock::new(|| regex(&format!("^{MEET_CODE_PATTERN}$")));
static MEET_PREFIX: LazyLock<Regex> = LazyLock::new(|| regex(r"^Meet\s*[-–]\s*"));
static APP_SUFFIX: LazyLock<Regex> =
    LazyLock::new(|| regex(r"(?i)\s*[|–-]\s*(Microsoft Teams|Zoom|Google Meet)\s*$"));
static ASSERTION_START: LazyLock<Regex> = LazyLock::new(|| regex(r"(?m)^\s*pid \d+\("));
static ASSERTION_OWNER: LazyLock<Regex> = LazyLock::new(|| regex(r"^\s*pid (\d+)\("));
static CREATED_FOR: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i)Created for PID:\s*(\d+)"));
static AUDIO_IN: LazyLock<Regex> = LazyLock::new(|| regex(r"(?i)audio-in"));
const WEBRTC_ASSERTION: &str = "WebRTC has active PeerConnections";
static BUNDLE_ID: LazyLock<Regex> = LazyLock::new(|| regex(r#"bundleID="([^"]+)""#));
static BUNDLE_PATH: LazyLock<Regex> = LazyLock::new(|| regex(r#"="(/[^"]+\.app)""#));

/// The title of the first tab in a Meet meeting ("Meet – Weekly" → "Weekly"); `None` without one.
pub fn meet_call_title(urls: &[String], titles: &[String]) -> Option<String> {
    let index = urls.iter().position(|url| MEET_CALL_URL.is_match(url))?;
    let title = titles.get(index).map_or("", String::as_str);
    let title = MEET_PREFIX.replace(title, "").trim().to_string();
    Some(if title.is_empty() || MEET_CODE.is_match(&title) {
        GENERIC_TITLE.to_string()
    } else {
        title
    })
}

/// `Weekly | Microsoft Teams` → `Weekly`; a title that only repeats the rule's window title → `Meeting`.
pub fn clean_title(title: &str, window_title: Option<&str>) -> String {
    let cleaned = APP_SUFFIX.replace(title, "").trim().to_string();
    let repeats_rule = window_title.is_some_and(|wanted| cleaned.eq_ignore_ascii_case(wanted));
    if cleaned.is_empty() || repeats_rule {
        GENERIC_TITLE.to_string()
    } else {
        cleaned
    }
}

/// A calendar event running during the call names a call its app calls just "Meeting".
pub fn call_title(detected: &str, event_title: Option<&str>) -> String {
    match event_title {
        Some(event) if detected == GENERIC_TITLE && !event.trim().is_empty() => event.to_string(),
        _ => detected.to_string(),
    }
}

/// Processes with a running audio input, from `pmset -g assertions`: coreaudiod holds an assertion per
/// audio session and names the process it is for (`Created for PID: 812.`, `Resources: audio-in`). The
/// owner of such an assertion counts too.
pub fn microphone_pids(assertions: &str) -> Vec<u32> {
    let mut pids: Vec<u32> = assertion_blocks(assertions)
        .filter(|block| AUDIO_IN.is_match(block))
        .flat_map(pids_in_block)
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

/// Processes with an open WebRTC connection, from `pmset -g assertions`: Chromium browsers hold
/// `WebRTC has active PeerConnections` for as long as a call runs, also while muted.
pub fn webrtc_pids(assertions: &str) -> Vec<u32> {
    let mut pids: Vec<u32> = assertion_blocks(assertions)
        .filter(|block| block.contains(WEBRTC_ASSERTION))
        .filter_map(|block| ASSERTION_OWNER.captures(block)?[1].parse().ok())
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

/// Each assertion starts with `pid N(owner):`; its details follow on indented lines.
fn assertion_blocks(assertions: &str) -> impl Iterator<Item = &str> {
    let starts: Vec<usize> = ASSERTION_START
        .find_iter(assertions)
        .map(|m| m.start())
        .collect();
    let ends: Vec<usize> = starts
        .iter()
        .skip(1)
        .copied()
        .chain([assertions.len()])
        .collect();
    starts
        .into_iter()
        .zip(ends)
        .map(move |(start, end)| &assertions[start..end])
}

fn pids_in_block(block: &str) -> Vec<u32> {
    let owner = ASSERTION_OWNER.captures(block).map(|c| c[1].to_string());
    let created_for = CREATED_FOR.captures_iter(block).map(|c| c[1].to_string());
    owner
        .into_iter()
        .chain(created_for)
        .filter_map(|pid| pid.parse().ok())
        .collect()
}

/// Whether one of the executables (`ps -o comm=` lines) lies inside the app bundle at `bundle_path`.
pub fn runs_from_bundle(executables: &str, bundle_path: &str) -> bool {
    let prefix = format!("{}/", bundle_path.trim_end_matches('/'));
    executables
        .lines()
        .any(|line| line.trim().starts_with(&prefix))
}

/// The bundle ids in `lsappinfo list`.
pub fn running_bundle_ids(list: &str) -> HashSet<String> {
    BUNDLE_ID
        .captures_iter(list)
        .map(|c| c[1].to_string())
        .collect()
}

/// The app's path in `lsappinfo info -only bundlepath -app <id>`.
pub fn bundle_path(info: &str) -> Option<String> {
    BUNDLE_PATH.captures(info).map(|c| c[1].to_string())
}

#[derive(Deserialize)]
struct BrowserTabs {
    #[serde(default)]
    urls: Vec<Vec<Option<String>>>,
    #[serde(default)]
    titles: Vec<Vec<Option<String>>>,
}

fn flat(windows: Vec<Vec<Option<String>>>) -> Vec<String> {
    windows
        .into_iter()
        .flatten()
        .map(Option::unwrap_or_default)
        .collect()
}

/// The URLs and titles of every tab from the tabs script's JSON (`{}` while the browser isn't running);
/// anything else is an error, never "no tabs".
pub fn parse_tabs(output: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let output = output.trim();
    // serde would also read a JSON array as the struct's fields in order.
    if !output.starts_with('{') {
        return Err(format!("The browser's tabs couldn't be read: {output:.80}"));
    }
    let tabs: BrowserTabs = serde_json::from_str(output)
        .map_err(|e| format!("The browser's tabs couldn't be read: {e}"))?;
    Ok((flat(tabs.urls), flat(tabs.titles)))
}

/// What a refused script says: -1743 is a refused Automation request; either spelling of the text.
pub fn automation_refused(stderr: &str) -> bool {
    let lower = stderr.to_lowercase();
    stderr.contains("-1743")
        || lower.contains("not authorized to send apple events")
        || lower.contains("not authorised to send apple events")
}

#[cfg(test)]
mod tests {
    use super::*;

    const LSAPPINFO_LIST: &str = include_str!("fixtures/lsappinfo-list.txt");
    const LSAPPINFO_BUNDLEPATH: &str = include_str!("fixtures/lsappinfo-bundlepath.txt");
    const PMSET_ASSERTIONS: &str = include_str!("fixtures/pmset-assertions.txt");
    const PS_COMM: &str = include_str!("fixtures/ps-comm.txt");

    const ASSERTIONS: &str = "Assertion status system-wide:
   PreventUserIdleSystemSleep     1
Listed by owning process:
   pid 1023(sharingd): [0x000078cb00018154] 00:01:43 PreventUserIdleSystemSleep named: \"Handoff\"
   pid 393(coreaudiod): [0x0000a8f200019c5e] 00:12:05 PreventUserIdleSleep named: \"com.apple.audio.BuiltInMicrophoneDevice.context.preventuseridlesleep\"
\tCreated for PID: 4711.
\tResources: audio-in
   pid 393(coreaudiod): [0x0000a8f200019c5f] 00:12:05 PreventUserIdleSleep named: \"com.apple.audio.context9.preventuseridlesleep\"
\tCreated for PID: 812.
\tResources: audio-out
Kernel Assertions: 0x104=USB,MAGICWAKE";

    fn strings(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn a_meet_home_page_is_not_a_call() {
        let urls = strings(&["https://meet.google.com/landing", "https://example.com"]);
        let titles = strings(&["Meet", "Example"]);
        assert_eq!(meet_call_title(&urls, &titles), None);
        assert_eq!(meet_call_title(&[], &[]), None);
    }

    #[test]
    fn a_meet_call_takes_the_title_of_the_meeting_tab() {
        let urls = strings(&[
            "https://mail.google.com",
            "https://meet.google.com/abc-defg-hij?authuser=0",
        ]);
        let titles = strings(&["Inbox", "Meet – Weekly sync"]);
        assert_eq!(meet_call_title(&urls, &titles), Some("Weekly sync".into()));

        let urls = strings(&["https://x", "https://meet.google.com/abc-defg-hij"]);
        let titles = strings(&["X", "Meet – Weekly"]);
        assert_eq!(meet_call_title(&urls, &titles), Some("Weekly".into()));
    }

    #[test]
    fn a_meet_code_after_other_path_segments_is_a_call() {
        let urls = strings(&["https://meet.google.com/meet/rxp-bzjc-eee?lfhs=2"]);
        let titles = strings(&["Meet - Knowledge Sharing Session - Squad P"]);
        assert_eq!(
            meet_call_title(&urls, &titles),
            Some("Knowledge Sharing Session - Squad P".into())
        );
    }

    #[test]
    fn a_meet_code_may_end_at_a_slash_a_fragment_or_the_url() {
        for url in [
            "https://meet.google.com/abc-defg-hij/",
            "https://meet.google.com/abc-defg-hij#x",
            "https://meet.google.com/abc-defg-hij",
            "meet.google.com/abc-defg-hij",
        ] {
            assert_eq!(
                meet_call_title(&strings(&[url]), &strings(&["Meet - Weekly"])),
                Some("Weekly".into()),
                "{url}"
            );
        }
    }

    #[test]
    fn a_longer_code_like_segment_or_another_host_is_not_a_meet_call() {
        for url in [
            "https://meet.google.com/abc-defg-hijk",
            "https://meet.google.com/abc-defg-hij-klm",
            "https://example.com/abc-defg-hij",
            "https://notmeet.google.com/abc-defg-hij",
        ] {
            assert_eq!(
                meet_call_title(&strings(&[url]), &strings(&["Meet - Weekly"])),
                None,
                "{url}"
            );
        }
    }

    #[test]
    fn a_meet_call_with_a_bare_code_or_no_title_is_a_generic_meeting() {
        let urls = strings(&["https://meet.google.com/abc-defg-hij"]);
        assert_eq!(
            meet_call_title(&urls, &strings(&["Meet - abc-defg-hij"])),
            Some("Meeting".into())
        );
        assert_eq!(meet_call_title(&urls, &[]), Some("Meeting".into()));
    }

    #[test]
    fn clean_title_drops_the_apps_name() {
        for (title, expected) in [
            ("Weekly | Microsoft Teams", "Weekly"),
            ("Zoom", "Zoom"),
            ("Standup - Zoom", "Standup"),
            ("", "Meeting"),
        ] {
            assert_eq!(clean_title(title, None), expected, "{title}");
        }
    }

    #[test]
    fn a_title_repeating_the_rules_window_title_is_a_generic_meeting() {
        assert_eq!(clean_title("call window", Some("Call Window")), "Meeting");
        assert_eq!(clean_title("Weekly", Some("Call Window")), "Weekly");
    }

    #[test]
    fn a_calendar_event_names_a_generic_meeting() {
        assert_eq!(call_title("Meeting", Some("Planning")), "Planning");
    }

    #[test]
    fn a_real_title_is_never_replaced_by_a_calendar_event() {
        assert_eq!(call_title("Weekly", Some("Planning")), "Weekly");
    }

    #[test]
    fn without_a_usable_event_title_the_generic_meeting_stays() {
        assert_eq!(call_title("Meeting", None), "Meeting");
        assert_eq!(call_title("Meeting", Some("  ")), "Meeting");
        assert_eq!(call_title("Meeting", Some("")), "Meeting");
    }

    #[test]
    fn microphone_pids_are_the_owner_and_target_of_audio_in_assertions() {
        assert_eq!(microphone_pids(ASSERTIONS), vec![393, 4711]);
    }

    const WEBRTC_ASSERTIONS: &str = "Listed by owning process:
   pid 1319(Google Chrome): [0x00012dc200019ddf] 00:18:29 NoIdleSleepAssertion named: \"WebRTC has active PeerConnections\"
   pid 1319(Google Chrome): [0x00012dc200019de0] 00:18:29 NoIdleSleepAssertion named: \"Playing audio\"
   pid 2200(Google Chrome Helper): [0x00012dc200019de1] 00:01:00 NoIdleSleepAssertion named: \"Video Wake Lock\"
   pid 393(coreaudiod): [0x0000a8f200019c5e] 00:12:05 PreventUserIdleSleep named: \"com.apple.audio.context.preventuseridlesleep\"
\tCreated for PID: 4711.
\tResources: audio-in
   pid 77(Arc): [0x00012dc200019de2] 00:00:10 NoIdleSleepAssertion named: \"WebRTC has active PeerConnections\"
   pid 1319(Google Chrome): [0x00012dc200019de3] 00:00:05 NoIdleSleepAssertion named: \"WebRTC has active PeerConnections\"
Kernel Assertions: 0x104=USB,MAGICWAKE";

    #[test]
    fn webrtc_pids_are_the_sorted_owners_of_peer_connection_assertions() {
        assert_eq!(webrtc_pids(WEBRTC_ASSERTIONS), vec![77, 1319]);
    }

    #[test]
    fn other_assertions_and_audio_in_targets_are_not_webrtc_pids() {
        let assertions = "Listed by owning process:
   pid 1319(Google Chrome): [0x1] 00:18:29 NoIdleSleepAssertion named: \"Playing audio\"
   pid 1319(Google Chrome): [0x2] 00:18:29 NoIdleSleepAssertion named: \"Video Wake Lock\"
   pid 393(coreaudiod): [0x3] 00:12:05 PreventUserIdleSleep named: \"x\"
\tCreated for PID: 4711.
\tResources: audio-in";
        assert_eq!(webrtc_pids(assertions), Vec::<u32>::new());
    }

    #[test]
    fn empty_assertions_give_no_webrtc_pids() {
        assert_eq!(webrtc_pids(""), Vec::<u32>::new());
    }

    #[test]
    fn assertions_without_audio_input_give_no_microphone_pids() {
        assert_eq!(
            microphone_pids("Listed by owning process:\n   pid 1(x): Handoff"),
            Vec::<u32>::new()
        );
        assert_eq!(microphone_pids(""), Vec::<u32>::new());
    }

    #[test]
    fn the_real_pmset_output_with_only_audio_out_has_no_microphone_pids() {
        assert_eq!(microphone_pids(PMSET_ASSERTIONS), Vec::<u32>::new());
    }

    #[test]
    fn helpers_inside_the_apps_bundle_count_as_running_from_it() {
        let teams = "/Applications/Microsoft Teams.app";
        let helper = format!(
            "{teams}/Contents/Helpers/Microsoft Teams WebView.app/Contents/MacOS/Microsoft Teams WebView"
        );
        assert!(runs_from_bundle(&helper, teams));
        assert!(runs_from_bundle(
            &format!("/usr/sbin/coreaudiod\n{helper}\n"),
            teams
        ));
    }

    #[test]
    fn a_process_outside_the_bundle_or_in_a_longer_named_one_does_not_count() {
        let teams = "/Applications/Microsoft Teams.app";
        let executables = "/usr/sbin/coreaudiod\n/Applications/Microsoft Teams Classic.app/x";
        assert!(!runs_from_bundle(executables, teams));
        assert!(!runs_from_bundle("", teams));
    }

    #[test]
    fn the_real_ps_output_runs_from_control_center_but_not_from_a_prefix_of_it() {
        assert!(runs_from_bundle(
            PS_COMM,
            "/System/Library/CoreServices/ControlCenter.app"
        ));
        assert!(!runs_from_bundle(
            PS_COMM,
            "/System/Library/CoreServices/Control.app"
        ));
    }

    #[test]
    fn running_bundle_ids_skip_apps_without_one() {
        let expected: HashSet<String> = [
            "com.apple.loginwindow",
            "com.apple.PressAndHold",
            "com.apple.WindowManager",
            "com.apple.controlcenter",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        assert_eq!(running_bundle_ids(LSAPPINFO_LIST), expected);
    }

    #[test]
    fn the_bundle_path_is_read_from_the_apps_info() {
        assert_eq!(
            bundle_path(LSAPPINFO_BUNDLEPATH),
            Some("/System/Library/CoreServices/Finder.app".to_string())
        );
        assert_eq!(bundle_path(""), None);
    }

    #[test]
    fn automation_refusals_are_recognized_by_code_or_message() {
        for (stderr, expected) in [
            (
                "execution error: Not authorized to send Apple events to Google Chrome. (-1743)",
                true,
            ),
            ("execution error: (-1743)", true),
            (
                "execution error: not authorized to send apple events to Teams.",
                true,
            ),
            ("Not authorised to send Apple events", true),
            ("execution error: Can’t get window 1. (-1728)", false),
            ("", false),
        ] {
            assert_eq!(automation_refused(stderr), expected, "{stderr}");
        }
    }

    #[test]
    fn browser_not_running_gives_no_tabs() {
        assert_eq!(parse_tabs("{}"), Ok((vec![], vec![])));
        assert_eq!(parse_tabs("  {}\n"), Ok((vec![], vec![])));
    }

    #[test]
    fn browser_tabs_of_all_windows_are_flattened_in_order() {
        let output =
            r#"{"urls":[["https://a","https://b"],["https://c"]],"titles":[["A","B"],["C"]]}"#;
        assert_eq!(
            parse_tabs(output),
            Ok((
                strings(&["https://a", "https://b", "https://c"]),
                strings(&["A", "B", "C"])
            ))
        );
    }

    #[test]
    fn null_tab_entries_become_empty_strings() {
        let output = r#"{"urls":[["https://a",null]],"titles":[[null,"B"]]}"#;
        assert_eq!(
            parse_tabs(output),
            Ok((strings(&["https://a", ""]), strings(&["", "B"])))
        );
    }

    #[test]
    fn output_that_is_not_the_tabs_json_is_an_error_not_no_tabs() {
        for output in ["", "Arc is not running", r#"{"urls":"x"}"#, "[]", "{"] {
            assert!(
                matches!(
                    parse_tabs(output),
                    Err(message) if message.starts_with("The browser's tabs couldn't be read")
                ),
                "{output}"
            );
        }
    }

    #[test]
    fn zoom_teams_and_meet_have_a_built_in_check() {
        for bundle in [ZOOM_BUNDLE, TEAMS_BUNDLE, MEET_BUNDLE] {
            assert!(built_in_check(bundle).is_some(), "{bundle}");
        }
    }

    #[test]
    fn another_app_has_no_built_in_check() {
        assert_eq!(built_in_check("com.apple.finder"), None);
    }
}
