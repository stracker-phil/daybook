//! What Slack should show, decided without side effects: a running call, then a calendar heads-up,
//! then the status the owner picked.

use std::sync::LazyLock;

use regex::Regex;

use crate::store::settings::{Presence, SlackStatus};

const MAX_STATUS_TEXT: usize = 100;

static PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{(\w+)\}").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s{2,}").unwrap());
static INTERNAL: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)^Internal\b").unwrap());

/// Fills `{key}` placeholders; an unknown key becomes empty.
pub fn fill_template(template: &str, values: &[(&str, &str)]) -> String {
    let filled = PLACEHOLDER.replace_all(template, |caps: &regex::Captures| {
        values
            .iter()
            .find(|(key, _)| *key == &caps[1])
            .map_or("", |(_, value)| value)
            .to_string()
    });
    SPACES.replace_all(&filled, " ").trim().to_string()
}

/// The status text with `{note}` (the channel message) filled in, as long as Slack allows.
pub fn status_text(mapping: &SlackStatus, note: &str) -> String {
    fill_template(&mapping.text, &[("note", note)])
        .chars()
        .take(MAX_STATUS_TEXT)
        .collect()
}

fn presence_name(presence: Presence) -> &'static str {
    match presence {
        Presence::Auto => "active",
        Presence::Away => "away",
        Presence::Keep => "presence unchanged",
    }
}

/// `:coffee: Break lunch · away`, for the log.
pub fn describe_status(mapping: &SlackStatus, note: &str) -> String {
    let text = if mapping.keep_status {
        "Status unchanged".to_string()
    } else {
        let status = [mapping.emoji.clone(), status_text(mapping, note)]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" ");
        if status.is_empty() {
            "(cleared)".into()
        } else {
            status
        }
    };
    format!("{text} · {}", presence_name(mapping.presence))
}

pub fn changes_nothing(mapping: &SlackStatus) -> bool {
    mapping.keep_status && mapping.presence == Presence::Keep
}

/// The project named in the status text; Internal is left out.
pub fn focus_of(project: Option<&str>) -> String {
    INTERNAL
        .replace(project.unwrap_or_default(), "")
        .trim()
        .to_string()
}

/// `9:00 - 18:00 | Oscorp`, or `Focus: Oscorp` when the status has no text.
pub fn with_project(mapping: &SlackStatus, project: Option<&str>) -> SlackStatus {
    let focus = focus_of(project);
    if focus.is_empty() || mapping.keep_status {
        return mapping.clone();
    }
    let text = if mapping.text.is_empty() {
        format!("Focus: {focus}")
    } else {
        format!("{} | {focus}", mapping.text)
    };
    SlackStatus {
        text,
        ..mapping.clone()
    }
}

/// A calendar heads-up: the rule's status, cleared by Slack when the absence starts.
#[derive(Clone, Debug, PartialEq)]
pub struct HeadsUp {
    /// "Absence within 8 h"
    pub label: String,
    /// The rule's status with its placeholders filled in.
    pub mapping: SlackStatus,
    /// Unix seconds.
    pub expiration: i64,
}

/// The status the owner picked.
#[derive(Clone, Debug, PartialEq)]
pub struct Wanted {
    pub mapping: SlackStatus,
    pub note: String,
    /// The status's workflow (Settings → Workflows); off leaves Slack alone.
    pub workflow_on: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    Call(SlackStatus),
    HeadsUp {
        mapping: SlackStatus,
        expiration: i64,
        detail: String,
    },
    Status {
        mapping: SlackStatus,
        note: String,
    },
    WorkflowOff {
        mapping: SlackStatus,
        note: String,
    },
}

