//! The repair rules of `Settings::from_json`: the prototype's normalizer tests ported key by key, and
//! the rules each group of the settings keeps.

use super::*;
use serde_json::json;

fn load(raw: Value) -> Settings {
    Settings::from_json(&raw)
}

fn rail(raw: Value) -> Rail {
    load(json!({ "rail": raw })).rail
}

fn tool_ids(settings: &Settings) -> Vec<ToolId> {
    settings.tools.iter().map(|tool| tool.id).collect()
}

fn default_tool_ids() -> Vec<ToolId> {
    default_tools().iter().map(|tool| tool.id).collect()
}

mod config_normalize_port {
    use super::*;

    #[test]
    fn broken_projects_fall_back_and_a_negative_check_in_interval_resets() {
        let settings = load(json!({
            "projects": "nope",
            "contexts": [{ "name": "Work", "checkInMinutes": -3 }]
        }));

        assert_eq!(settings.projects, defaults::projects());
        assert_eq!(settings.contexts[0].check_in_minutes, 15);
    }

    #[test]
    fn routines_without_a_name_are_dropped_and_the_others_get_an_id() {
        let settings = load(json!({
            "routines": [
                {
                    "name": "Evening Wrap",
                    "steps": [{ "title": "" }, { "title": "Timesheet", "link": " https://x " }]
                },
                { "name": "" }
            ]
        }));

        assert_eq!(settings.routines.len(), 1);
        assert_eq!(settings.routines[0].id, "evening-wrap");
        assert_eq!(
            settings.routines[0].steps,
            vec![RoutineStep {
                title: "Timesheet".into(),
                link: "https://x".into(),
                days: vec![]
            }]
        );
    }

    #[test]
    fn the_first_working_message_is_optional_and_check_in_intervals_stay_in_range() {
        let settings = load(json!({
            "contexts": [
                { "name": "A", "checkInMinutes": 20.4 },
                { "name": "B", "checkInMinutes": 999 }
            ]
        }));

        assert_eq!(settings.status_messages.first_working, "");
        let intervals: Vec<u32> = settings
            .contexts
            .iter()
            .map(|context| context.check_in_minutes)
            .collect();
        assert_eq!(intervals, [20, 15]);
    }

    #[test]
    fn music_entries_and_a_contexts_start_entry_are_kept_only_while_they_exist() {
        let settings = load(json!({
            "music": [
                { "name": "Deep Focus", "source": "Deep Focus" },
                { "name": "", "source": "" },
                { "name": "Radio", "kind": "link", "source": "https://music.apple.com/at/station/x/ra.1" },
                { "name": "Cell", "kind": "artist", "source": "Cell", "artistId": "27067520", "storefront": "AT" }
            ],
            "contexts": [
                { "name": "A", "music": { "onCall": "pause", "startWith": "deep-focus", "callVolume": 500 } },
                { "name": "B", "music": { "startWith": "gone" } }
            ]
        }));

        let entries: Vec<(&str, MusicKind)> = settings
            .music
            .iter()
            .map(|entry| (entry.id.as_str(), entry.kind))
            .collect();
        assert_eq!(
            entries,
            [
                ("deep-focus", MusicKind::Playlist),
                ("radio", MusicKind::Link),
                ("cell", MusicKind::Artist)
            ]
        );
        assert_eq!(
            (
                settings.music[2].artist_id.as_str(),
                settings.music[2].storefront.as_str(),
                settings.music[0].artist_id.as_str()
            ),
            ("27067520", "at", "")
        );
        assert_eq!(
            settings.music[1].source,
            "https://music.apple.com/at/station/x/ra.1?app=music"
        );
        let music: Vec<(OnCall, &str, u8)> = settings
            .contexts
            .iter()
            .map(|context| {
                (
                    context.music.on_call,
                    context.music.start_with.as_str(),
                    context.music.call_volume,
                )
            })
            .collect();
        assert_eq!(
            music,
            [(OnCall::Pause, "deep-focus", 2), (OnCall::Lower, "", 2)]
        );
    }

    #[test]
    fn an_emptied_list_stays_empty() {
        let settings = load(json!({ "routines": [], "links": [] }));

        assert!(settings.routines.is_empty());
        assert!(settings.links.is_empty());
    }

    #[test]
    fn contexts_get_a_palette_colour_valid_work_hours_and_the_timelines_hours_stay_in_order() {
        let settings = load(json!({
            "contexts": [
                {
                    "name": "A",
                    "color": "pink",
                    "workHours": { "days": [1, 9], "from": "9:00", "to": "23:30" },
                    "autoSwitch": true
                },
                { "name": "B", "color": "nope", "autoSwitch": true }
            ],
            "timeline": { "firstHour": 7, "lastHour": 3 }
        }));

        let colors: Vec<ContextColor> = settings.contexts.iter().map(|c| c.color).collect();
        assert_eq!(colors, [ContextColor::Pink, ContextColor::Purple]);
        let hours: Vec<Option<WorkHours>> = settings
            .contexts
            .iter()
            .map(|c| c.work_hours.clone())
            .collect();
        assert_eq!(
            hours,
            [
                Some(WorkHours {
                    days: vec![1],
                    from: "09:00".into(),
                    to: "23:30".into()
                }),
                None
            ]
        );
        let auto: Vec<bool> = settings.contexts.iter().map(|c| c.auto_switch).collect();
        assert_eq!(auto, [true, false]);
        assert_eq!(
            settings.timeline,
            Timeline {
                first_hour: 7,
                last_hour: 24
            }
        );
    }

