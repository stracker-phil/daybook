//! A routine's items for a day and which of them the plan section has ticked. Pure.

use serde::Serialize;
use ts_rs::TS;

use crate::store::settings::{Context, Feature, Routine};
use crate::vault::ops::plan::{is_step_done, read_plan_tasks};

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct RoutineStepView {
    /// The item's place in the routine, also on days that skip earlier items.
    pub index: u32,
    pub title: String,
    pub link: String,
    pub done: bool,
}

/// A routine as the wizard shows it: today's items, ticked or not.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct RoutineView {
    pub id: String,
    pub name: String,
    pub steps: Vec<RoutineStepView>,
}

/// A routine's progress as the board, the menus and the reminders show it.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct RoutineProgress {
    pub id: String,
    pub name: String,
    pub done: u32,
    pub total: u32,
}

fn count(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

/// The routines of `context`: none without the context or its Routines feature.
pub fn active_routines<'a>(routines: &'a [Routine], context: Option<&Context>) -> Vec<&'a Routine> {
    let Some(context) = context.filter(|context| context.features.contains(&Feature::Routines))
    else {
        return Vec::new();
    };
    routines
        .iter()
        .filter(|routine| routine.contexts.is_empty() || routine.contexts.contains(&context.id))
        .collect()
}

/// `routine` with the items due on `weekday` (0 = Sunday) and whether `lines`' plan section ticked them.
pub fn routine_view(
    routine: &Routine,
    lines: &[String],
    heading: &str,
    weekday: u8,
) -> RoutineView {
    let tasks = read_plan_tasks(lines, heading);
    RoutineView {
        id: routine.id.clone(),
        name: routine.name.clone(),
        steps: routine
            .steps_on(weekday)
            .into_iter()
            .map(|(index, step)| RoutineStepView {
                index: count(index),
                title: step.title.clone(),
                link: step.link.clone(),
                done: is_step_done(&tasks, &step.title),
            })
            .collect(),
    }
}

impl RoutineView {
    pub fn progress(&self) -> RoutineProgress {
        RoutineProgress {
            id: self.id.clone(),
            name: self.name.clone(),
            done: count(self.steps.iter().filter(|step| step.done).count()),
            total: count(self.steps.len()),
        }
    }

