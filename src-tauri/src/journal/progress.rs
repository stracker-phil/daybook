//! The configured journal groups with the answers a note holds, and how far they are. Pure.

use serde::Serialize;
use ts_rs::TS;

use crate::store::settings::{AnswerKind, JournalGroup, JournalQuestion};
use crate::vault::note::{Note, is_blank};
use crate::vault::ops::journal::{
    JournalAnswer, JournalGroupAnswers, ParsedGroup, find_group, group_answers, is_answered,
    parse_journal_groups,
};

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct JournalQuestionView {
    pub question: String,
    pub hint: String,
    pub kind: AnswerKind,
    pub answer: JournalAnswer,
    pub answered: bool,
}

/// A group as the overlay shows it; one without questions is free text (`note`).
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct JournalGroupView {
    pub title: String,
    pub questions: Vec<JournalQuestionView>,
    pub answered: u32,
    pub total: u32,
    pub note: String,
}

/// A group's progress as the board, the menus and the reminders show it.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct JournalProgress {
    pub title: String,
    pub answered: u32,
    pub total: u32,
    /// A free-text group was written.
    pub has_note: bool,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

fn question_view(
    question: &JournalQuestion,
    answers: &mut JournalGroupAnswers,
) -> JournalQuestionView {
    let answer = answers
        .answers
        .remove(&question.question)
        .unwrap_or_default();
    JournalQuestionView {
        question: question.question.clone(),
        hint: question.hint.clone(),
        kind: question.kind,
        answered: is_answered(Some(&answer), question.kind),
        answer,
    }
}

fn group_view(config: &JournalGroup, found: Option<&ParsedGroup>) -> JournalGroupView {
    let mut answers = group_answers(config, found);
    let questions: Vec<JournalQuestionView> = config
        .questions
        .iter()
        .map(|question| question_view(question, &mut answers))
        .collect();
    JournalGroupView {
        title: config.title.clone(),
        answered: count(questions.iter().filter(|q| q.answered).count()),
        total: count(questions.len()),
        questions,
        note: answers.note,
    }
}

/// `groups` filled with the answers `note` holds under `heading`; all empty without a note.
pub fn journal_groups(
    note: Option<&str>,
    groups: &[JournalGroup],
    heading: &str,
) -> Vec<JournalGroupView> {
    let lines = note.map(|text| Note::parse(text).lines);
    let parsed = lines
        .as_deref()
        .and_then(|lines| parse_journal_groups(lines, heading));
    groups
        .iter()
        .map(|config| group_view(config, find_group(parsed.as_deref(), &config.title)))
        .collect()
}

impl JournalGroupView {
    pub fn progress(&self) -> JournalProgress {
        JournalProgress {
            title: self.title.clone(),
            answered: self.answered,
            total: self.total,
            has_note: self.questions.is_empty() && !is_blank(&self.note),
        }
    }
}