    #[test]
    fn projects_stay_active_and_get_clean_keywords_contexts_and_launcher_projects() {
        let settings = load(json!({
            "projects": [{
                "name": "Oscorp",
                "keywords": [" QRS ", "qrs", "", "Initech"],
                "launcherProjects": [
                    { "id": "p1", "name": "QRS" },
                    { "id": "p1" },
                    { "name": "no id" },
                    { "id": "p2" }
                ]
            }]
        }));
        let project = &settings.projects[0];

        assert!(project.active);
        assert_eq!(project.contexts, Vec::<String>::new());
        assert_eq!(project.keywords, ["QRS", "Initech"]);
        assert_eq!(
            project.launcher_projects,
            [
                LinkedProject {
                    id: "p1".into(),
                    name: "QRS".into()
                },
                LinkedProject {
                    id: "p2".into(),
                    name: "p2".into()
                }
            ]
        );
        assert!(
            !load(json!({ "projects": [{ "name": "Old", "active": false }] })).projects[0].active
        );
        let linked = load(json!({
            "projects": [{
                "name": "Oscorp",
                "timetrackerProjects": [
                    { "id": "t1", "name": "Oscorp Core" },
                    { "id": "t1", "name": "again" },
                    { "name": "no id" },
                    { "id": "t2" }
                ]
            }]
        }));
        assert_eq!(
            linked.projects[0].timetracker_projects,
            [
                LinkedProject {
                    id: "t1".into(),
                    name: "Oscorp Core".into()
                },
                LinkedProject {
                    id: "t2".into(),
                    name: "t2".into()
                }
            ]
        );
        assert_eq!(linked.projects[0].launcher_projects, []);
        assert_eq!(
            load(json!({ "projects": [{ "name": "W", "contexts": ["work", "work", " "] }] }))
                .projects[0]
                .contexts,
            ["work"]
        );
    }
}

mod rail_port {
    use super::*;

    #[test]
    fn missing_or_broken_values_fall_back_to_the_defaults() {
        assert_eq!(load(json!({})).rail, Rail::default());
        assert_eq!(
            rail(json!({ "shape": "hexagon", "triggerWidth": "wide" })),
            Rail::default()
        );
    }

    #[test]
    fn an_older_file_keeps_its_edge_and_look() {
        let slim = rail(json!({ "enabled": false, "side": "left", "shape": "slim" }));

        assert!(!slim.enabled);
        assert_eq!(slim.edges, RailEdges::Left);
        assert_eq!(slim.shape, RailShape::Pill);
        assert_eq!(slim.size, RailSize::Compact);
        assert!(!slim.captions);
        assert_eq!(
            rail(json!({ "shape": "borderless" })).shape,
            RailShape::Floating
        );
    }

    #[test]
    fn a_call_has_its_own_edge_and_place_right_and_centered_unless_set() {
        assert_eq!(Rail::default().call_edge, RailCallEdge::Right);
        assert_eq!(Rail::default().call_position, RailCallPosition::Center);

        let set = rail(json!({
            "edges": "both",
            "position": "pointer",
            "callEdge": "left",
            "callPosition": "top"
        }));
        assert_eq!(set.edges, RailEdges::Both);
        assert_eq!(set.position, RailPosition::Pointer);
        assert_eq!(set.call_edge, RailCallEdge::Left);
        assert_eq!(set.call_position, RailCallPosition::Top);

        let odd = rail(json!({ "callEdge": "both", "callPosition": "pointer" }));
        assert_eq!(odd.call_edge, RailCallEdge::Right);
        assert_eq!(odd.call_position, RailCallPosition::Center);
    }

    #[test]
    fn numbers_stay_in_range() {
        let numbers = rail(json!({
            "triggerWidth": 500,
            "showDelay": -10,
            "hideDelay": 20,
            "tooltipDelay": 12.6
        }));

        assert_eq!(
            (
                numbers.trigger_width,
                numbers.show_delay,
                numbers.hide_delay,
                numbers.tooltip_delay
            ),
            (40, 0, 100, 13)
        );
    }
}

mod whole_settings {
    use super::*;

    #[test]
    fn input_that_is_not_an_object_gives_the_defaults() {
        for raw in [
            json!(null),
            json!(7),
            json!("x"),
            json!(true),
            json!([1, 2]),
        ] {
            assert_eq!(load(raw.clone()), Settings::default(), "{raw}");
        }
    }

