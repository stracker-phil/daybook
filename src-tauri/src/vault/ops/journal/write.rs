//! Writing one group's answers as its callout.

use super::{
    JournalAnswer, JournalGroupAnswers, NUMBERED, ParsedGroup, TASK, find_group, is_placeholder,
    key, parse_journal_groups, trimmed_block,
};
use crate::store::settings::{AnswerKind, JournalGroup, JournalQuestion};
use crate::vault::note::{Note, find_heading, insert_lines, is_blank, place_heading, section_end};

fn answer_lines(question: &JournalQuestion, answer: &JournalAnswer) -> Vec<String> {
    match question.kind {
        AnswerKind::List => answer
            .items
            .iter()
            .map(|item| item.trim())
            .filter(|item| !item.is_empty())
            .enumerate()
            .map(|(i, item)| format!("{}. {item}", i + 1))
            .collect(),
        AnswerKind::Task => answer
            .tasks
            .iter()
            .filter(|task| !is_blank(&task.text))
            .map(|task| {
                format!(
                    "- [{}] {}",
                    if task.done { "x" } else { " " },
                    task.text.trim()
                )
            })
            .collect(),
        // A blank line would end the answer when it's read back, so text answers have none.
        AnswerKind::Text => answer
            .text
            .lines()
            .map(str::trim_end)
            .filter(|line| !is_blank(line))
            .map(String::from)
            .collect(),
    }
}

fn quoted(header: String, body: &[String]) -> Vec<String> {
    let mut callout = vec![header];
    callout.extend(body.iter().map(|line| {
        if line.is_empty() {
            ">".to_string()
        } else {
            format!("> {line}")
        }
    }));
    callout
}

/// The callout for one group, or nothing when nothing is answered. `existing` keeps the callout's
/// own header (its fold state) and any questions the app doesn't know.
fn group_callout(
    config: &JournalGroup,
    answers: &JournalGroupAnswers,
    existing: Option<&ParsedGroup>,
) -> Vec<String> {
    let header = existing.map_or_else(
        || {
            let fold = if config.folded { "-" } else { "+" };
            format!("> [!{}]{fold} {}", config.callout, config.title)
        },
        |group| group.header.clone(),
    );
    if config.questions.is_empty() {
        let text: Vec<String> = answers
            .note
            .lines()
            .map(|line| line.trim_end().to_string())
            .collect();
        let text = trimmed_block(&text);
        return if text.is_empty() {
            Vec::new()
        } else {
            quoted(header, text)
        };
    }

    let empty = JournalAnswer::default();
    let mut blocks: Vec<Vec<String>> = Vec::new();
    for question in &config.questions {
        let lines = answer_lines(
            question,
            answers.answers.get(&question.question).unwrap_or(&empty),
        );
        if lines.is_empty() {
            continue;
        }
        let title = if question.hint.is_empty() {
            format!("**{}**", question.question)
        } else {
            format!("**{}** - _{}_", question.question, question.hint)
        };
        blocks.push([vec![title], lines].concat());
    }
    let is_known = |text: &str| {
        config
            .questions
            .iter()
            .any(|q| key(&q.question) == key(text))
    };
    for other in existing.map_or(&[][..], |group| &group.questions) {
        if is_known(&other.question) {
            continue;
        }
        let answered = other.slots.iter().any(|slot| {
            let text = TASK
                .captures(slot)
                .map(|task| task[2].to_string())
                .or_else(|| NUMBERED.captures(slot).map(|item| item[1].to_string()))
                .unwrap_or_else(|| slot.clone());
            !is_placeholder(&text)
        });
        if answered {
            blocks.push(other.raw.clone());
        }
    }
    if blocks.is_empty() {
        return Vec::new();
    }
    let blank_line = String::new();
    let body = blocks.join(&blank_line);
    quoted(header, &body)
}

/// Removes `lines[from..to]` and collapses the blank lines left around the gap.
fn remove_range(lines: &mut Vec<String>, from: usize, to: usize) {
    lines.drain(from..to);
    while from < lines.len() && is_blank(&lines[from]) && (from == 0 || is_blank(&lines[from - 1]))
    {
        lines.remove(from);
    }
}

/// Drops the journal heading when its section holds nothing but blank lines.
fn remove_empty_section(lines: &mut Vec<String>, heading: &str) {
    let Some(index) = find_heading(lines, heading) else {
        return;
    };
    let end = section_end(lines, index);
    if lines[index + 1..end].iter().any(|line| !is_blank(line)) {
        return;
    }
    remove_range(lines, index, end);
    // The blank line that separated the section from what came before it can go too.
    let at_gap = lines.get(index).is_none_or(|line| is_blank(line));
    if index > 0 && is_blank(&lines[index - 1]) && at_gap {
        lines.remove(index - 1);
    }
}

/// Writes one group's answers: replaces its callout, creates it (in Settings order, the section
/// too if needed) or removes it when nothing is answered any more.
pub fn write_journal_group(
    note: &mut Note,
    heading: &str,
    order: &[String],
    groups: &[JournalGroup],
    title: &str,
    answers: &JournalGroupAnswers,
) -> Result<(), String> {
    let config = groups
        .iter()
        .find(|group| key(&group.title) == key(title))
        .ok_or_else(|| format!("“{title}” isn't a journal group."))?;
    let parsed = parse_journal_groups(&note.lines, heading);
    let existing = find_group(parsed.as_deref(), &config.title);
    let callout = group_callout(config, answers, existing);

    if let Some(existing) = existing {
        if callout.is_empty() {
            remove_range(&mut note.lines, existing.start, existing.end);
            remove_empty_section(&mut note.lines, heading);
        } else {
            note.lines.splice(existing.start..existing.end, callout);
        }
        return Ok(());
    }
    if callout.is_empty() {
        return Ok(());
    }
    let Some(parsed) = parsed else {
        place_heading(note, heading, order, callout);
        return Ok(());
    };

    // Before the first group that comes later in Settings, else after the section's content.
    let position = |title: &str| {
        groups
            .iter()
            .position(|group| key(&group.title) == key(title))
    };
    let mine = position(&config.title);
    let later = parsed
        .iter()
        .find(|group| position(&group.title).is_some_and(|other| Some(other) > mine));
    let lines = &mut note.lines;
    if let Some(later) = later {
        insert_lines(lines, later.start, callout);
        return Ok(());
    }
    let index = find_heading(lines, heading).unwrap_or_default();
    let last = (index + 1..section_end(lines, index))
        .rev()
        .find(|&i| !is_blank(&lines[i]))
        .unwrap_or(index);
    insert_lines(lines, last + 1, [vec![String::new()], callout].concat());
    Ok(())
}