/// A call wins, then a heads-up, then the picked status. A heads-up with presence "no change" takes the
/// picked status's presence (active while Working, away on a break).
pub fn decide(call: Option<&SlackStatus>, heads_up: Option<&HeadsUp>, wanted: Wanted) -> Decision {
    if let Some(call) = call {
        return Decision::Call(call.clone());
    }
    if let Some(heads_up) = heads_up {
        let followed = if wanted.workflow_on {
            wanted.mapping.presence
        } else {
            Presence::Keep
        };
        let presence = match heads_up.mapping.presence {
            Presence::Keep => followed,
            presence => presence,
        };
        return Decision::HeadsUp {
            mapping: SlackStatus {
                presence,
                ..heads_up.mapping.clone()
            },
            expiration: heads_up.expiration,
            detail: format!("Status rule: {}", heads_up.label),
        };
    }
    let Wanted { mapping, note, .. } = wanted;
    if wanted.workflow_on {
        Decision::Status { mapping, note }
    } else {
        Decision::WorkflowOff { mapping, note }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn working() -> SlackStatus {
        SlackStatus::new(":white_check_mark:", "9:00 - 18:00", Presence::Auto)
    }

    fn call() -> SlackStatus {
        SlackStatus::new(":spiral_calendar_pad:", "In a Zoom call", Presence::Auto)
    }

    fn heads_up() -> HeadsUp {
        HeadsUp {
            label: "Absence within 8 h".into(),
            mapping: SlackStatus::new(":information_source:", "Away later", Presence::Keep),
            expiration: 1_800_000_000,
        }
    }

    fn wanted() -> Wanted {
        Wanted {
            mapping: working(),
            note: String::new(),
            workflow_on: true,
        }
    }

    #[test]
    fn a_running_call_wins_over_everything_else() {
        assert_eq!(
            decide(Some(&call()), Some(&heads_up()), wanted()),
            Decision::Call(call())
        );
    }

    #[test]
    fn a_heads_up_wins_over_the_picked_status_and_takes_its_presence_when_it_has_none() {
        let Decision::HeadsUp {
            mapping,
            expiration,
            detail,
        } = decide(None, Some(&heads_up()), wanted())
        else {
            panic!("expected a heads-up");
        };
        assert_eq!(
            mapping,
            SlackStatus::new(":information_source:", "Away later", Presence::Auto)
        );
        assert_eq!(expiration, 1_800_000_000);
        assert_eq!(detail, "Status rule: Absence within 8 h");
    }

    #[test]
    fn a_heads_up_keeps_presence_when_the_picked_status_workflow_is_off() {
        let off = Wanted {
            workflow_on: false,
            ..wanted()
        };
        let Decision::HeadsUp { mapping, .. } = decide(None, Some(&heads_up()), off) else {
            panic!("expected a heads-up");
        };
        assert_eq!(mapping.presence, Presence::Keep);
    }

    #[test]
    fn a_heads_up_with_its_own_presence_keeps_it() {
        let mut rule = heads_up();
        rule.mapping.presence = Presence::Away;
        let Decision::HeadsUp { mapping, .. } = decide(None, Some(&rule), wanted()) else {
            panic!("expected a heads-up");
        };
        assert_eq!(mapping.presence, Presence::Away);
    }

    #[test]
    fn the_picked_status_is_shown_or_left_alone_when_its_workflow_is_off() {
        let note = "focus time".to_string();
        assert_eq!(
            decide(
                None,
                None,
                Wanted {
                    note: note.clone(),
                    ..wanted()
                }
            ),
            Decision::Status {
                mapping: working(),
                note: note.clone()
            }
        );
        assert_eq!(
            decide(
                None,
                None,
                Wanted {
                    note: note.clone(),
                    workflow_on: false,
                    ..wanted()
                }
            ),
            Decision::WorkflowOff {
                mapping: working(),
                note
            }
        );
    }

    #[test]
    fn the_project_ends_the_status_text() {
        assert_eq!(
            with_project(&working(), Some("Oscorp")).text,
            "9:00 - 18:00 | Oscorp"
        );
        let no_text = SlackStatus {
            text: String::new(),
            ..working()
        };
        assert_eq!(with_project(&no_text, Some("Oscorp")).text, "Focus: Oscorp");
    }

    #[test]
    fn the_project_changes_nothing_but_the_text() {
        let with = with_project(&working(), Some("Oscorp"));
        assert_eq!(with.emoji, ":white_check_mark:");
        assert_eq!(with.presence, Presence::Auto);
    }

    #[test]
    fn internal_and_no_project_leave_the_status_as_it_is() {
        for project in [
            None,
            Some(""),
            Some("Internal"),
            Some("internal"),
            Some("  "),
        ] {
            assert_eq!(with_project(&working(), project), working(), "{project:?}");
        }
    }

    #[test]
    fn only_the_leading_internal_word_is_left_out_of_the_project() {
        for (project, expected) in [
            (Some("Internal tools"), "tools"),
            (Some("INTERNAL Ops"), "Ops"),
            (Some("Internally"), "Internally"),
            (Some("Not Internal"), "Not Internal"),
            (Some(" Oscorp "), "Oscorp"),
            (None, ""),
        ] {
            assert_eq!(focus_of(project), expected, "{project:?}");
        }
    }

    #[test]
    fn a_status_that_keeps_the_text_gets_no_project() {
        let keeping = SlackStatus {
            keep_status: true,
            ..working()
        };
        assert_eq!(with_project(&keeping, Some("Oscorp")), keeping);
    }

    #[test]
    fn a_status_is_described_for_the_action_log() {
        let keeping = SlackStatus {
            keep_status: true,
            presence: Presence::Keep,
            ..working()
        };
        let cleared = SlackStatus::new("", "", Presence::Away);
        let emoji_only = SlackStatus::new(":coffee:", "", Presence::Auto);
        let noted = SlackStatus::new(":speech_balloon:", "Busy: {note}", Presence::Keep);
        for (mapping, note, expected) in [
            (working(), "", ":white_check_mark: 9:00 - 18:00 · active"),
            (keeping, "", "Status unchanged · presence unchanged"),
            (cleared, "", "(cleared) · away"),
            (emoji_only, "", ":coffee: · active"),
            (
                noted,
                "writing",
                ":speech_balloon: Busy: writing · presence unchanged",
            ),
        ] {
            assert_eq!(describe_status(&mapping, note), expected, "{mapping:?}");
        }
    }

    #[test]
    fn a_status_changes_nothing_only_when_text_and_presence_are_both_kept() {
        for (keep_status, presence, expected) in [
            (true, Presence::Keep, true),
            (true, Presence::Auto, false),
            (false, Presence::Keep, false),
        ] {
            let mapping = SlackStatus {
                keep_status,
                presence,
                ..working()
            };
            assert_eq!(changes_nothing(&mapping), expected, "{mapping:?}");
        }
    }

    #[test]
    fn the_status_text_fills_the_note() {
        let mapping = SlackStatus::new(":memo:", "Writing {note}", Presence::Auto);
        assert_eq!(status_text(&mapping, "the report"), "Writing the report");
        assert_eq!(status_text(&mapping, ""), "Writing");
    }

    #[test]
    fn the_status_text_is_cut_at_100_characters() {
        let mapping = SlackStatus::new(":memo:", "{note}", Presence::Auto);
        let long = "ä".repeat(150);
        assert_eq!(status_text(&mapping, &long), "ä".repeat(100));
    }

    #[test]
    fn a_template_fills_known_keys_empties_unknown_ones_and_collapses_spaces() {
        for (template, expected) in [
            ("Hello {name}!", "Hello Ann!"),
            ("{name} {other} done", "Ann done"),
            ("  {other}  {name}  ", "Ann"),
            ("no placeholders", "no placeholders"),
            ("", ""),
        ] {
            assert_eq!(
                fill_template(template, &[("name", "Ann")]),
                expected,
                "{template:?}"
            );
        }
    }
}