    #[test]
    fn serialized_settings_load_back_unchanged() {
        let rich = load(json!({
            "accent": "teal",
            "lightScheme": "light",
            "textSize": "large",
            "vault": { "path": "/tmp/vault", "name": "Notes" },
            "slack": { "channel": "#status", "statuses": { "break": { "keepStatus": true } } },
            "projects": [{ "name": "Nexo", "slackEmoji": "nexo", "keywords": ["a", "A", "b"] }],
            "contexts": [
                { "name": "Work", "workHours": { "from": "9:00", "to": "17:00" }, "autoSwitch": true,
                  "features": ["status", "bogus", "jira"], "music": { "startWith": "focus" } },
                { "name": "Work" }
            ],
            "links": [{ "title": "T", "target": "https://x", "days": [3, 1], "time": { "from": 60, "to": 120 } }],
            "music": [{ "name": "Focus", "source": "Focus" }, { "name": "Radio", "kind": "link",
                      "source": "https://music.apple.com/at/station/x/ra.1" }],
            "routines": [{ "name": "Lunch", "timing": { "due": { "from": "12:00" }, "openWhenDue": true } }],
            "tools": [{ "id": "music", "rail": false }, { "id": "hub", "window": true }],
            "hotkeys": { "search": "" },
            "api": { "enabled": true, "port": 4000, "token": "t" }
        }));

        let back = load(serde_json::to_value(&rich).unwrap());

        assert_eq!(back, rich);
        assert_eq!(rich.clone().sanitized(), rich);
    }

    #[test]
    fn the_appearance_keeps_working_beside_the_groups() {
        let settings = load(json!({
            "lightScheme": "light",
            "darkScheme": "nope",
            "accent": "chartreuse",
            "textSize": "large",
            "launchAtLogin": true
        }));

        assert_eq!(
            (
                settings.light_scheme,
                settings.dark_scheme,
                settings.accent.as_str(),
                settings.text_size,
                settings.launch_at_login
            ),
            (Scheme::Light, Scheme::Dark, "system", TextSize::Large, true)
        );
    }

    #[test]
    fn the_api_is_off_by_default() {
        assert!(!Settings::default().api.enabled);
    }
}

mod daily_note {
    use super::*;

    fn order(raw: Value) -> Vec<DailySection> {
        load(json!({ "dailyNote": { "order": raw } }))
            .daily_note
            .order
    }

    #[test]
    fn the_section_order_fills_in_missing_sections_after_the_one_they_follow_by_default() {
        use DailySection::*;
        for (stored, expected) in [
            (json!([]), vec![Journal, Focus, Plan, Notes]),
            (json!("nope"), vec![Journal, Focus, Plan, Notes]),
            (
                json!(["notes", "journal"]),
                vec![Notes, Journal, Focus, Plan],
            ),
            (json!(["notes"]), vec![Journal, Focus, Plan, Notes]),
            (
                json!(["plan", "notes", "focus", "journal"]),
                vec![Plan, Notes, Focus, Journal],
            ),
            (
                json!(["journal", "plan", "notes"]),
                vec![Journal, Focus, Plan, Notes],
            ),
        ] {
            assert_eq!(order(stored.clone()), expected, "{stored}");
        }
    }

    #[test]
    fn duplicate_and_unknown_sections_including_a_stored_activity_are_dropped() {
        use DailySection::*;
        for (stored, expected) in [
            (
                json!(["journal", "journal", "activity", "notes", "focus"]),
                vec![Journal, Notes, Focus, Plan],
            ),
            (json!(["activity"]), vec![Journal, Focus, Plan, Notes]),
            (
                json!(["plan", 3, null, "plan"]),
                vec![Journal, Focus, Plan, Notes],
            ),
        ] {
            assert_eq!(order(stored.clone()), expected, "{stored}");
        }
    }

    #[test]
    fn blank_texts_and_unknown_levels_fall_back_to_the_defaults() {
        let note = load(json!({
            "dailyNote": {
                "fallbackFolder": "  ",
                "fallbackFormat": "DD.MM.YYYY",
                "notes": null,
                "journal": { "level": "#####", "text": "" },
                "plan": { "level": "###", "text": " Plan " }
            }
        }))
        .daily_note;

        let defaults = DailyNote::default();
        assert_eq!(note.fallback_folder, defaults.fallback_folder);
        assert_eq!(note.fallback_format, "DD.MM.YYYY");
        assert_eq!(note.notes, defaults.notes);
        assert_eq!(note.journal, defaults.journal);
        assert_eq!(note.focus, defaults.focus);
        assert_eq!(
            note.plan,
            SectionPlacement {
                level: HeadingLevel::H3,
                text: "Plan".into()
            }
        );
    }
}

mod tools {
    use super::*;

