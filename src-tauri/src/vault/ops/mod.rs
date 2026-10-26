//! A change to a daily note as data, so a change waiting for its note can be saved to disk and survive
//! a restart. Headings, placement and journal questions are looked up when it's applied.

pub mod focus;
pub mod journal;
pub mod notes;
pub mod plan;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use self::journal::JournalGroupAnswers;
use self::notes::NoteSectionRef;
use crate::store::settings::{DailySection, JournalGroup, Settings};
use crate::vault::note::Note;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum NoteOp {
    /// Rewrites one journal group's callout with these answers.
    JournalGroup {
        group: String,
        answers: JournalGroupAnswers,
    },
    /// Ticks (`done`) or unticks a routine item in the plan section.
    PlanStep { title: String, done: bool },
    /// A routine started: its items that aren't in the plan section yet are added unticked.
    PlanList { titles: Vec<String> },
    /// Replaces the day's focus list.
    Focus { body: String },
    /// Quick notes: rewrites (or adds) one section below the notes heading, maybe renaming it.
    NoteSection {
        title: String,
        level: u8,
        new_title: String,
        body: String,
        /// The section's text the editor started from; the op fails when the note holds another.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        expected: Option<String>,
    },
}

/// Where the ops write: the section headings and the order that places a missing one.
#[derive(Clone, Debug)]
pub struct NoteLayout {
    pub journal: String,
    pub focus: String,
    pub plan: String,
    pub notes: String,
    pub order: Vec<String>,
    pub journal_groups: Vec<JournalGroup>,
}

impl NoteLayout {
    pub fn from_settings(settings: &Settings) -> Self {
        let daily = &settings.daily_note;
        Self {
            journal: daily.heading(DailySection::Journal),
            focus: daily.heading(DailySection::Focus),
            plan: daily.heading(DailySection::Plan),
            notes: daily.heading(DailySection::Notes),
            order: daily.heading_order(),
            journal_groups: settings.journal.clone(),
        }
    }
}

