//! Routines are checklists in a plain task list of today's note (default `## Plan for Today`): when a
//! routine starts its items are added unticked, ticking one writes `- [x]`. A line that already exists
//! is used in place.

use std::collections::HashMap;
use std::sync::LazyLock;

use regex::Regex;

use crate::vault::note::{Note, append_to_section, find_heading, section_end};

static TASK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(\s*[-*+]\s\[)([ xX])(\]\s+)(.*?)\s*$").unwrap());

fn key(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn mark(done: bool) -> &'static str {
    if done { "x" } else { " " }
}

fn task_line(title: &str, done: bool) -> String {
    format!("- [{}] {}", mark(done), title.trim())
}

/// Each task of the section by its title (case and spacing ignored): whether it is ticked.
pub fn read_plan_tasks(lines: &[String], heading: &str) -> HashMap<String, bool> {
    let mut tasks = HashMap::new();
    let Some(index) = find_heading(lines, heading) else {
        return tasks;
    };
    for line in &lines[index + 1..section_end(lines, index)] {
        if let Some(task) = TASK.captures(line) {
            tasks.insert(key(&task[4]), &task[2] != " ");
        }
    }
    tasks
}

pub fn is_step_done(tasks: &HashMap<String, bool>, title: &str) -> bool {
    tasks.get(&key(title)) == Some(&true)
}

/// Adds the items that aren't in the section yet, unticked and in order.
pub fn add_plan_tasks(note: &mut Note, heading: &str, order: &[String], titles: &[String]) {
    let existing = read_plan_tasks(&note.lines, heading);
    let missing: Vec<String> = titles
        .iter()
        .filter(|title| !existing.contains_key(&key(title)))
        .map(|title| task_line(title, false))
        .collect();
    if !missing.is_empty() {
        append_to_section(note, heading, missing, order);
    }
}

/// Ticks (or unticks) the item's line, adding it to the section if needed.
pub fn set_plan_task(note: &mut Note, heading: &str, order: &[String], title: &str, done: bool) {
    if let Some(index) = find_heading(&note.lines, heading) {
        let end = section_end(&note.lines, index);
        for line in &mut note.lines[index + 1..end] {
            let Some(task) = TASK.captures(line) else {
                continue;
            };
            if key(&task[4]) != key(title) {
                continue;
            }
            *line = format!("{}{}{}{}", &task[1], mark(done), &task[3], &task[4]);
            return;
        }
    }
    append_to_section(note, heading, vec![task_line(title, done)], order);
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADING: &str = "## Plan for Today";

    fn order() -> Vec<String> {
        ["## Journal", "## Plan for Today", "## Activity", "## Notes"]
            .map(String::from)
            .to_vec()
    }

    fn tick(text: &str, title: &str, done: bool) -> String {
        let mut note = Note::parse(text);
        set_plan_task(&mut note, HEADING, &order(), title, done);
        note.serialize()
    }

    #[test]
    fn ticks_an_existing_template_task_in_place() {
        let text =
            "# Day\n\n## Plan for Today\n\n- [ ] Plan day (Slack)\n- [ ] Book meeting rooms\n";
        assert_eq!(
            tick(text, "Book meeting rooms", true),
            "# Day\n\n## Plan for Today\n\n- [ ] Plan day (Slack)\n- [x] Book meeting rooms\n"
        );
    }

    #[test]
    fn adds_the_section_and_the_step_when_they_are_missing() {
        assert_eq!(
            tick("# Day\n\n## Notes\n", "Time recording", false),
            "# Day\n\n## Plan for Today\n\n- [ ] Time recording\n\n## Notes\n"
        );
    }

    #[test]
    fn adds_a_starting_routines_missing_items_unticked_and_keeps_existing_ones() {
        let mut note = Note::parse("# Day\n\n## Plan for Today\n\n- [x] Book meeting rooms\n");
        let titles = ["Plan day (Slack)", "Book meeting rooms"].map(String::from);
        add_plan_tasks(&mut note, HEADING, &order(), &titles);
        assert_eq!(
            note.serialize(),
            "# Day\n\n## Plan for Today\n\n- [x] Book meeting rooms\n- [ ] Plan day (Slack)\n"
        );
    }

    #[test]
    fn reads_which_steps_are_done() {
        let lines = [
            "## Plan for Today",
            "- [x] Book  meeting rooms",
            "- [ ] Other",
        ]
        .map(String::from);
        let tasks = read_plan_tasks(&lines, HEADING);
        assert!(is_step_done(&tasks, "Book meeting rooms"));
        assert!(!is_step_done(&tasks, "Other"));
    }
}