    #[test]
    fn a_missing_tool_is_inserted_after_its_default_predecessor() {
        let all = default_tools();
        for removed in 0..all.len() {
            let stored: Vec<Value> = all
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != removed)
                .map(|(_, tool)| json!({ "id": tool.id }))
                .collect();

            let settings = load(json!({ "tools": stored }));

            assert_eq!(
                tool_ids(&settings),
                default_tool_ids(),
                "without {:?}",
                all[removed].id
            );
        }
    }

    #[test]
    fn every_tool_appears_once_and_unknown_ids_are_dropped() {
        use ToolId::*;
        let settings = load(json!({
            "tools": [{ "id": "music" }, { "id": "nope" }, { "id": "context" }, { "id": "music" }, 7]
        }));

        assert_eq!(
            tool_ids(&settings),
            [
                Music, Context, Status, Activity, Call, Separator1, Journal, Hub, Search, Jira,
                QuickNotes, Separator2, Links, Projects
            ]
        );
    }

    #[test]
    fn the_hub_is_never_in_the_window_and_placements_are_kept() {
        let settings = load(json!({
            "tools": [
                { "id": "hub", "window": true },
                { "id": "search", "rail": false, "window": false }
            ]
        }));
        let find = |id| settings.tools.iter().find(|tool| tool.id == id).unwrap();

        assert_eq!(
            (find(ToolId::Hub).rail, find(ToolId::Hub).window),
            (true, false)
        );
        assert_eq!(
            (find(ToolId::Search).rail, find(ToolId::Search).window),
            (false, false)
        );
        assert_eq!(
            (find(ToolId::Jira).rail, find(ToolId::Jira).window),
            (true, true)
        );
    }

    #[test]
    fn broken_tools_give_the_default_tools() {
        assert_eq!(load(json!({ "tools": "x" })).tools, default_tools());
    }
}

mod hotkeys {
    use super::*;

    #[test]
    fn a_stored_empty_shortcut_stays_cleared_and_missing_ones_get_their_default() {
        let hotkeys = load(json!({
            "hotkeys": { "search": "", "journal": "cmd+j", "links": 7, "hub": "cmd+h" }
        }))
        .hotkeys;
        let defaults = Hotkeys::default();

        assert_eq!(hotkeys.search, "");
        assert_eq!(hotkeys.journal, "cmd+j");
        assert_eq!(hotkeys.hub, "cmd+h");
        assert_eq!(hotkeys.links, defaults.links);
        assert_eq!(hotkeys.toggle_rail, defaults.toggle_rail);
        assert_eq!(hotkeys.quick_notes, defaults.quick_notes);
    }

    #[test]
    fn none_leaves_every_shortcut_empty() {
        let value = serde_json::to_value(Hotkeys::none()).unwrap();
        let fields = value.as_object().unwrap();
        assert_eq!(fields.len(), 14);
        for (name, shortcut) in fields {
            assert_eq!(shortcut, "", "{name}");
        }
    }
}

mod reminders {
    use super::*;

    fn routine(id: &str, enabled: bool) -> ReminderRule {
        ReminderRule::Routine {
            enabled,
            routine: id.into(),
        }
    }

    fn journal(group: &str) -> ReminderRule {
        ReminderRule::Journal {
            enabled: true,
            group: group.into(),
        }
    }

    #[test]
    fn the_defaults_have_one_rule_per_routine_and_journal_group() {
        assert_eq!(
            load(json!({})).reminders,
            vec![
                ReminderRule::Meeting {
                    enabled: true,
                    minutes_before: 60
                },
                routine("morning", true),
                routine("evening", true),
                journal("Dawn"),
                journal("Dusk"),
                journal("Health"),
                ReminderRule::Todos {
                    enabled: false,
                    span: TodoSpan::Today,
                    from: "16:00".into(),
                    to: String::new()
                }
            ]
        );
    }

    #[test]
    fn new_items_get_a_rule_at_the_end_and_rules_of_removed_items_are_dropped() {
        let settings = load(json!({ "routines": [{ "name": "Lunch" }], "journal": [] }));

        assert_eq!(
            settings.reminders,
            vec![
                ReminderRule::Meeting {
                    enabled: true,
                    minutes_before: 60
                },
                ReminderRule::Todos {
                    enabled: false,
                    span: TodoSpan::Today,
                    from: "16:00".into(),
                    to: String::new()
                },
                routine("lunch", true)
            ]
        );
    }

    #[test]
    fn every_item_has_exactly_one_rule_and_the_stored_order_wins() {
        let settings = load(json!({
            "reminders": [
                { "kind": "routine", "routine": "evening", "enabled": false },
                { "kind": "routine", "routine": "evening" },
                { "kind": "routine", "routine": "gone" },
                { "kind": "journal", "group": "Dawn" }
            ]
        }));

        assert_eq!(
            settings.reminders,
            vec![
                routine("evening", false),
                journal("Dawn"),
                routine("morning", true),
                journal("Dusk"),
                journal("Health")
            ]
        );
    }