fn count(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

impl NoteOp {
    /// One line for the action log: what the op writes.
    pub fn describe(&self) -> String {
        match self {
            Self::JournalGroup { group, .. } => format!("Journal: {group}"),
            Self::PlanStep { title, done } => {
                format!(
                    "Plan for Today: - [{}] {title}",
                    if *done { "x" } else { " " }
                )
            }
            Self::PlanList { titles } => format!(
                "Plan for Today: {} added if missing",
                count(titles.len(), "item", "items")
            ),
            Self::Focus { body } => {
                let lines = body.lines().filter(|line| !line.trim().is_empty()).count();
                format!("Focus: {}", count(lines, "line", "lines"))
            }
            Self::NoteSection {
                title,
                level,
                new_title,
                ..
            } => {
                let renamed = if new_title == title {
                    String::new()
                } else {
                    format!(" (was {title})")
                };
                format!(
                    "Notes: {} {new_title}{renamed}",
                    "#".repeat((*level).into())
                )
            }
        }
    }

    fn edit(&self, note: &mut Note, layout: &NoteLayout) -> Result<(), String> {
        let order = &layout.order;
        match self {
            Self::JournalGroup { group, answers } => journal::write_journal_group(
                note,
                &layout.journal,
                order,
                &layout.journal_groups,
                group,
                answers,
            )?,
            Self::PlanStep { title, done } => {
                plan::set_plan_task(note, &layout.plan, order, title, *done)
            }
            Self::PlanList { titles } => plan::add_plan_tasks(note, &layout.plan, order, titles),
            Self::Focus { body } => focus::write_focus(note, &layout.focus, order, body),
            Self::NoteSection {
                title,
                level,
                new_title,
                body,
                expected,
            } => {
                let section = NoteSectionRef {
                    title: title.clone(),
                    level: *level,
                };
                if let Some(expected) = expected
                    && !notes::section_holds(&note.lines, &layout.notes, &section, expected)
                {
                    return Err(notes::SECTION_CHANGED.into());
                }
                notes::write_note_section(note, &layout.notes, order, &section, new_title, body);
            }
        }
        Ok(())
    }

    /// The note's new text, `None` when the op changes nothing. An error means the op can't apply
    /// (e.g. its journal group is gone).
    pub fn apply(&self, text: &str, layout: &NoteLayout) -> Result<Option<String>, String> {
        let mut note = Note::parse(text);
        let before = note.lines.clone();
        self.edit(&mut note, layout)?;
        Ok((note.lines != before).then(|| note.serialize()))
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn layout() -> NoteLayout {
        NoteLayout::from_settings(&Settings::default())
    }

    fn plan_step(title: &str, done: bool) -> NoteOp {
        NoteOp::PlanStep {
            title: title.into(),
            done,
        }
    }

    fn note_section(title: &str, new_title: &str, level: u8, body: &str) -> NoteOp {
        NoteOp::NoteSection {
            title: title.into(),
            level,
            new_title: new_title.into(),
            body: body.into(),
            expected: None,
        }
    }

    fn expecting(title: &str, body: &str, expected: &str) -> NoteOp {
        NoteOp::NoteSection {
            title: title.into(),
            level: 3,
            new_title: title.into(),
            body: body.into(),
            expected: Some(expected.into()),
        }
    }

    const HELD: &str = "## Notes\n\n### A\n\n- one\n- two\n";

    #[test]
    fn a_note_section_op_writes_when_the_note_holds_the_expected_text() {
        assert_eq!(
            expecting("A", "- three", "- one\n- two").apply(HELD, &layout()),
            Ok(Some("## Notes\n\n### A\n\n- three\n".into()))
        );
    }

    #[test]
    fn a_note_section_op_fails_when_the_note_holds_other_text() {
        assert_eq!(
            expecting("A", "- three", "- other").apply(HELD, &layout()),
            Err(notes::SECTION_CHANGED.to_string())
        );
    }

    #[test]
    fn an_empty_expectation_adds_a_missing_topic_and_fails_on_a_topic_with_text() {
        assert_eq!(
            expecting("A", "", "").apply("# Day\n", &layout()),
            Ok(Some("# Day\n\n## Notes\n\n### A\n".into()))
        );
        assert_eq!(
            expecting("A", "", "").apply(HELD, &layout()),
            Err(notes::SECTION_CHANGED.to_string())
        );
    }

    #[test]
    fn blank_lines_and_trailing_newlines_do_not_count_as_a_change() {
        for expected in ["- one\n- two\n", "\n\n- one\n- two\n\n", "- one\n- two  \n"] {
            assert_eq!(
                expecting("A", "- three", expected).apply(HELD, &layout()),
                Ok(Some("## Notes\n\n### A\n\n- three\n".into())),
                "{expected:?}"
            );
        }
    }

    #[test]
    fn a_note_section_op_with_an_expectation_round_trips() {
        let shape = json!({"kind": "noteSection", "title": "A", "level": 3, "newTitle": "B",
            "body": "x", "expected": "y"});
        let op: NoteOp = serde_json::from_value(shape.clone()).unwrap();
        assert_eq!(
            op,
            NoteOp::NoteSection {
                title: "A".into(),
                level: 3,
                new_title: "B".into(),
                body: "x".into(),
                expected: Some("y".into()),
            }
        );
        assert_eq!(serde_json::to_value(&op).unwrap(), shape);
    }

    #[test]
    fn ops_written_by_the_prototype_read_back_as_the_same_json() {
        let shapes = [
            json!({"kind": "journalGroup", "group": "Dawn", "answers": {
                "answers": {"Q": {"items": ["a"], "tasks": [{"text": "t", "done": true}], "text": "x"}},
                "note": "n"
            }}),
            json!({"kind": "planStep", "title": "Plan day", "done": true}),
            json!({"kind": "planList", "titles": ["A", "B"]}),
            json!({"kind": "focus", "body": "- `[ ]` One"}),
            json!({"kind": "noteSection", "title": "A", "level": 3, "newTitle": "B", "body": "x"}),
        ];
        for shape in shapes {
            let op: NoteOp = serde_json::from_value(shape.clone())
                .unwrap_or_else(|err| panic!("{shape}: {err}"));
            assert_eq!(serde_json::to_value(&op).unwrap(), shape);
        }
    }

    #[test]
    fn a_note_section_op_reads_its_camel_case_fields() {
        let op: NoteOp = serde_json::from_value(
            json!({"kind": "noteSection", "title": "A", "level": 3, "newTitle": "B", "body": "x"}),
        )
        .unwrap();
        assert_eq!(op, note_section("A", "B", 3, "x"));
    }

    #[test]
    fn removed_kinds_do_not_deserialize() {
        for shape in [
            json!({"kind": "activity", "time": "09:00", "label": "Working"}),
            json!({"kind": "status", "time": "09:00", "status": "working", "label": "Working"}),
            json!({"kind": "meeting", "start": "09:00", "end": "10:00", "label": "Call"}),
        ] {
            assert!(
                serde_json::from_value::<NoteOp>(shape.clone()).is_err(),
                "{shape}"
            );
        }
    }

    #[test]
    fn describe_names_what_each_op_writes() {
        let cases = [
            (
                NoteOp::JournalGroup {
                    group: "Dawn".into(),
                    answers: JournalGroupAnswers::default(),
                },
                "Journal: Dawn",
            ),
            (
                plan_step("Plan day", true),
                "Plan for Today: - [x] Plan day",
            ),
            (
                plan_step("Plan day", false),
                "Plan for Today: - [ ] Plan day",
            ),
            (
                NoteOp::PlanList {
                    titles: vec!["A".into()],
                },
                "Plan for Today: 1 item added if missing",
            ),
            (
                NoteOp::PlanList {
                    titles: vec!["A".into(), "B".into()],
                },
                "Plan for Today: 2 items added if missing",
            ),
            (
                NoteOp::PlanList { titles: Vec::new() },
                "Plan for Today: 0 items added if missing",
            ),
            (NoteOp::Focus { body: "- a".into() }, "Focus: 1 line"),
            (
                NoteOp::Focus {
                    body: "- a\n\n  \n- b\n".into(),
                },
                "Focus: 2 lines",
            ),
            (
                NoteOp::Focus {
                    body: "  \n".into(),
                },
                "Focus: 0 lines",
            ),
            (note_section("Review", "Review", 3, ""), "Notes: ### Review"),
            (
                note_section("Review", "Sprint review", 4, ""),
                "Notes: #### Sprint review (was Review)",
            ),
        ];
        for (op, expected) in cases {
            assert_eq!(op.describe(), expected, "{op:?}");
        }
    }

    #[test]
    fn the_layout_takes_its_headings_and_order_from_the_default_settings() {
        let layout = layout();
        assert_eq!(layout.journal, "## Journal");
        assert_eq!(layout.focus, "## Focus");
        assert_eq!(layout.plan, "## Plan for Today");
        assert_eq!(layout.notes, "## Notes");
        assert_eq!(
            layout.order,
            ["## Journal", "## Focus", "## Plan for Today", "## Notes"]
        );
        assert_eq!(layout.journal_groups, Settings::default().journal);
    }

    #[test]
    fn an_op_that_changes_nothing_returns_none() {
        let text = "# Day\n\n## Plan for Today\n\n- [x] Plan day\n";
        assert_eq!(plan_step("Plan day", true).apply(text, &layout()), Ok(None));
        let titles = NoteOp::PlanList {
            titles: vec!["Plan day".into()],
        };
        assert_eq!(titles.apply(text, &layout()), Ok(None));
        let focus = NoteOp::Focus {
            body: String::new(),
        };
        assert_eq!(focus.apply("# Day\n", &layout()), Ok(None));
    }

    #[test]
    fn a_plan_step_ticks_the_task_below_the_plan_heading() {
        let text = "# Day\n\n## Plan for Today\n\n- [ ] Plan day\n";
        assert_eq!(
            plan_step("Plan day", true).apply(text, &layout()),
            Ok(Some(
                "# Day\n\n## Plan for Today\n\n- [x] Plan day\n".into()
            ))
        );
    }

    #[test]
    fn a_missing_plan_section_is_created_at_its_place_in_the_order() {
        assert_eq!(
            plan_step("Plan day", false).apply("# Day\n\n## Notes\n", &layout()),
            Ok(Some(
                "# Day\n\n## Plan for Today\n\n- [ ] Plan day\n\n## Notes\n".into()
            ))
        );
    }

    #[test]
    fn a_plan_list_adds_the_missing_items_unticked() {
        let op = NoteOp::PlanList {
            titles: vec!["Plan day".into(), "Review".into()],
        };
        assert_eq!(
            op.apply("# Day\n\n## Plan for Today\n\n- [x] Plan day\n", &layout()),
            Ok(Some(
                "# Day\n\n## Plan for Today\n\n- [x] Plan day\n- [ ] Review\n".into()
            ))
        );
    }

    #[test]
    fn a_changed_plan_heading_moves_where_plan_steps_write() {
        let mut settings = Settings::default();
        settings.daily_note.plan.text = "Today".into();
        let layout = NoteLayout::from_settings(&settings);
        assert_eq!(layout.plan, "## Today");
        assert_eq!(
            plan_step("Plan day", true).apply("# Day\n\n## Today\n\n- [ ] Plan day\n", &layout),
            Ok(Some("# Day\n\n## Today\n\n- [x] Plan day\n".into()))
        );
    }

    #[test]
    fn a_focus_op_creates_the_focus_section_before_the_plan_and_notes() {
        let op = NoteOp::Focus {
            body: "- [ ] One".into(),
        };
        assert_eq!(
            op.apply("# Day\n\n## Notes\n", &layout()),
            Ok(Some("# Day\n\n## Focus\n\n- [ ] One\n\n## Notes\n".into()))
        );
    }

    #[test]
    fn a_note_section_op_creates_the_notes_section_last() {
        assert_eq!(
            note_section("A", "A", 3, "- hi").apply("# Day\n\n## Journal\n\nx\n", &layout()),
            Ok(Some(
                "# Day\n\n## Journal\n\nx\n\n## Notes\n\n### A\n\n- hi\n".into()
            ))
        );
    }

    #[test]
    fn a_note_section_op_renames_and_rewrites_the_section() {
        let text = "## Notes\n\n### A\n\nold\n";
        assert_eq!(
            note_section("A", "B", 3, "new").apply(text, &layout()),
            Ok(Some("## Notes\n\n### B\n\nnew\n".into()))
        );
    }

    #[test]
    fn a_journal_group_op_for_an_unknown_group_is_an_error() {
        let op = NoteOp::JournalGroup {
            group: "No such group".into(),
            answers: JournalGroupAnswers::default(),
        };
        assert!(op.apply("# Day\n", &layout()).is_err());
    }

    #[test]
    fn ops_serialize_with_a_kind_tag() {
        let value: Value = serde_json::to_value(NoteOp::PlanList { titles: Vec::new() }).unwrap();
        assert_eq!(value, json!({"kind": "planList", "titles": []}));
    }
}
