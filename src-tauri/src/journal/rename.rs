//! A journal group renamed in Settings keeps its reminder rule (and with it its place among the
//! reminders). Pure.

use crate::store::settings::{JournalGroup, ReminderRule};

/// (old, new) titles of groups renamed in place: same position, the old title unused afterwards and the
/// new one unused before.
fn renamed<'a>(previous: &'a [JournalGroup], next: &'a [JournalGroup]) -> Vec<(&'a str, &'a str)> {
    if previous.len() != next.len() {
        return Vec::new();
    }
    previous
        .iter()
        .zip(next)
        .filter(|(before, after)| {
            before.title != after.title
                && !next.iter().any(|group| group.title == before.title)
                && !previous.iter().any(|group| group.title == after.title)
        })
        .map(|(before, after)| (before.title.as_str(), after.title.as_str()))
        .collect()
}

pub fn follow_renamed_groups(
    previous: &[JournalGroup],
    next: &[JournalGroup],
    rules: &mut [ReminderRule],
) {
    let moves = renamed(previous, next);
    for rule in rules.iter_mut() {
        if let ReminderRule::Journal { group, .. } = rule
            && let Some((_, to)) = moves.iter().find(|(from, _)| from == group)
        {
            *group = to.to_string();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::Timing;

    fn groups(titles: &[&str]) -> Vec<JournalGroup> {
        titles
            .iter()
            .map(|title| JournalGroup {
                title: (*title).into(),
                callout: "NOTE".into(),
                folded: false,
                questions: Vec::new(),
                timing: Timing::default(),
            })
            .collect()
    }

    fn journal(group: &str, enabled: bool) -> ReminderRule {
        ReminderRule::Journal {
            enabled,
            group: group.into(),
        }
    }

    fn routine(id: &str) -> ReminderRule {
        ReminderRule::Routine {
            enabled: true,
            routine: id.into(),
        }
    }

    fn followed(
        previous: &[&str],
        next: &[&str],
        mut rules: Vec<ReminderRule>,
    ) -> Vec<ReminderRule> {
        follow_renamed_groups(&groups(previous), &groups(next), &mut rules);
        rules
    }

    #[test]
    fn a_renamed_group_moves_its_rule_keeping_enabled_and_position() {
        let rules = vec![
            routine("morning"),
            journal("Dawn", true),
            journal("Dusk", false),
            journal("Health", true),
        ];
        assert_eq!(
            followed(
                &["Dawn", "Dusk", "Health"],
                &["Dawn", "Nacht", "Health"],
                rules
            ),
            vec![
                routine("morning"),
                journal("Dawn", true),
                journal("Nacht", false),
                journal("Health", true),
            ]
        );
    }

    #[test]
    fn rules_stay_when_nothing_was_renamed() {
        let rules = vec![journal("Dawn", true), journal("Dusk", true)];
        let cases: [(&[&str], &[&str]); 4] = [
            (&["Dawn", "Dusk"], &["Dawn", "Dusk"]),
            (&["Dawn", "Dusk"], &["Dusk", "Dawn"]),
            (&["Dawn", "Dusk"], &["Dawn", "Dusk", "Neu"]),
            (&["Dawn", "Dusk"], &["Dawn"]),
        ];
        for (previous, next) in cases {
            assert_eq!(
                followed(previous, next, rules.clone()),
                rules,
                "{previous:?} -> {next:?}"
            );
        }
    }

    #[test]
    fn a_rename_together_with_an_added_or_removed_group_changes_nothing() {
        let rules = vec![journal("Dawn", true)];
        assert_eq!(
            followed(&["Dawn"], &["Frueh", "Dusk"], rules.clone()),
            rules
        );
        assert_eq!(
            followed(&["Dawn", "Dusk"], &["Frueh"], rules.clone()),
            rules
        );
    }

    #[test]
    fn a_rename_onto_an_existing_title_changes_nothing() {
        let rules = vec![journal("Dawn", true), journal("Dusk", true)];
        assert_eq!(
            followed(&["Dawn", "Dusk"], &["Dusk", "Dusk"], rules.clone()),
            rules
        );
    }

    #[test]
    fn only_journal_rules_of_the_renamed_group_follow() {
        let rules = vec![routine("Dawn"), journal("Dusk", true)];
        assert_eq!(
            followed(&["Dawn", "Dusk"], &["Frueh", "Dusk"], rules.clone()),
            rules
        );
    }
}
