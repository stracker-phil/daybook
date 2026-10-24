use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::SystemTime;

use serde_json::{Value, json};

use super::*;
use crate::store::settings::{DailySection, Feature, LinkedProject, Settings};
use crate::test_support::temp_config_dir;

const FIXTURE: &str = include_str!("fixtures/prototype-settings.json");

struct Folders {
    _prototype: tempfile::TempDir,
    _daybook: tempfile::TempDir,
    prototype: PathBuf,
    daybook: PathBuf,
}

/// A prototype folder with a `settings.json` and an empty Daybook folder.
fn folders(prototype_settings: &str) -> Folders {
    let prototype = temp_config_dir();
    let daybook = temp_config_dir();
    fs::write(prototype.path().join("settings.json"), prototype_settings).unwrap();
    Folders {
        prototype: prototype.path().to_path_buf(),
        daybook: daybook.path().to_path_buf(),
        _prototype: prototype,
        _daybook: daybook,
    }
}

fn run_import(folders: &Folders) -> Option<Import> {
    run(&folders.prototype, &folders.daybook)
}

fn imported(result: Option<Import>) -> ImportReport {
    match result {
        Some(Import::Imported(report)) => report,
        other => panic!("not imported: {other:?}"),
    }
}

fn json_of(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

/// Every file below `dir` with its bytes and modification time.
fn snapshot(dir: &Path) -> BTreeMap<String, (Vec<u8>, SystemTime)> {
    fn walk(root: &Path, dir: &Path, into: &mut BTreeMap<String, (Vec<u8>, SystemTime)>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, into);
            } else {
                let name = path.strip_prefix(root).unwrap().display().to_string();
                let modified = fs::metadata(&path).unwrap().modified().unwrap();
                into.insert(name, (fs::read(&path).unwrap(), modified));
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(dir, dir, &mut files);
    files
}

fn all_keys(value: &Value, into: &mut BTreeSet<String>) {
    match value {
        Value::Object(map) => {
            for (key, inner) in map {
                into.insert(key.clone());
                all_keys(inner, into);
            }
        }
        Value::Array(items) => items.iter().for_each(|item| all_keys(item, into)),
        _ => {}
    }
}

fn import_state(name: &str, prototype_state: &Value) -> Value {
    let folders = folders("{}");
    fs::write(folders.prototype.join(name), prototype_state.to_string()).unwrap();
    imported(run_import(&folders));
    let target = if name == "mode.json" {
        "context.json"
    } else {
        name
    };
    json_of(&folders.daybook.join(target))
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

mod settings_fixture {
    use super::*;

    fn import_fixture() -> (Folders, ImportReport, Settings) {
        let folders = folders(FIXTURE);
        let report = imported(run_import(&folders));
        let saved = JsonFile::new(folders.daybook.join("settings.json")).load::<Settings>();
        (folders, report, saved)
    }

    #[test]
    fn the_fixture_carries_the_tracking_keys_that_the_import_drops() {
        let prototype: Value = serde_json::from_str(FIXTURE).unwrap();
        let config = &prototype["config"];

        assert!(prototype["dailyNote"]["activity"].is_object());
        assert!(
            prototype["dailyNote"]["order"]
                .as_array()
                .unwrap()
                .contains(&json!("activity"))
        );
        assert!(config["activityFormat"]["activity"].is_string());
        assert!(config["activityFormat"]["working"].is_string());
        assert!(config["activityFormat"]["afk"].is_string());
        assert!(config["statusLogLines"].is_object());
        assert!(config["clients"][0]["color"].is_string());
        assert!(
            config["modes"][1]["features"]
                .as_array()
                .unwrap()
                .contains(&json!("tracking"))
        );
    }

    #[test]
    fn the_report_counts_what_was_imported() {
        let (_folders, report, _saved) = import_fixture();

        assert_eq!(
            report,
            ImportReport {
                contexts: 2,
                projects: 4,
                links: 5,
                routines: 2,
                journal_groups: 3,
                state_files: vec![],
                skipped: vec![],
                slack_token: false,
            }
        );
    }

    #[test]
    fn modes_become_contexts_and_tracking_becomes_status() {
        let (_folders, _report, saved) = import_fixture();

        let contexts: Vec<(&str, &Vec<Feature>)> = saved
            .contexts
            .iter()
            .map(|context| (context.id.as_str(), &context.features))
            .collect();
        assert_eq!(
            contexts,
            [
                ("private", &vec![Feature::Routines]),
                (
                    "work",
                    &vec![
                        Feature::Status,
                        Feature::Slack,
                        Feature::Jira,
                        Feature::Routines
                    ]
                ),
            ]
        );
        assert_eq!(saved.contexts[1].search_scope, "work");
        assert!(saved.contexts[1].auto_switch);
    }

    #[test]
    fn clients_become_projects_with_their_contexts_keywords_and_launcher_projects() {
        let (_folders, _report, saved) = import_fixture();

        let names: Vec<&str> = saved.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Internal", "Acme", "Globex", "Hobby"]);
        let globex = &saved.projects[2];
        assert_eq!(globex.contexts, strings(&["work"]));
        assert_eq!(globex.keywords, strings(&["GLX", "Globex"]));
        assert_eq!(globex.slack_emoji, ":globex:");
        assert_eq!(globex.launcher_projects.len(), 4);
        assert_eq!(
            globex.launcher_projects[0],
            LinkedProject {
                id: "proj-0003".into(),
                name: "Globex Payments".into()
            }
        );
        assert!(saved.projects[0].launcher_projects.is_empty());
        assert_eq!(saved.projects[3].contexts, strings(&["private"]));
    }

    #[test]
    fn links_and_routines_carry_the_old_modes_and_clients_as_contexts_and_projects() {
        let (_folders, _report, saved) = import_fixture();

        assert_eq!(saved.links[0].title, "Team: Weekly Planning");
        assert_eq!(saved.links[0].contexts, Some(strings(&["work"])));
        assert_eq!(saved.links[0].projects, None);
        assert_eq!(saved.links[4].projects, Some(strings(&["Globex"])));
        assert!(saved.links[4].pinned);
        assert_eq!(saved.links[0].event.as_deref(), Some("weekly planning"));
        let routines: Vec<(&str, &Vec<String>)> = saved
            .routines
            .iter()
            .map(|routine| (routine.id.as_str(), &routine.contexts))
            .collect();
        assert_eq!(
            routines,
            [
                ("morning", &strings(&["work"])),
                ("evening", &strings(&["work"]))
            ]
        );
        assert_eq!(saved.routines[0].steps.len(), 4);
    }

    #[test]
    fn the_journal_groups_keep_their_titles() {
        let (_folders, _report, saved) = import_fixture();

        let titles: Vec<&str> = saved.journal.iter().map(|g| g.title.as_str()).collect();
        assert_eq!(titles, ["Dawn", "Dusk", "Health"]);
        assert_eq!(saved.journal[2].callout, "HEART");
        assert!(saved.journal[2].folded);
    }

    #[test]
    fn status_messages_come_from_the_activity_format() {
        let (_folders, _report, saved) = import_fixture();

        assert_eq!(saved.status_messages.working, "re");
        assert_eq!(saved.status_messages.first_working, "good morning :wave:");
        assert_eq!(saved.status_messages.break_, "afk");
        assert_eq!(saved.status_messages.off, "off for today :wave:");
    }

    #[test]
    fn the_slack_channel_statuses_and_manual_workflows_move_into_slack() {
        let (_folders, _report, saved) = import_fixture();

        assert_eq!(saved.slack.channel, "#team-updates");
        assert_eq!(saved.slack.statuses.working.emoji, ":white_check_mark:");
        assert_eq!(saved.slack.statuses.working.text, "9:00 - 18:00");
        assert_eq!(saved.slack.statuses.break_.text, "AFK");
        assert_eq!(saved.slack.statuses.off.text, "Day's done");
        assert!(saved.slack.manual_workflows.working);
        assert!(!saved.slack.manual_workflows.break_);
        assert!(saved.slack.manual_workflows.off);
    }

    #[test]
    fn meeting_detection_and_rules_move_into_meetings() {
        let (_folders, _report, saved) = import_fixture();

        assert!(saved.meetings.detection);
        let apps: Vec<&str> = saved
            .meetings
            .rules
            .iter()
            .map(|r| r.app.as_str())
            .collect();
        assert_eq!(apps, ["Zoom", "Meet", "Teams"]);
        assert_eq!(
            saved.meetings.rules[0].window_title.as_deref(),
            Some("Zoom Meeting")
        );
    }

    #[test]
    fn search_scopes_and_excludes_move_into_search() {
        let (_folders, _report, saved) = import_fixture();

        let scopes: Vec<&str> = saved
            .search
            .scopes
            .iter()
            .map(|s| s.name.as_str())
            .collect();
        assert_eq!(
            scopes,
            ["journal", "private", "work", "hobby", "inbox", "all"]
        );
        assert_eq!(
            saved.search.exclude,
            strings(&["_/", ".obsidian/", ".trash/"])
        );
    }

    #[test]
    fn the_daily_note_groups_merge_without_the_activity_section() {
        let (_folders, _report, saved) = import_fixture();

        assert_eq!(saved.daily_note.fallback_folder, "1-Journal");
        assert_eq!(saved.daily_note.fallback_format, "YYYY-MM-DD");
        assert_eq!(saved.daily_note.notes, "## Notes");
        assert_eq!(saved.daily_note.plan.text, "Plan for Today");
        assert_eq!(
            saved.daily_note.order,
            [
                DailySection::Journal,
                DailySection::Focus,
                DailySection::Plan,
                DailySection::Notes
            ]
        );
    }

    #[test]
    fn vault_hotkeys_api_jira_and_tools_are_kept() {
        let (folders, _report, _saved) = import_fixture();
        let saved = json_of(&folders.daybook.join("settings.json"));

        assert_eq!(saved["vault"]["path"], json!("/Users/example/Vault"));
        assert_eq!(saved["vault"]["name"], json!("Example Vault"));
        assert_eq!(saved["hotkeys"]["hub"], json!("alt+cmd+enter"));
        assert_eq!(saved["hotkeys"]["toggleRail"], json!("f10"));
        assert_eq!(
            saved["api"]["token"],
            json!("fake-api-token-0123456789abcdef")
        );
        assert_eq!(saved["jira"]["defaultPrefix"], json!("QRS"));
        assert_eq!(saved["music"].as_array().unwrap().len(), 3);
        let search_tool = saved["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|tool| tool["id"] == json!("search"))
            .unwrap();
        assert_eq!(
            search_tool,
            &json!({ "id": "search", "rail": false, "window": true })
        );
    }

    #[test]
    fn the_rail_settings_are_kept() {
        let (folders, _report, _saved) = import_fixture();
        let saved = json_of(&folders.daybook.join("settings.json"));

        let rail = &saved["rail"];
        assert_eq!(rail["edges"], json!("both"));
        assert_eq!(rail["shape"], json!("docked"));
        assert_eq!(rail["theme"], json!("dark"));
        assert_eq!(rail["highlight"], json!("filled"));
        assert_eq!(rail["position"], json!("pointer"));
        assert_eq!(rail["showDelay"], json!(200));
        assert_eq!(rail["hideDelay"], json!(700));
        assert_eq!(rail["triggerWidth"], json!(6));
        assert_eq!(rail["captions"], json!(false));
    }

    #[test]
    fn nothing_of_time_tracking_is_saved() {
        let (folders, _report, _saved) = import_fixture();
        let saved = json_of(&folders.daybook.join("settings.json"));

        let mut keys = BTreeSet::new();
        all_keys(&saved, &mut keys);
        for dropped in [
            "activityFormat",
            "statusLogLines",
            "clients",
            "modes",
            "afk",
        ] {
            assert!(!keys.contains(dropped), "{dropped}");
        }
        assert!(saved["dailyNote"].get("activity").is_none());
        assert!(
            !saved["dailyNote"]["order"]
                .as_array()
                .unwrap()
                .contains(&json!("activity"))
        );
        for project in saved["projects"].as_array().unwrap() {
            assert!(project.get("color").is_none(), "{project}");
        }
    }
}

mod state_files {
    use super::*;

    #[test]
    fn the_active_context_is_copied_as_context_json() {
        let state = import_state(
            "mode.json",
            &json!({ "id": "work", "setAt": "2026-10-24T08:00:00Z" }),
        );

        assert_eq!(
            state,
            json!({ "id": "work", "setAt": "2026-10-24T08:00:00Z" })
        );
    }

    #[test]
    fn an_empty_context_becomes_null() {
        let state = import_state("mode.json", &json!({ "id": "", "setAt": "" }));

        assert_eq!(state, json!({ "id": null, "setAt": null }));
    }

    #[test]
    fn the_status_memory_date_becomes_iso() {
        let state = import_state(
            "status-memory.json",
            &json!({
                "date": "Sat Oct 24 2026",
                "status": "working",
                "since": "2026-10-24T08:00:00Z",
                "workedToday": true
            }),
        );

        assert_eq!(
            state,
            json!({
                "date": "2026-10-24",
                "status": "working",
                "since": "2026-10-24T08:00:00Z",
                "workedToday": true
            })
        );
    }

    #[test]
    fn a_status_memory_with_unknown_values_is_repaired() {
        let state = import_state(
            "status-memory.json",
            &json!({ "date": "yesterday", "status": "sleeping", "since": "" }),
        );

        assert_eq!(
            state,
            json!({ "date": "", "status": null, "since": null, "workedToday": false })
        );
    }

    #[test]
    fn jira_tickets_keep_their_days_and_drop_what_is_not_a_list() {
        let state = import_state(
            "jira-tickets.json",
            &json!({
                "Sat Oct 24 2026": ["QRS-1", "QRS-2"],
                "Fri Oct 23 2026": ["QRS-3"],
                "broken": "QRS-4"
            }),
        );

        assert_eq!(
            state,
            json!({
                "Sat Oct 24 2026": ["QRS-1", "QRS-2"],
                "Fri Oct 23 2026": ["QRS-3"]
            })
        );
    }

    #[test]
    fn fired_routines_keep_their_date_as_it_was() {
        let state = import_state(
            "routines.json",
            &json!({ "date": "Sat Oct 24 2026", "fired": ["morning", "evening"] }),
        );

        assert_eq!(
            state,
            json!({ "date": "Sat Oct 24 2026", "fired": ["morning", "evening"] })
        );
    }

    #[test]
    fn the_day_message_date_becomes_iso() {
        let state = import_state(
            "day-message.json",
            &json!({
                "date": "Sat Oct 24 2026",
                "channel": "C0000001",
                "ts": "1761292800.000100",
                "greeting": "good morning :wave:",
                "eodSent": true
            }),
        );

        assert_eq!(
            state,
            json!({
                "date": "2026-10-24",
                "channel": "C0000001",
                "ts": "1761292800.000100",
                "greeting": "good morning :wave:",
                "eodSent": true
            })
        );
    }

    #[test]
    fn the_shown_slack_status_keeps_known_presence_and_expiry() {
        let state = import_state(
            "slack-shown.json",
            &json!({
                "emoji": ":spiral_calendar_pad:",
                "text": "In a Zoom Call",
                "presence": "away",
                "expiresAt": 1761300000000.0
            }),
        );

        assert_eq!(
            state,
            json!({
                "emoji": ":spiral_calendar_pad:",
                "text": "In a Zoom Call",
                "presence": "away",
                "expiresAt": 1761300000000.0
            })
        );
    }

    #[test]
    fn an_unknown_slack_presence_becomes_null() {
        let state = import_state(
            "slack-shown.json",
            &json!({ "emoji": ":x:", "text": "t", "presence": "keep" }),
        );

        assert_eq!(
            state,
            json!({ "emoji": ":x:", "text": "t", "presence": null, "expiresAt": null })
        );
    }

    #[test]
    fn pending_entries_lose_the_note_changes_that_only_wrote_the_activity_log() {
        let kept = |file: &str, created_at: u32, kind: &str| json!({ "file": file, "createdAt": created_at, "op": { "kind": kind } });
        let state = import_state(
            "pending-entries.json",
            &json!({
                "entries": [
                    kept("a.md", 1, "append"),
                    kept("b.md", 2, "activity"),
                    kept("c.md", 3, "status"),
                    kept("d.md", 4, "meeting"),
                    kept("e.md", 5, "journal"),
                    { "createdAt": 6, "op": { "kind": "append" } }
                ]
            }),
        );

        assert_eq!(
            state,
            json!({ "entries": [kept("a.md", 1, "append"), kept("e.md", 5, "journal")] })
        );
    }

    #[test]
    fn every_present_state_file_is_written_under_its_daybook_name() {
        let folders = folders("{}");
        for name in [
            "mode.json",
            "status-memory.json",
            "jira-tickets.json",
            "routines.json",
            "day-message.json",
            "slack-shown.json",
            "pending-entries.json",
        ] {
            fs::write(folders.prototype.join(name), "{}").unwrap();
        }

        let report = imported(run_import(&folders));

        assert_eq!(
            report.state_files,
            strings(&[
                "context.json",
                "status-memory.json",
                "jira-tickets.json",
                "routines.json",
                "day-message.json",
                "slack-shown.json",
                "pending-entries.json"
            ])
        );
        assert!(report.skipped.is_empty());
        for name in &report.state_files {
            assert!(folders.daybook.join(name).is_file(), "{name}");
        }
    }

    #[test]
    fn missing_state_files_are_not_copied() {
        let folders = folders("{}");
        fs::write(folders.prototype.join("routines.json"), "{}").unwrap();

        let report = imported(run_import(&folders));

        assert_eq!(report.state_files, strings(&["routines.json"]));
        assert!(report.skipped.is_empty());
        let mut files: Vec<String> = snapshot(&folders.daybook).into_keys().collect();
        files.sort();
        assert_eq!(files, ["routines.json", "settings.json"]);
    }

    #[test]
    fn an_existing_daybook_state_file_is_kept_and_reported_as_skipped() {
        let folders = folders("{}");
        fs::write(folders.prototype.join("day-message.json"), "{}").unwrap();
        fs::write(folders.prototype.join("routines.json"), "{}").unwrap();
        fs::write(folders.daybook.join("day-message.json"), "mine").unwrap();

        let report = imported(run_import(&folders));

        assert_eq!(
            fs::read_to_string(folders.daybook.join("day-message.json")).unwrap(),
            "mine"
        );
        assert_eq!(report.state_files, strings(&["routines.json"]));
        assert_eq!(report.skipped.len(), 1);
        assert!(
            report.skipped[0].contains("day-message.json"),
            "{:?}",
            report.skipped
        );
        assert!(folders.daybook.join("settings.json").is_file());
    }

    #[test]
    fn a_state_file_with_invalid_json_is_skipped_and_the_rest_is_imported() {
        let folders = folders("{}");
        fs::write(folders.prototype.join("routines.json"), "{ nope").unwrap();
        fs::write(folders.prototype.join("mode.json"), r#"{"id":"work"}"#).unwrap();

        let report = imported(run_import(&folders));

        assert_eq!(report.state_files, strings(&["context.json"]));
        assert_eq!(report.skipped.len(), 1);
        assert!(
            report.skipped[0].contains("routines.json"),
            "{:?}",
            report.skipped
        );
        assert!(!folders.daybook.join("routines.json").exists());
        assert!(folders.daybook.join("settings.json").is_file());
    }
}

mod when_it_runs {
    use super::*;

    #[test]
    fn a_second_run_does_nothing() {
        let folders = folders(FIXTURE);
        fs::write(folders.prototype.join("mode.json"), r#"{"id":"work"}"#).unwrap();
        imported(run_import(&folders));
        let after_first = snapshot(&folders.daybook);

        assert_eq!(run_import(&folders), None);

        assert_eq!(snapshot(&folders.daybook), after_first);
    }

    #[test]
    fn an_existing_daybook_settings_file_means_nothing_is_imported() {
        let folders = folders(FIXTURE);
        fs::write(folders.prototype.join("mode.json"), r#"{"id":"work"}"#).unwrap();
        fs::write(folders.daybook.join("settings.json"), "mine").unwrap();

        assert_eq!(run_import(&folders), None);

        let files: Vec<String> = snapshot(&folders.daybook).into_keys().collect();
        assert_eq!(files, ["settings.json"]);
        assert_eq!(
            fs::read_to_string(folders.daybook.join("settings.json")).unwrap(),
            "mine"
        );
    }

    #[test]
    fn a_missing_prototype_folder_imports_nothing() {
        let daybook = temp_config_dir();
        let missing = daybook.path().join("no-prototype");

        assert_eq!(run(&missing, daybook.path()), None);

        assert!(snapshot(daybook.path()).is_empty());
    }

    #[test]
    fn a_prototype_folder_without_settings_imports_nothing() {
        let prototype = temp_config_dir();
        let daybook = temp_config_dir();
        fs::write(prototype.path().join("mode.json"), r#"{"id":"work"}"#).unwrap();

        assert_eq!(run(prototype.path(), daybook.path()), None);

        assert!(snapshot(daybook.path()).is_empty());
    }

    #[test]
    fn invalid_prototype_settings_fail_without_writing_settings_so_it_is_tried_again() {
        let folders = folders("{ not json");
        fs::write(folders.prototype.join("mode.json"), r#"{"id":"work"}"#).unwrap();

        assert!(matches!(run_import(&folders), Some(Import::Failed(_))));

        assert!(!folders.daybook.join("settings.json").exists());
        fs::write(folders.prototype.join("settings.json"), "{}").unwrap();
        imported(run_import(&folders));
        assert!(folders.daybook.join("settings.json").is_file());
    }

    #[test]
    fn the_prototype_folder_is_unchanged_by_an_import() {
        let folders = folders(FIXTURE);
        fs::write(folders.prototype.join("mode.json"), r#"{"id":"work"}"#).unwrap();
        fs::write(
            folders.prototype.join("day-message.json"),
            r#"{"date":"Sat Oct 24 2026"}"#,
        )
        .unwrap();
        fs::write(folders.prototype.join("slack-token.bin"), [1u8, 2, 3]).unwrap();
        let before = snapshot(&folders.prototype);

        imported(run_import(&folders));

        assert_eq!(snapshot(&folders.prototype), before);
    }
}

mod slack_token {
    use super::*;

    #[test]
    fn an_encrypted_slack_token_is_reported_and_has_to_be_entered_again() {
        let folders = folders(FIXTURE);
        fs::write(folders.prototype.join("slack-token.bin"), [9u8; 8]).unwrap();

        let report = imported(run_import(&folders));

        assert!(report.slack_token);
        assert!(report.notice().contains("Slack"), "{}", report.notice());
        assert!(report.log_text().contains("Slack"), "{}", report.log_text());
    }

    #[test]
    fn without_a_slack_token_the_notice_does_not_mention_slack() {
        let folders = folders(FIXTURE);

        let report = imported(run_import(&folders));

        assert!(!report.slack_token);
        assert!(!report.notice().contains("Slack"), "{}", report.notice());
        assert!(
            !report.log_text().contains("Slack"),
            "{}",
            report.log_text()
        );
    }

    #[test]
    fn the_notice_names_the_counts() {
        let folders = folders(FIXTURE);

        let notice = imported(run_import(&folders)).notice();

        for count in [
            "2 contexts",
            "4 projects",
            "5 links",
            "2 routines",
            "3 journal groups",
        ] {
            assert!(notice.contains(count), "{count} in {notice}");
        }
    }

    #[test]
    fn a_single_item_is_not_pluralized() {
        let report = ImportReport {
            contexts: 1,
            projects: 1,
            links: 1,
            routines: 1,
            journal_groups: 1,
            state_files: vec![],
            skipped: vec![],
            slack_token: false,
        };

        let notice = report.notice();

        for count in [
            "1 context,",
            "1 project,",
            "1 link,",
            "1 routine,",
            "1 journal group.",
        ] {
            assert!(notice.contains(count), "{count} in {notice}");
        }
    }

    #[test]
    fn the_log_lists_state_files_and_what_was_skipped() {
        let report = ImportReport {
            contexts: 0,
            projects: 0,
            links: 0,
            routines: 0,
            journal_groups: 0,
            state_files: strings(&["context.json"]),
            skipped: strings(&["day-message.json: Daybook already has day-message.json"]),
            slack_token: false,
        };

        let text = report.log_text();

        assert!(text.contains("context.json"), "{text}");
        assert!(
            text.contains("day-message.json: Daybook already has day-message.json"),
            "{text}"
        );
    }
}
