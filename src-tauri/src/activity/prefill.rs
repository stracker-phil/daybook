//! `activity:prefill:<project>:<note>[:<ttProjectId>[:<tag>]]`: the activity overlay with a project and
//! note filled in. Each part is percent-encoded like `encodeURIComponent`, `:` too. Pure.

use crate::tools::{link_argument, link_target};

pub const PREFIX: &str = "activity:prefill:";

/// What a prefill command fills in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Prefill {
    /// The Daybook project.
    pub project: Option<String>,
    pub note: String,
    /// TimeTracker's project, when the pick named one (a recent timer).
    pub tt_project_id: Option<String>,
    pub tag: Option<String>,
}

/// The family's argument for `prefill`.
pub fn prefill_argument(prefill: &Prefill) -> String {
    let mut parts = vec![
        link_argument(prefill.project.as_deref().unwrap_or_default()),
        link_argument(&prefill.note),
    ];
    if let Some(id) = &prefill.tt_project_id {
        parts.push(link_argument(id));
        parts.extend(prefill.tag.as_deref().map(link_argument));
    }
    parts.join(":")
}

pub fn prefill_command(prefill: &Prefill) -> String {
    format!("{PREFIX}{}", prefill_argument(prefill))
}

fn non_empty(part: Option<String>) -> Option<String> {
    part.filter(|part| !part.is_empty())
}

/// The prefill behind the family's argument.
pub fn parse_prefill(argument: &str) -> Prefill {
    let mut parts = argument.split(':').map(link_target);
    Prefill {
        project: non_empty(parts.next()),
        note: parts.next().unwrap_or_default(),
        tt_project_id: non_empty(parts.next()),
        tag: non_empty(parts.next()),
    }
}

/// `daybook activity:prefill Oscorp QRS-1 review`: the first word is the project, the rest the note.
pub fn argument_from_words(words: &[String]) -> Result<String, String> {
    let Some((project, note)) = words.split_first() else {
        return Err("Needs a project and a note.".into());
    };
    Ok(prefill_argument(&Prefill {
        project: Some(project.clone()),
        note: note.join(" "),
        ..Prefill::default()
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prefill(project: Option<&str>, note: &str) -> Prefill {
        Prefill {
            project: project.map(str::to_string),
            note: note.into(),
            ..Prefill::default()
        }
    }

    fn argument(command: &str) -> &str {
        command.strip_prefix(PREFIX).unwrap()
    }

    #[test]
    fn the_command_encodes_the_project_and_the_note() {
        assert_eq!(
            prefill_command(&prefill(Some("Oscorp"), "QRS-1: review")),
            "activity:prefill:Oscorp:QRS-1%3A%20review"
        );
    }

    #[test]
    fn a_prefill_round_trips_through_its_command() {
        let original = prefill(Some("Osc:orp / 50%"), "QRS-1: review ünï");
        let command = prefill_command(&original);
        assert_eq!(parse_prefill(argument(&command)), original);
    }

    #[test]
    fn an_empty_project_parses_as_none() {
        let command = prefill_command(&prefill(None, "note"));
        assert_eq!(command, "activity:prefill::note");
        assert_eq!(parse_prefill(argument(&command)), prefill(None, "note"));
    }

    #[test]
    fn the_tracker_project_and_tag_round_trip() {
        let original = Prefill {
            project: Some("Oscorp".into()),
            note: "review".into(),
            tt_project_id: Some("55555555-5555".into()),
            tag: Some("Deep Work".into()),
        };
        let command = prefill_command(&original);
        assert_eq!(
            command,
            "activity:prefill:Oscorp:review:55555555-5555:Deep%20Work"
        );
        assert_eq!(parse_prefill(argument(&command)), original);
    }

    #[test]
    fn the_tracker_project_alone_round_trips_without_a_tag() {
        let original = Prefill {
            tt_project_id: Some("sdk".into()),
            ..prefill(Some("Oscorp"), "review")
        };
        let command = prefill_command(&original);
        assert_eq!(command, "activity:prefill:Oscorp:review:sdk");
        assert_eq!(parse_prefill(argument(&command)), original);
    }

    #[test]
    fn a_tag_without_a_tracker_project_is_not_part_of_the_command() {
        let original = Prefill {
            tag: Some("Dev".into()),
            ..prefill(Some("Oscorp"), "review")
        };
        assert_eq!(prefill_command(&original), "activity:prefill:Oscorp:review");
    }

    #[test]
    fn a_short_argument_fills_what_it_has() {
        assert_eq!(parse_prefill("Oscorp"), prefill(Some("Oscorp"), ""));
        assert_eq!(parse_prefill(""), prefill(None, ""));
    }

    #[test]
    fn the_first_word_is_the_project_and_the_rest_the_note() {
        let words: Vec<String> = ["Oscorp", "QRS-1", "review"].map(String::from).to_vec();
        let argument = argument_from_words(&words).unwrap();
        assert_eq!(
            parse_prefill(&argument),
            prefill(Some("Oscorp"), "QRS-1 review")
        );
    }

    #[test]
    fn a_project_alone_gives_an_empty_note() {
        let argument = argument_from_words(&["Oscorp".to_string()]).unwrap();
        assert_eq!(parse_prefill(&argument), prefill(Some("Oscorp"), ""));
    }

    #[test]
    fn no_words_is_an_error() {
        assert_eq!(
            argument_from_words(&[]),
            Err("Needs a project and a note.".to_string())
        );
    }
}