    #[test]
    fn rules_are_repaired_and_invalid_ones_dropped() {
        let settings = load(json!({
            "routines": [],
            "journal": [],
            "reminders": [
                { "kind": "meeting", "minutesBefore": 30, "enabled": false },
                { "kind": "meeting", "minutesBefore": 999 },
                { "kind": "bogus" },
                { "kind": "routine" },
                { "kind": "todos", "span": "week", "from": "8:00" },
                { "kind": "todos", "span": "week", "from": "late" }
            ]
        }));

        assert_eq!(
            settings.reminders,
            vec![
                ReminderRule::Meeting {
                    enabled: false,
                    minutes_before: 30
                },
                ReminderRule::Meeting {
                    enabled: true,
                    minutes_before: 60
                },
                ReminderRule::Todos {
                    enabled: true,
                    span: TodoSpan::Week,
                    from: "08:00".into(),
                    to: String::new()
                }
            ]
        );
    }

    #[test]
    fn rules_serialize_with_their_kind_and_camel_case_fields() {
        let value = serde_json::to_value(vec![
            ReminderRule::Meeting {
                enabled: true,
                minutes_before: 60,
            },
            routine("morning", false),
            journal("Dawn"),
            ReminderRule::Todos {
                enabled: true,
                span: TodoSpan::All,
                from: "16:00".into(),
                to: String::new(),
            },
        ])
        .unwrap();

        assert_eq!(
            value,
            json!([
                { "kind": "meeting", "enabled": true, "minutesBefore": 60 },
                { "kind": "routine", "enabled": false, "routine": "morning" },
                { "kind": "journal", "enabled": true, "group": "Dawn" },
                { "kind": "todos", "enabled": true, "span": "all", "from": "16:00", "to": "" }
            ])
        );
    }
}

mod ids {
    use super::*;

    #[test]
    fn duplicate_ids_get_a_two_suffix() {
        let settings = load(json!({
            "contexts": [
                { "name": "Work" },
                { "name": "Work" },
                { "name": "Other", "id": "x" },
                { "name": "Third", "id": "x" },
                { "name": "!!!" }
            ],
            "routines": [{ "name": "Lunch" }, { "name": "Lunch" }],
            "music": [{ "name": "Focus" }, { "name": "Focus" }]
        }));

        let contexts: Vec<&str> = settings.contexts.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(contexts, ["work", "work-2", "x", "x-2", "context"]);
        let routines: Vec<&str> = settings.routines.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(routines, ["lunch", "lunch-2"]);
        let music: Vec<&str> = settings.music.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(music, ["focus", "focus-2"]);
    }
}

mod status {
    use super::*;

    #[test]
    fn status_rules_are_sorted_by_hours_and_invalid_ones_dropped() {
        let rules = load(json!({
            "statusRules": {
                "rules": [
                    { "hours": 48 },
                    { "hours": 0 },
                    { "hours": "x" },
                    { "hours": 4.5 },
                    { "hours": 100000 },
                    "bad",
                    { "hours": 8, "enabled": false }
                ]
            }
        }))
        .status_rules
        .rules;

        let summary: Vec<(u32, bool)> = rules.iter().map(|r| (r.hours, r.enabled)).collect();
        assert_eq!(summary, [(5, true), (8, false), (48, true)]);
        assert_eq!(rules[0].slack, StatusRules::default().rules[0].slack);
    }

    #[test]
    fn an_emptied_rule_list_stays_empty_and_a_broken_one_gives_the_defaults() {
        assert!(
            load(json!({ "statusRules": { "rules": [] } }))
                .status_rules
                .rules
                .is_empty()
        );
        assert_eq!(
            load(json!({ "statusRules": { "rules": "x" } })).status_rules,
            StatusRules::default()
        );
    }

    #[test]
    fn status_messages_keep_empty_optional_texts_but_not_an_empty_off_message() {
        let messages = load(json!({
            "statusMessages": { "working": "", "firstWorking": "gm", "break": "brb", "off": "" }
        }))
        .status_messages;

        assert_eq!(
            messages,
            StatusMessages {
                working: String::new(),
                first_working: "gm".into(),
                break_: "brb".into(),
                off: StatusMessages::default().off
            }
        );
    }

    #[test]
    fn the_end_of_day_check_needs_a_question_and_both_messages() {
        let end = load(json!({
            "endOfDay": { "enabled": false, "question": " ", "url": "", "done": "ok", "incomplete": "" }
        }))
        .end_of_day;
        let defaults = EndOfDay::default();

        assert_eq!(
            end,
            EndOfDay {
                enabled: false,
                reminder: String::new(),
                question: defaults.question,
                url: String::new(),
                done: "ok".into(),
                incomplete: defaults.incomplete
            }
        );
    }

    #[test]
    fn slack_statuses_fall_back_per_field() {
        let slack = load(json!({
            "slack": {
                "channel": "#team",
                "statuses": { "working": { "text": "Busy", "presence": "nope" }, "off": 3 },
                "manualWorkflows": { "break": false }
            }
        }))
        .slack;
        let defaults = Slack::default();

        assert_eq!(slack.channel, "#team");
        assert_eq!(
            slack.statuses.working,
            SlackStatus {
                keep_status: false,
                emoji: defaults.statuses.working.emoji.clone(),
                text: "Busy".into(),
                presence: Presence::Auto
            }
        );
        assert_eq!(slack.statuses.off, defaults.statuses.off);
        assert_eq!(
            slack.manual_workflows,
            StatusMap {
                working: true,
                break_: false,
                off: true
            }
        );
    }
}

