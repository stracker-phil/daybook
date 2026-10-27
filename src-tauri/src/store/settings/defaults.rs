//! The owner's starting configuration: what a fresh install has before anything is edited.

use super::contexts::{Context, ContextColor, Feature, Project, WorkHours};
use super::music::ContextMusic;
use super::workflows::{
    AnswerKind, JournalGroup, JournalQuestion, OpenEvent, ReminderRule, Routine, RoutineStep,
    Timing, TodoSpan,
};

pub fn projects() -> Vec<Project> {
    vec![Project::new("Internal"), Project::new("Oscorp")]
}

pub fn contexts() -> Vec<Context> {
    vec![
        Context {
            id: "work".into(),
            label: "[W]".into(),
            name: "Work".into(),
            features: vec![
                Feature::Status,
                Feature::Slack,
                Feature::Jira,
                Feature::Routines,
            ],
            search_scope: "work".into(),
            color: ContextColor::Blue,
            work_hours: Some(WorkHours {
                days: vec![1, 2, 3, 4, 5],
                from: "09:00".into(),
                to: "18:00".into(),
            }),
            auto_switch: true,
            calendars: vec![],
            check_in_minutes: 15,
            music: ContextMusic::default(),
        },
        Context {
            id: "private".into(),
            label: "[P]".into(),
            name: "Private".into(),
            features: vec![],
            search_scope: "private".into(),
            color: ContextColor::Green,
            work_hours: None,
            auto_switch: false,
            calendars: vec![],
            check_in_minutes: 15,
            music: ContextMusic::default(),
        },
    ]
}

pub fn journal() -> Vec<JournalGroup> {
    let group = |title: &str, callout: &str, folded, timing, questions| JournalGroup {
        title: title.into(),
        callout: callout.into(),
        folded,
        questions,
        timing,
    };
    let q = JournalQuestion::new;
    vec![
        group(
            "Dawn",
            "NOTE",
            false,
            Timing::new("10:00", "13:00", vec![]),
            vec![
                q(
                    "What am I grateful for?",
                    "Small things count too",
                    AnswerKind::List,
                ),
                q(
                    "My current goal",
                    "Where do I want to be in a year?",
                    AnswerKind::Text,
                ),
                q(
                    "The \"One Thing\"",
                    "What would I do today even without pay?",
                    AnswerKind::Task,
                ),
            ],
        ),
        group(
            "Dusk",
            "NOTE",
            false,
            Timing::new("17:30", "", vec![]),
            vec![
                q(
                    "Three things I'm proud of",
                    "What went well? What did I add?",
                    AnswerKind::List,
                ),
                q(
                    "Which task should I have done or dropped today?",
                    "How could the day have gone better?",
                    AnswerKind::Text,
                ),
                q(
                    "What did I learn today?",
                    "Track personal and professional growth",
                    AnswerKind::Text,
                ),
            ],
        ),
        group("Health", "HEART", true, Timing::default(), vec![]),
    ]
}

pub fn routines() -> Vec<Routine> {
    let step = RoutineStep::new;
    vec![
        Routine {
            id: "morning".into(),
            name: "Morning".into(),
            contexts: vec!["work".into()],
            timing: Timing::new("09:45", "13:00", vec![OpenEvent::Start]),
            steps: vec![
                step("Plan day (Slack)", ""),
                step("Book meeting rooms", ""),
                step("Review Jira tasks assigned to me", ""),
                step("Give one piece of feedback today", ""),
            ],
        },
        Routine {
            id: "evening".into(),
            name: "Evening".into(),
            contexts: vec!["work".into()],
            timing: Timing::new("18:00", "", vec![OpenEvent::Off]),
            steps: vec![
                step("Time recording", "https://time.example.com"),
                step(
                    "Brag doc: anything worth logging today?",
                    "3-Personal/Work-Log.md",
                ),
            ],
        },
    ]
}

pub fn reminders() -> Vec<ReminderRule> {
    let routine = |id: &str| ReminderRule::Routine {
        enabled: true,
        routine: id.into(),
    };
    let journal = |group: &str| ReminderRule::Journal {
        enabled: true,
        group: group.into(),
    };
    vec![
        ReminderRule::Meeting {
            enabled: true,
            minutes_before: 60,
        },
        routine("morning"),
        routine("evening"),
        journal("Dawn"),
        journal("Dusk"),
        journal("Health"),
        ReminderRule::Todos {
            enabled: false,
            span: TodoSpan::Today,
            from: "16:00".into(),
            to: String::new(),
        },
    ]
}