    pub fn has_open_steps(&self) -> bool {
        self.steps.iter().any(|step| !step.done)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::store::settings::{RoutineStep, Timing};

    const HEADING: &str = "## Plan for Today";

    fn step(title: &str, days: &[u8]) -> RoutineStep {
        RoutineStep {
            days: days.to_vec(),
            ..RoutineStep::new(title, "")
        }
    }

    fn routine(id: &str, contexts: &[&str], steps: Vec<RoutineStep>) -> Routine {
        Routine {
            id: id.into(),
            name: id.to_uppercase(),
            contexts: contexts.iter().map(|c| c.to_string()).collect(),
            steps,
            timing: Timing::default(),
        }
    }

    fn morning() -> Routine {
        routine(
            "morning",
            &[],
            vec![
                step("Plan day", &[]),
                step("Weekly report", &[5]),
                step("Book rooms", &[1, 3]),
            ],
        )
    }

    fn plan() -> Vec<String> {
        [
            "## Plan for Today",
            "- [x] book rooms",
            "- [ ] Plan day",
            "",
        ]
        .map(String::from)
        .to_vec()
    }

    fn context(id: &str, features: &[&str]) -> Context {
        Context::read_all(&[json!({ "name": id, "id": id, "features": features })]).remove(0)
    }

    fn titles(view: &RoutineView) -> Vec<&str> {
        view.steps.iter().map(|s| s.title.as_str()).collect()
    }

    #[test]
    fn monday_lists_the_daily_and_monday_items_with_their_place_and_ticks() {
        let view = routine_view(&morning(), &plan(), HEADING, 1);

        let steps: Vec<(u32, &str, bool)> = view
            .steps
            .iter()
            .map(|s| (s.index, s.title.as_str(), s.done))
            .collect();
        assert_eq!(steps, vec![(0, "Plan day", false), (2, "Book rooms", true)]);
    }

    #[test]
    fn friday_lists_the_daily_and_friday_items() {
        let view = routine_view(&morning(), &plan(), HEADING, 5);

        assert_eq!(titles(&view), vec!["Plan day", "Weekly report"]);
    }

    #[test]
    fn a_day_without_matching_items_keeps_only_the_daily_ones() {
        let view = routine_view(&morning(), &plan(), HEADING, 0);

        assert_eq!(titles(&view), vec!["Plan day"]);
    }

    #[test]
    fn the_view_carries_the_routine_identity_and_the_item_link() {
        let routine = routine(
            "evening",
            &[],
            vec![RoutineStep::new("Open log", "3-Personal/Work-Log")],
        );

        let view = routine_view(&routine, &[], HEADING, 2);

        assert_eq!(view.id, "evening");
        assert_eq!(view.name, "EVENING");
        assert_eq!(view.steps[0].link, "3-Personal/Work-Log");
    }

    #[test]
    fn progress_counts_the_ticked_items_of_the_day() {
        let progress = routine_view(&morning(), &plan(), HEADING, 1).progress();

        assert_eq!(
            progress,
            RoutineProgress {
                id: "morning".into(),
                name: "MORNING".into(),
                done: 1,
                total: 2,
            }
        );
    }

    #[test]
    fn without_plan_lines_nothing_is_done() {
        let progress = routine_view(&morning(), &[], HEADING, 1).progress();

        assert_eq!((progress.done, progress.total), (0, 2));
    }

    #[test]
    fn a_routine_has_open_steps_until_every_item_is_ticked() {
        let all_done = ["## Plan for Today", "- [x] Plan day", "- [X] book rooms"]
            .map(String::from)
            .to_vec();

        assert!(routine_view(&morning(), &plan(), HEADING, 1).has_open_steps());
        assert!(!routine_view(&morning(), &all_done, HEADING, 1).has_open_steps());
    }

    #[test]
    fn a_view_without_steps_has_no_open_steps() {
        let empty = routine("empty", &[], vec![]);

        assert!(!routine_view(&empty, &[], HEADING, 1).has_open_steps());
    }

    #[test]
    fn no_context_has_no_routines() {
        assert!(active_routines(&[morning()], None).is_empty());
    }

    #[test]
    fn a_context_without_the_routines_feature_has_no_routines() {
        let work = context("work", &["status", "slack"]);

        assert!(active_routines(&[morning()], Some(&work)).is_empty());
    }

    #[test]
    fn a_context_gets_the_routines_for_all_contexts_and_its_own_in_settings_order() {
        let routines = [
            routine("a", &["work"], vec![]),
            routine("b", &["private"], vec![]),
            routine("c", &[], vec![]),
            routine("d", &["private", "work"], vec![]),
        ];
        let work = context("work", &["routines"]);

        let ids: Vec<&str> = active_routines(&routines, Some(&work))
            .iter()
            .map(|r| r.id.as_str())
            .collect();

        assert_eq!(ids, vec!["a", "c", "d"]);
    }

    #[test]
    fn progress_and_step_views_serialize_with_the_frontend_field_names() {
        let view = routine_view(
            &routine(
                "m",
                &[],
                vec![RoutineStep::new("Plan day", "https://x.test")],
            ),
            &[],
            HEADING,
            1,
        );

        assert_eq!(
            serde_json::to_value(view.progress()).unwrap(),
            json!({ "id": "m", "name": "M", "done": 0, "total": 1 })
        );
        assert_eq!(
            serde_json::to_value(&view.steps[0]).unwrap(),
            json!({ "index": 0, "title": "Plan day", "link": "https://x.test", "done": false })
        );
    }
}