mod values {
    use super::*;

    #[test]
    fn the_api_port_must_be_a_whole_number_from_1024_to_65535() {
        for (port, expected) in [
            (json!(8080), 8080),
            (json!(1024), 1024),
            (json!(65535), 65535),
            (json!(1023), 27180),
            (json!(65536), 27180),
            (json!(80), 27180),
            (json!(8080.5), 27180),
            (json!("8080"), 27180),
            (json!(null), 27180),
        ] {
            assert_eq!(
                load(json!({ "api": { "port": port } })).api.port,
                expected,
                "{port}"
            );
        }
    }

    #[test]
    fn the_timelines_last_hour_stays_after_its_first_hour() {
        for ((first, last), expected) in [
            ((json!(7), json!(3)), (7, 24)),
            ((json!(9), json!(18)), (9, 18)),
            ((json!(23), json!(24)), (23, 24)),
            ((json!(10), json!(10)), (10, 24)),
            ((json!(-1), json!(10)), (0, 10)),
            ((json!(24), json!(30)), (0, 24)),
            ((json!(5.4), json!(12.6)), (5, 13)),
            ((json!("x"), json!(null)), (0, 24)),
        ] {
            let timeline =
                load(json!({ "timeline": { "firstHour": first, "lastHour": last } })).timeline;
            assert_eq!(
                (timeline.first_hour, timeline.last_hour),
                expected,
                "{first}/{last}"
            );
        }
    }

    #[test]
    fn slack_emojis_get_colons_around_the_name() {
        for (stored, expected) in [
            ("nexo", ":nexo:"),
            (":nexo:", ":nexo:"),
            (" :ne xo: ", ":nexo:"),
            ("", ""),
            (":", ""),
        ] {
            let settings = load(json!({ "projects": [{ "name": "P", "slackEmoji": stored }] }));
            assert_eq!(settings.projects[0].slack_emoji, expected, "{stored:?}");
        }
    }

    #[test]
    fn music_links_open_in_the_music_app_and_other_links_stay_as_they_are() {
        for (source, expected) in [
            (
                "https://music.apple.com/at/station/x/ra.1",
                "https://music.apple.com/at/station/x/ra.1?app=music",
            ),
            (
                "https://music.apple.com/at/album/x?i=1&app=itunes",
                "https://music.apple.com/at/album/x?i=1&app=music",
            ),
            (
                "https://music.apple.com/at/album/x?app=music",
                "https://music.apple.com/at/album/x?app=music",
            ),
            ("https://example.com/x", "https://example.com/x"),
            (
                "https://evilmusic.apple.com/x",
                "https://evilmusic.apple.com/x",
            ),
            (
                "https://evilmusic.apple.com.example.com/x",
                "https://evilmusic.apple.com.example.com/x",
            ),
        ] {
            let settings =
                load(json!({ "music": [{ "name": "L", "kind": "link", "source": source }] }));
            assert_eq!(settings.music[0].source, expected, "{source}");
        }
    }

    #[test]
    fn artist_ids_and_storefronts_are_only_kept_when_valid_for_an_artist() {
        for (entry, expected) in [
            (
                json!({ "kind": "artist", "artistId": "123", "storefront": "AT" }),
                ("123", "at"),
            ),
            (
                json!({ "kind": "artist", "artistId": "12a", "storefront": "AUT" }),
                ("", ""),
            ),
            (
                json!({ "kind": "artist", "artistId": "", "storefront": "a1" }),
                ("", ""),
            ),
            (
                json!({ "kind": "playlist", "artistId": "123", "storefront": "at" }),
                ("", ""),
            ),
        ] {
            let mut raw = entry.clone();
            raw["name"] = json!("E");
            let settings = load(json!({ "music": [raw] }));
            assert_eq!(
                (
                    settings.music[0].artist_id.as_str(),
                    settings.music[0].storefront.as_str()
                ),
                expected,
                "{entry}"
            );
        }
    }

    #[test]
    fn context_volume_and_start_entry_defaults() {
        let settings = load(json!({
            "music": [{ "name": "Focus", "source": "Focus" }],
            "contexts": [{ "name": "A", "music": { "callVolume": "30", "startWith": " focus ", "resume": false } }]
        }));
        let music = &settings.contexts[0].music;

        assert_eq!(
            (
                music.call_volume,
                music.start_with.as_str(),
                music.resume,
                music.pause_when_away
            ),
            (30, "focus", false, true)
        );
    }

    #[test]
    fn contexts_keep_only_known_features_and_valid_work_hours() {
        let settings = load(json!({
            "contexts": [
                { "name": "A", "features": ["slack", "bogus", "status"], "searchScope": " ",
                  "workHours": { "from": "25:00", "to": "18:00" }, "autoSwitch": true, "checkInMinutes": 0 }
            ]
        }));
        let context = &settings.contexts[0];

        assert_eq!(context.features, [Feature::Slack, Feature::Status]);
        assert_eq!(context.search_scope, "all");
        assert_eq!(context.work_hours, None);
        assert!(!context.auto_switch);
        assert_eq!(context.check_in_minutes, 0);
    }

