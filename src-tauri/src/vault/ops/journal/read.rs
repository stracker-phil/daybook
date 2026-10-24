//! Reading the journal's callouts and the answers in them.

use super::{
    BULLET, CALLOUT_LINE, CALLOUT_START, JournalAnswer, JournalGroupAnswers, JournalTask, NUMBERED,
    ParsedGroup, ParsedQuestion, QUESTION, TASK, is_placeholder, key, trimmed_block,
};
use crate::store::settings::{AnswerKind, JournalGroup};
use crate::vault::note::{find_heading, is_blank, section_end};

/// Every callout in the journal section, in note order; `None` without a journal section.
pub fn parse_journal_groups(lines: &[String], heading: &str) -> Option<Vec<ParsedGroup>> {
    let index = find_heading(lines, heading)?;
    let mut groups: Vec<ParsedGroup> = Vec::new();
    let mut in_group = false;
    let mut in_question = false;
    for (i, line) in lines
        .iter()
        .enumerate()
        .take(section_end(lines, index))
        .skip(index + 1)
    {
        if let Some(start) = CALLOUT_START.captures(line) {
            let title = Some(start[3].trim())
                .filter(|title| !title.is_empty())
                .unwrap_or(start[1].trim());
            groups.push(ParsedGroup {
                title: title.to_string(),
                header: line.clone(),
                start: i,
                end: i + 1,
                questions: Vec::new(),
                body: Vec::new(),
            });
            in_group = true;
            in_question = false;
            continue;
        }
        let (Some(callout), true, Some(group)) =
            (CALLOUT_LINE.captures(line), in_group, groups.last_mut())
        else {
            in_group = false;
            in_question = false;
            continue;
        };
        let content = callout[1].to_string();
        group.end = i + 1;
        group.body.push(content.clone());
        if is_blank(&content) {
            in_question = false;
        } else if let Some(question) = QUESTION.captures(content.trim()) {
            group.questions.push(ParsedQuestion {
                question: question[1].trim().to_string(),
                slots: Vec::new(),
                raw: vec![content],
            });
            in_question = true;
        } else if in_question && let Some(question) = group.questions.last_mut() {
            question.slots.push(content.clone());
            question.raw.push(content);
        }
    }
    Some(groups)
}

fn read_answer(slots: &[String], kind: AnswerKind) -> JournalAnswer {
    match kind {
        AnswerKind::Task => JournalAnswer {
            tasks: slots
                .iter()
                .filter_map(|slot| TASK.captures(slot))
                .filter(|task| !is_placeholder(&task[2]))
                .map(|task| JournalTask {
                    text: task[2].trim().to_string(),
                    done: &task[1] != " ",
                })
                .collect(),
            ..Default::default()
        },
        AnswerKind::List => JournalAnswer {
            items: slots
                .iter()
                .map(|slot| {
                    let item = NUMBERED
                        .captures(slot)
                        .or_else(|| BULLET.captures(slot))
                        .map_or(slot.as_str(), |m| m.get(1).map_or("", |item| item.as_str()));
                    item.trim().to_string()
                })
                .filter(|item| !is_placeholder(item))
                .collect(),
            ..Default::default()
        },
        AnswerKind::Text => JournalAnswer {
            text: slots
                .iter()
                .map(|slot| slot.trim_end())
                .filter(|slot| !is_placeholder(slot))
                .collect::<Vec<_>>()
                .join("\n"),
            ..Default::default()
        },
    }
}

pub fn is_answered(answer: Option<&JournalAnswer>, kind: AnswerKind) -> bool {
    let Some(answer) = answer else {
        return false;
    };
    match kind {
        AnswerKind::Task => answer.tasks.iter().any(|task| !is_blank(&task.text)),
        AnswerKind::List => answer.items.iter().any(|item| !is_blank(item)),
        AnswerKind::Text => !is_blank(&answer.text),
    }
}

fn group_note(group: &ParsedGroup) -> String {
    trimmed_block(&group.body)
        .iter()
        .map(|line| line.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The note's answers to `config`'s questions, every question present (unanswered ones empty).
pub fn group_answers(config: &JournalGroup, parsed: Option<&ParsedGroup>) -> JournalGroupAnswers {
    let answers = config
        .questions
        .iter()
        .map(|question| {
            let found = parsed.and_then(|group| {
                group
                    .questions
                    .iter()
                    .find(|q| key(&q.question) == key(&question.question))
            });
            let answer = found.map_or_else(JournalAnswer::default, |q| {
                read_answer(&q.slots, question.kind)
            });
            (question.question.clone(), answer)
        })
        .collect();
    let note = match parsed {
        Some(group) if config.questions.is_empty() => group_note(group),
        _ => String::new(),
    };
    JournalGroupAnswers { answers, note }
}

pub fn find_group<'a>(groups: Option<&'a [ParsedGroup]>, title: &str) -> Option<&'a ParsedGroup> {
    groups?.iter().find(|group| key(&group.title) == key(title))
}