/// `(answered, total)` over all groups.
pub fn overall(groups: &[JournalProgress]) -> (u32, u32) {
    groups.iter().fold((0, 0), |(answered, total), group| {
        (answered + group.answered, total + group.total)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::{JournalQuestion, Timing};

    const NOTE: &str = "## Journal\n\n> [!NOTE]+ Dawn\n> **Grateful?**\n> 1. Sunshine\n\n> [!HEART]- Health\n> Slept well.\n";

    fn group(title: &str, callout: &str, questions: Vec<JournalQuestion>) -> JournalGroup {
        JournalGroup {
            title: title.into(),
            callout: callout.into(),
            folded: false,
            questions,
            timing: Timing::default(),
        }
    }

    fn configured() -> Vec<JournalGroup> {
        vec![
            group(
                "Dawn",
                "NOTE",
                vec![
                    JournalQuestion::new("Grateful?", "", AnswerKind::List),
                    JournalQuestion::new("Ziel", "", AnswerKind::Text),
                ],
            ),
            group("Health", "HEART", Vec::new()),
        ]
    }

    fn progress_of(note: Option<&str>, groups: &[JournalGroup]) -> Vec<JournalProgress> {
        journal_groups(note, groups, "## Journal")
            .iter()
            .map(JournalGroupView::progress)
            .collect()
    }

    #[test]
    fn groups_hold_the_answers_the_note_has() {
        let views = journal_groups(Some(NOTE), &configured(), "## Journal");
        assert_eq!(views.len(), 2);

        let morgen = &views[0];
        assert_eq!(morgen.title, "Dawn");
        assert_eq!(
            morgen
                .questions
                .iter()
                .map(|q| q.answered)
                .collect::<Vec<_>>(),
            vec![true, false]
        );
        assert_eq!(morgen.questions[0].answer.items, vec!["Sunshine".to_string()]);
        assert_eq!((morgen.answered, morgen.total), (1, 2));
        assert_eq!(morgen.note, "");

        let health = &views[1];
        assert!(health.questions.is_empty());
        assert_eq!(health.note, "Slept well.");
        assert_eq!((health.answered, health.total), (0, 0));
    }

    #[test]
    fn without_a_note_every_answer_is_empty() {
        let views = journal_groups(None, &configured(), "## Journal");
        assert!(
            views[0]
                .questions
                .iter()
                .all(|q| !q.answered && q.answer == JournalAnswer::default())
        );
        assert_eq!(views[1].note, "");
        assert_eq!(overall(&progress_of(None, &configured())), (0, 2));
    }

    #[test]
    fn overall_sums_answered_and_total_over_the_groups() {
        assert_eq!(overall(&progress_of(Some(NOTE), &configured())), (1, 2));
        assert_eq!(overall(&[]), (0, 0));
    }

    #[test]
    fn a_note_without_the_journal_section_leaves_everything_unanswered() {
        let progress = progress_of(
            Some("## Other\n\n> [!NOTE] Dawn\n> 1. Sunshine\n"),
            &configured(),
        );
        assert_eq!(overall(&progress), (0, 2));
        assert!(progress.iter().all(|p| !p.has_note));
    }

    #[test]
    fn a_group_missing_from_the_note_counts_as_unanswered() {
        let note = "## Journal\n\n> [!HEART]- Health\n> Slept well.\n";
        let views = journal_groups(Some(note), &configured(), "## Journal");
        assert_eq!((views[0].answered, views[0].total), (0, 2));
        assert!(views[0].questions.iter().all(|q| !q.answered));
        assert_eq!(views[1].note, "Slept well.");
    }

    #[test]
    fn group_titles_match_the_note_case_insensitively() {
        let note = "## Journal\n\n> [!NOTE]+ DAWN\n> **grateful?**\n> 1. Sunshine\n";
        let views = journal_groups(Some(note), &configured(), "## Journal");
        assert_eq!((views[0].answered, views[0].total), (1, 2));
    }

    #[test]
    fn a_free_text_group_has_a_note_only_when_written() {
        let written = progress_of(Some(NOTE), &configured());
        assert!(written[1].has_note);

        let empty = progress_of(
            Some("## Journal\n\n> [!HEART]- Health\n>\n"),
            &configured(),
        );
        assert!(!empty[1].has_note);
        assert!(!progress_of(None, &configured())[1].has_note);
    }

    #[test]
    fn a_group_with_questions_never_has_a_note() {
        let progress = progress_of(Some(NOTE), &configured());
        assert!(!progress[0].has_note);
    }

    #[test]
    fn progress_serializes_with_camel_case_fields() {
        let progress = progress_of(Some(NOTE), &configured());
        assert_eq!(
            serde_json::to_value(&progress[1]).unwrap(),
            serde_json::json!({
                "title": "Health",
                "answered": 0,
                "total": 0,
                "hasNote": true,
            })
        );
    }
}