    #[test]
    fn project_keywords_are_unique_whatever_their_case() {
        let settings = load(json!({
            "projects": [{ "name": "P", "keywords": ["Foo", "foo", "FOO", "Bar", " bar "] }]
        }));

        assert_eq!(settings.projects[0].keywords, ["Foo", "Bar"]);
    }

    #[test]
    fn the_vault_needs_a_name_and_an_empty_path_means_none() {
        let vault = load(
            json!({ "vault": { "path": "", "name": "  ", "obsidianCliPath": "/bin/obsidian" } }),
        )
        .vault;

        assert_eq!(
            vault,
            Vault {
                path: None,
                name: "Vault".into(),
                obsidian_cli_path: "/bin/obsidian".into()
            }
        );
    }
}

mod links {
    use super::*;

    #[test]
    fn links_need_a_target_and_unset_rules_are_none() {
        let links = load(json!({
            "links": [
                { "title": "No target" },
                { "title": " Plain ", "target": " https://x ", "contexts": [], "days": [], "event": " " },
                {
                    "target": "t",
                    "contexts": ["work"],
                    "projects": ["Oscorp"],
                    "days": [3, 1, 1, 9],
                    "time": { "from": "540", "to": 600 },
                    "event": "Standup",
                    "eventWindow": "before",
                    "leadMinutes": 5,
                    "pinned": true
                },
                { "target": "u", "time": { "from": 600, "to": 540 }, "leadMinutes": -1, "eventWindow": "nope" }
            ]
        }))
        .links;

        assert_eq!(links.len(), 3);
        assert_eq!(
            links[0],
            QuickLink {
                title: "Plain".into(),
                target: "https://x".into(),
                contexts: None,
                projects: None,
                days: None,
                time: None,
                event: None,
                event_window: EventWindow::UntilEnd,
                lead_minutes: 15.0,
                pinned: false
            }
        );
        assert_eq!(
            links[1],
            QuickLink {
                title: String::new(),
                target: "t".into(),
                contexts: Some(vec!["work".into()]),
                projects: Some(vec!["Oscorp".into()]),
                days: Some(vec![1, 3]),
                time: Some(MinuteRange {
                    from: 540.0,
                    to: 600.0
                }),
                event: Some("Standup".into()),
                event_window: EventWindow::Before,
                lead_minutes: 5.0,
                pinned: true
            }
        );
        assert_eq!(
            (
                links[2].time.clone(),
                links[2].lead_minutes,
                links[2].event_window
            ),
            (None, 15.0, EventWindow::UntilEnd)
        );
    }
}

mod calls {
    use super::*;

    fn open_with(raw: Value) -> OpenWith {
        load(json!({ "joining": { "openWith": raw } }))
            .joining
            .open_with
    }

    fn app(name: &str, bundle_id: &str) -> Option<AppRef> {
        Some(AppRef {
            app: name.into(),
            bundle_id: bundle_id.into(),
        })
    }

    #[test]
    fn a_service_left_out_keeps_its_default() {
        let defaults = Joining::default().open_with;

        assert_eq!(open_with(json!({})), defaults);
        assert_eq!(load(json!({})).joining, Joining::default());
        assert_eq!(
            open_with(json!({ "Zoom": { "app": "Zoom", "bundleId": "us.zoom.xos" } })).meet,
            defaults.meet
        );
    }

    #[test]
    fn a_service_stored_as_null_or_without_a_bundle_opens_with_macos() {
        assert_eq!(open_with(json!({ "Meet": null })).meet, None);
        assert_eq!(open_with(json!({ "Meet": { "bundleId": " " } })).meet, None);
        assert_eq!(open_with(json!({ "Meet": "x" })).meet, None);
    }

    #[test]
    fn a_picked_app_shows_its_bundle_id_when_it_has_no_name() {
        let with = open_with(json!({
            "Zoom": { "bundleId": " x.y " },
            "Teams": { "app": "Teams", "bundleId": "com.microsoft.teams2" }
        }));

        assert_eq!(with.zoom, app("x.y", "x.y"));
        assert_eq!(with.teams, app("Teams", "com.microsoft.teams2"));
    }

    #[test]
    fn apps_started_first_need_a_bundle_id_and_an_empty_list_stays_empty() {
        let joining = load(json!({
            "joining": { "startFirst": [{ "app": "A" }, { "bundleId": "b.c" }] }
        }))
        .joining;
        assert_eq!(
            joining.start_first,
            [AppRef {
                app: "b.c".into(),
                bundle_id: "b.c".into()
            }]
        );

        assert!(
            load(json!({ "joining": { "startFirst": [] } }))
                .joining
                .start_first
                .is_empty()
        );
        assert_eq!(
            load(json!({ "joining": { "startFirst": "x" } }))
                .joining
                .start_first,
            Joining::default().start_first
        );
    }

    #[test]
    fn meeting_rules_need_an_app_and_a_bundle_id_and_get_a_status_text() {
        let meetings = load(json!({
            "meetings": {
                "detection": false,
                "rules": [
                    { "app": "Webex", "bundleId": "com.webex", "windowTitle": " " },
                    { "app": "NoBundle" },
                    { "bundleId": "no.app" },
                    { "app": "Slack", "bundleId": "com.slack", "windowTitle": "Huddle", "enabled": false }
                ]
            }
        }))
        .meetings;

        assert!(!meetings.detection);
        assert_eq!(meetings.rules.len(), 2);
        assert_eq!(meetings.rules[0].window_title, None);
        assert_eq!(meetings.rules[0].slack.text, "In a Webex meeting");
        assert!(meetings.rules[0].enabled);
        assert_eq!(meetings.rules[1].window_title.as_deref(), Some("Huddle"));
        assert!(!meetings.rules[1].enabled);
    }

    #[test]
    fn removed_meeting_rules_stay_removed() {
        assert!(
            load(json!({ "meetings": { "rules": [] } }))
                .meetings
                .rules
                .is_empty()
        );
    }
}

mod journal {
    use super::*;

    #[test]
    fn callouts_are_normalized_to_upper_case_words() {
        for (stored, expected) in [
            (json!("[!heart]"), "HEART"),
            (json!("warning"), "WARNING"),
            (json!(" [!Note] "), "NOTE"),
            (json!("bad name!"), "NOTE"),
            (json!(""), "NOTE"),
            (json!(null), "NOTE"),
        ] {
            let settings = load(json!({ "journal": [{ "title": "G", "callout": stored }] }));
            assert_eq!(settings.journal[0].callout, expected, "{stored}");
        }
    }

    #[test]
    fn groups_need_a_title_and_questions_need_a_question() {
        let settings = load(json!({
            "journal": [
                { "title": "" },
                {
                    "title": "Morning",
                    "questions": [
                        { "question": "" },
                        { "question": " Why? ", "hint": " h ", "kind": "nope" },
                        { "question": "Todo", "kind": "task" }
                    ]
                }
            ]
        }));

        assert_eq!(settings.journal.len(), 1);
        assert_eq!(
            settings.journal[0].questions,
            [
                JournalQuestion::new("Why?", "h", AnswerKind::Text),
                JournalQuestion::new("Todo", "", AnswerKind::Task)
            ]
        );
    }

    #[test]
    fn a_due_window_is_cleaned_and_opening_when_due_needs_one() {
        let settings = load(json!({
            "journal": [
                { "title": "A", "timing": { "due": { "from": "9:00" }, "openWhenDue": true, "openOn": ["start", "bogus", "start", "meeting-end"] } },
                { "title": "B", "timing": { "due": { "from": "late" }, "openWhenDue": true } },
                { "title": "C", "timing": { "due": { "from": "22:00", "to": "06:30" } } }
            ]
        }));
        let timings: Vec<&Timing> = settings.journal.iter().map(|g| &g.timing).collect();

        assert_eq!(
            timings[0],
            &Timing {
                due: Some(TimeWindow::new("09:00", "")),
                open_when_due: true,
                open_on: vec![OpenEvent::Start, OpenEvent::MeetingEnd]
            }
        );
        assert_eq!(timings[1], &Timing::default());
        assert_eq!(
            timings[2],
            &Timing {
                due: Some(TimeWindow::new("22:00", "06:30")),
                open_when_due: false,
                open_on: vec![]
            }
        );
    }

    #[test]
    fn routine_steps_keep_sorted_distinct_weekdays() {
        let settings = load(json!({
            "routines": [{ "name": "R", "steps": [{ "title": "S", "days": [5, 1, 1, 7, 2.5] }] }]
        }));

        assert_eq!(settings.routines[0].steps[0].days, [1, 5]);
    }
}

mod routine_days {
    use super::*;

    fn routine() -> Routine {
        let routines = Routine::read_all(&[json!({
            "name": "Morning",
            "steps": [
                { "title": "Plan day" },
                { "title": "Weekly report", "days": [5] },
                { "title": "Book rooms", "days": [1, 3] }
            ]
        })]);
        routines.into_iter().next().unwrap()
    }

    fn titles(routine: &Routine, weekday: u8) -> Vec<(usize, &str)> {
        routine
            .steps_on(weekday)
            .into_iter()
            .map(|(index, step)| (index, step.title.as_str()))
            .collect()
    }

    #[test]
    fn daily_items_and_the_ones_due_that_weekday_are_kept_with_their_position() {
        assert_eq!(titles(&routine(), 3), [(0, "Plan day"), (2, "Book rooms")]);
    }

    #[test]
    fn the_friday_item_is_due_on_fridays() {
        assert_eq!(
            titles(&routine(), 5),
            [(0, "Plan day"), (1, "Weekly report")]
        );
    }

    #[test]
    fn only_daily_items_are_due_on_a_day_no_item_names() {
        assert_eq!(titles(&routine(), 0), [(0, "Plan day")]);
    }
}
