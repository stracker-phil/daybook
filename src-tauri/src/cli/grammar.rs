//! The words the `daybook` command understands: own actions first, else one of the app's commands
//! by the words of its id. Pure; the running app parses, since it knows the commands and their choices.

use super::help::{TOOL, choice_rows, command_list, group_help, help_text, words_of};
use crate::automation::{Action, Params};
use crate::bus::{ArgChoice, FromWords};

/// A command as the command line knows it; a family (`arg` set) takes words after its id.
#[derive(Clone, Debug, Default)]
pub struct CliCommand {
    /// `music:toggle`, or a family's prefix such as `music:play:`.
    pub id: String,
    pub help: String,
    /// Families: the argument as help shows it, e.g. `<entry>`.
    pub arg: Option<String>,
    /// What the argument can be right now: the words must name one. Without it any words go.
    pub choices: Option<Vec<ArgChoice>>,
    pub from_words: Option<FromWords>,
}

/// What the app itself does for a word of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppWord {
    Focus,
    Hide,
    Quit,
    /// The version and whether the window shows.
    Report,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Plan {
    Print(String),
    /// Words the command can't use; exit code 2.
    Usage(String),
    Action {
        action: Action,
        params: Params,
    },
    Command(String),
    App(AppWord),
}

pub fn is_status(word: &str) -> bool {
    matches!(word, "working" | "break" | "off")
}

fn usage(message: impl Into<String>) -> Plan {
    Plan::Usage(message.into())
}

fn action(action: Action, params: &[(&str, String)]) -> Plan {
    Plan::Action {
        action,
        params: params
            .iter()
            .map(|(k, v)| (k.to_string(), v.clone()))
            .collect(),
    }
}

fn help(catalog: &[CliCommand], group: Option<&str>) -> Plan {
    let Some(group) = group else {
        return Plan::Print(help_text(catalog));
    };
    match group_help(catalog, &group.to_lowercase()) {
        Some(text) => Plan::Print(text),
        None => usage(format!(
            "There’s no “{group}” group. {TOOL} help lists them all."
        )),
    }
}

fn knows(catalog: &[CliCommand], id: &str) -> bool {
    catalog.iter().any(|command| match command.arg {
        Some(_) => id.starts_with(&command.id) && id.len() > command.id.len(),
        None => id == command.id,
    })
}

fn pick_from(family: &CliCommand) -> String {
    match family.choices.as_deref().unwrap_or_default() {
        [] => "There’s nothing to pick right now.".into(),
        choices => format!(
            "Pick one of:\n{}",
            choice_rows(choices, usize::MAX, "").join("\n")
        ),
    }
}

fn family_plan(family: &CliCommand, rest: &[String]) -> Plan {
    if let Some(from_words) = family.from_words {
        return match from_words(rest) {
            Ok(argument) => Plan::Command(format!("{}{argument}", family.id)),
            Err(message) => usage(message),
        };
    }
    let text = rest.join(" ");
    let Some(choices) = &family.choices else {
        return Plan::Command(format!("{}{text}", family.id));
    };
    let key = text.to_lowercase();
    let picked = choices.iter().find(|choice| {
        choice.word.to_lowercase() == key
            || choice
                .title
                .as_ref()
                .is_some_and(|t| t.to_lowercase() == key)
    });
    match picked {
        Some(choice) => Plan::Command(format!("{}{}", family.id, choice.argument)),
        None => {
            let name = family
                .arg
                .as_deref()
                .unwrap_or_default()
                .replace(['<', '>'], "");
            usage(format!("There’s no {name} “{text}”. {}", pick_from(family)))
        }
    }
}

fn unknown(args: &[String], catalog: &[CliCommand]) -> Plan {
    let said = format!("“{TOOL} {}”", args.join(" "));
    let first = args[0].to_lowercase();
    usage(match group_help(catalog, &first) {
        Some(group) => format!("Unknown command {said}. {TOOL} {first} offers:\n{group}"),
        None => format!("Unknown command {said}. {TOOL} help lists everything."),
    })
}

/// `daybook journal open` runs `journal:open`; a family takes the words after its prefix.
fn command_plan(args: &[String], catalog: &[CliCommand]) -> Plan {
    let words: Vec<String> = args.iter().map(|w| w.to_lowercase()).collect();
    if let Some(exact) = catalog
        .iter()
        .find(|c| c.arg.is_none() && words_of(&c.id) == words)
    {
        return Plan::Command(exact.id.clone());
    }
    let mut families: Vec<&CliCommand> = catalog.iter().filter(|c| c.arg.is_some()).collect();
    families.sort_by_key(|f| std::cmp::Reverse(words_of(&f.id).len()));
    for family in families {
        let prefix = words_of(&family.id);
        if words.len() < prefix.len() || words[..prefix.len()] != prefix[..] {
            continue;
        }
        let rest = &args[prefix.len()..];
        if !rest.is_empty() {
            return family_plan(family, rest);
        }
        let needs = format!(
            "{TOOL} {} needs {}.",
            prefix.join(" "),
            family.arg.as_deref().unwrap_or_default()
        );
        return usage(match family.choices {
            Some(_) => format!("{needs} {}", pick_from(family)),
            None => needs,
        });
    }
    unknown(args, catalog)
}

/// `slack <status> [note...]` or `slack custom <emoji> [text...]`.
fn slack_plan(rest: &[String]) -> Plan {
    let next = rest.first().map(|w| w.to_lowercase());
    match next.as_deref() {
        Some(status) if is_status(status) => action(
            Action::Slack,
            &[
                ("status", status.to_string()),
                ("note", rest[1..].join(" ")),
            ],
        ),
        Some("custom") if rest.len() >= 2 => action(
            Action::Slack,
            &[("emoji", rest[1].clone()), ("text", rest[2..].join(" "))],
        ),
        Some("custom") => usage(format!("{TOOL} slack custom needs an emoji, e.g. :tomato:")),
        _ => usage(format!("{TOOL} slack needs working, break, off or custom.")),
    }
}

/// `command <id>`, or `command --list`.
fn command_id_plan(rest: &[String], catalog: &[CliCommand]) -> Plan {
    match rest.first().map(String::as_str) {
        Some("--list" | "-l") => Plan::Print(command_list(catalog)),
        _ if rest.len() != 1 => usage(format!(
            "command needs one command id, e.g. {TOOL} command journal:open. {TOOL} commands lists them."
        )),
        _ if knows(catalog, &rest[0]) => Plan::Command(rest[0].clone()),
        _ => usage(format!(
            "Unknown command id “{}”. {TOOL} commands lists them.",
            rest[0]
        )),
    }
}

pub fn parse_cli(args: &[String], catalog: &[CliCommand]) -> Plan {
    if let Some(at) = args.iter().position(|a| a == "--help" || a == "-h") {
        return help(catalog, (at > 0).then(|| args[0].as_str()));
    }
    let Some((first, rest)) = args.split_first() else {
        return action(Action::Show, &[]);
    };
    let next = rest.first().map(|w| w.to_lowercase());
    let next = next.as_deref();
    match first.to_lowercase().as_str() {
        "show" if rest.is_empty() => return action(Action::Show, &[]),
        "show" => return usage(format!("{TOOL} show takes no words.")),
        "focus" if rest.is_empty() => return Plan::App(AppWord::Focus),
        "hide" if rest.is_empty() => return Plan::App(AppWord::Hide),
        "quit" if rest.is_empty() => return Plan::App(AppWord::Quit),
        "status" if rest.is_empty() => return Plan::App(AppWord::Report),
        "help" => return help(catalog, rest.first().map(String::as_str)),
        "commands" => return Plan::Print(command_list(catalog)),
        "state" => return action(Action::State, &[]),
        "status" if next.is_some_and(is_status) => {
            return action(
                Action::Status,
                &[
                    ("status", next.unwrap_or_default().to_string()),
                    ("note", rest[1..].join(" ")),
                ],
            );
        }
        "slack" => return slack_plan(rest),
        "command" => return command_id_plan(rest, catalog),
        _ => {}
    }
    command_plan(args, catalog)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(words: &[String]) -> Result<String, String> {
        match words.join("").parse::<u32>() {
            Ok(n) if n <= 100 => Ok(n.to_string()),
            _ => Err("0 to 100".into()),
        }
    }

    fn command(id: &str, help: &str) -> CliCommand {
        CliCommand {
            id: id.into(),
            help: help.into(),
            ..CliCommand::default()
        }
    }

    fn family(id: &str, help: &str, arg: &str, choices: Option<Vec<ArgChoice>>) -> CliCommand {
        CliCommand {
            arg: Some(arg.into()),
            choices,
            ..command(id, help)
        }
    }

    fn catalog() -> Vec<CliCommand> {
        vec![
            command("status:break", "Change Status… with Break picked"),
            command("status:toggle", "Change Status…"),
            command("focus:open", "Open the focus list"),
            command("app:quit", "Quit Daybook"),
            command("journal:open", "Journal…"),
            family(
                "journal:open:",
                "Journal… at one group",
                "<group>",
                Some(vec![ArgChoice::new("Morning Pages")]),
            ),
            command("music:toggle", "Play or pause Music"),
            command("music:play", "Play"),
            family(
                "music:play:",
                "Play an entry",
                "<entry>",
                Some(vec![ArgChoice {
                    word: "focus".into(),
                    title: Some("Focus Mix".into()),
                    argument: "focus".into(),
                }]),
            ),
            CliCommand {
                from_words: Some(volume),
                ..family("music:volume:", "Set Music’s volume", "<0-100>", None)
            },
            family(
                "mode:",
                "Switch to a context",
                "<context>",
                Some(Vec::new()),
            ),
            family("jira:issue:", "Open a Jira issue", "<key>", None),
        ]
    }

    fn plan(args: &[&str]) -> Plan {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        parse_cli(&args, &catalog())
    }

    fn picked(id: &str) -> Plan {
        Plan::Command(id.into())
    }

    fn params(pairs: &[(&str, &str)]) -> Params {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn usage_text(args: &[&str]) -> String {
        match plan(args) {
            Plan::Usage(message) => message,
            other => panic!("{args:?} is not a usage: {other:?}"),
        }
    }

    fn printed(args: &[&str]) -> String {
        match plan(args) {
            Plan::Print(text) => text,
            other => panic!("{args:?} does not print: {other:?}"),
        }
    }

    #[test]
    fn no_words_show_the_window() {
        assert_eq!(
            plan(&[]),
            Plan::Action {
                action: Action::Show,
                params: Params::new()
            }
        );
    }

    #[test]
    fn show_takes_no_words() {
        assert_eq!(
            plan(&["Show"]),
            Plan::Action {
                action: Action::Show,
                params: Params::new()
            }
        );
        assert_eq!(usage_text(&["show", "x"]), "daybook show takes no words.");
    }

    #[test]
    fn help_anywhere_prints_and_never_runs() {
        assert!(printed(&["--help"]).starts_with("Usage: daybook [command] [words...]"));
        assert!(printed(&["command", "--help"]).contains("daybook command <id>"));
        assert!(printed(&["music", "play", "-h"]).contains("Focus Mix"));
        assert!(
            printed(&["status", "break", "--help"]).contains("daybook status working|break|off")
        );
        assert!(printed(&["quit", "--help"]).contains("Quit Daybook"));
    }

    #[test]
    fn help_with_a_group_shows_that_group() {
        assert_eq!(printed(&["help", "MUSIC"]), printed(&["music", "--help"]));
        assert_eq!(printed(&["help"]), printed(&["--help"]));
        assert_eq!(
            usage_text(&["help", "nope"]),
            "There’s no “nope” group. daybook help lists them all."
        );
        assert_eq!(
            usage_text(&["nope", "--help"]),
            "There’s no “nope” group. daybook help lists them all."
        );
    }

    #[test]
    fn words_of_the_app_are_own_words_only_on_their_own() {
        for (word, expected) in [
            ("status", AppWord::Report),
            ("focus", AppWord::Focus),
            ("hide", AppWord::Hide),
            ("quit", AppWord::Quit),
            ("QUIT", AppWord::Quit),
        ] {
            assert_eq!(plan(&[word]), Plan::App(expected), "{word}");
        }
    }

    #[test]
    fn focus_followed_by_words_is_a_command() {
        assert_eq!(plan(&["focus", "open"]), picked("focus:open"));
        assert_eq!(plan(&["app", "quit"]), picked("app:quit"));
    }

    #[test]
    fn app_words_followed_by_unknown_words_are_unknown_commands() {
        let message = usage_text(&["quit", "now"]);
        assert!(
            message.starts_with("Unknown command “daybook quit now”. daybook quit offers:\n"),
            "{message}"
        );
    }

    #[test]
    fn state_is_an_action_and_ignores_extra_words() {
        for args in [&["state"][..], &["State", "now"]] {
            assert_eq!(
                plan(args),
                Plan::Action {
                    action: Action::State,
                    params: Params::new()
                },
                "{args:?}"
            );
        }
    }

    #[test]
    fn a_status_with_a_note_is_an_action() {
        assert_eq!(
            plan(&["status", "break", "lunch", "time"]),
            Plan::Action {
                action: Action::Status,
                params: params(&[("status", "break"), ("note", "lunch time")])
            }
        );
        assert_eq!(
            plan(&["Status", "OFF"]),
            Plan::Action {
                action: Action::Status,
                params: params(&[("status", "off"), ("note", "")])
            }
        );
    }

    #[test]
    fn a_word_after_status_that_is_no_status_is_looked_up_as_a_command() {
        assert_eq!(plan(&["status", "toggle"]), picked("status:toggle"));
        let message = usage_text(&["status", "foo"]);
        assert!(
            message.starts_with(
                "Unknown command “daybook status foo”. daybook status offers:\n  daybook status"
            ),
            "{message}"
        );
    }

    #[test]
    fn slack_sets_a_status_or_a_custom_one() {
        assert_eq!(
            plan(&["slack", "working", "deep", "work"]),
            Plan::Action {
                action: Action::Slack,
                params: params(&[("status", "working"), ("note", "deep work")])
            }
        );
        assert_eq!(
            plan(&["slack", "custom", ":tomato:", "Focus"]),
            Plan::Action {
                action: Action::Slack,
                params: params(&[("emoji", ":tomato:"), ("text", "Focus")])
            }
        );
        assert_eq!(
            plan(&["slack", "custom", ":tomato:"]),
            Plan::Action {
                action: Action::Slack,
                params: params(&[("emoji", ":tomato:"), ("text", "")])
            }
        );
    }

    #[test]
    fn slack_without_a_usable_second_word_is_a_usage() {
        for (args, message) in [
            (
                &["slack"][..],
                "daybook slack needs working, break, off or custom.",
            ),
            (
                &["slack", "loud"],
                "daybook slack needs working, break, off or custom.",
            ),
            (
                &["slack", "custom"],
                "daybook slack custom needs an emoji, e.g. :tomato:",
            ),
        ] {
            assert_eq!(usage_text(args), message, "{args:?}");
        }
    }

    #[test]
    fn the_command_word_runs_a_command_by_its_id() {
        for (args, expected) in [
            (&["command", "status:break"][..], picked("status:break")),
            (&["command", "mode:work"], picked("mode:work")),
            (&["command", "music:play:jazz"], picked("music:play:jazz")),
            (&["command", "journal:open"], picked("journal:open")),
        ] {
            assert_eq!(plan(args), expected, "{args:?}");
        }
    }

    #[test]
    fn the_command_word_refuses_unknown_ids_and_wrong_word_counts() {
        for (args, message) in [
            (
                &["command", "nope"][..],
                "Unknown command id “nope”. daybook commands lists them.",
            ),
            (
                &["command", "mode:"],
                "Unknown command id “mode:”. daybook commands lists them.",
            ),
            (
                &["command", "Status:Break"],
                "Unknown command id “Status:Break”. daybook commands lists them.",
            ),
            (
                &["command"],
                "command needs one command id, e.g. daybook command journal:open. daybook commands lists them.",
            ),
            (
                &["command", "a", "b"],
                "command needs one command id, e.g. daybook command journal:open. daybook commands lists them.",
            ),
        ] {
            assert_eq!(usage_text(args), message, "{args:?}");
        }
    }

    #[test]
    fn the_list_of_commands_comes_from_commands_and_command_list_flags() {
        let list = printed(&["commands"]);
        assert_eq!(printed(&["command", "--list"]), list);
        assert_eq!(printed(&["command", "-l"]), list);
        for line in [
            "music:play:focus\tPlay an entry: Focus Mix",
            "jira:issue:<key>\tOpen a Jira issue",
            "mode:<context>\tSwitch to a context",
        ] {
            assert!(list.lines().any(|l| l == line), "{line}");
        }
    }

    #[test]
    fn commands_match_by_the_words_of_their_id_without_regard_to_case() {
        for (args, id) in [
            (&["journal", "open"][..], "journal:open"),
            (&["Music", "PLAY"], "music:play"),
            (&["status", "toggle"], "status:toggle"),
        ] {
            assert_eq!(plan(args), picked(id), "{args:?}");
        }
    }

    #[test]
    fn a_family_takes_the_words_after_its_prefix() {
        for (args, id) in [
            (&["music", "play", "Focus", "mix"][..], "music:play:focus"),
            (&["music", "play", "focus"], "music:play:focus"),
            (
                &["journal", "open", "morning", "pages"],
                "journal:open:Morning Pages",
            ),
            (&["jira", "issue", "QRS-1"], "jira:issue:QRS-1"),
            (&["Jira", "Issue", "qrs", "1"], "jira:issue:qrs 1"),
            (&["music", "volume", "50"], "music:volume:50"),
        ] {
            assert_eq!(plan(args), picked(id), "{args:?}");
        }
    }

    #[test]
    fn a_family_argument_that_is_no_choice_lists_the_choices() {
        assert_eq!(
            usage_text(&["music", "play", "jazz"]),
            "There’s no entry “jazz”. Pick one of:\n      focus  Focus Mix"
        );
        assert_eq!(
            usage_text(&["mode", "work"]),
            "There’s no context “work”. There’s nothing to pick right now."
        );
    }

    #[test]
    fn a_family_without_words_says_what_it_needs() {
        assert_eq!(
            usage_text(&["mode"]),
            "daybook mode needs <context>. There’s nothing to pick right now."
        );
        assert_eq!(
            usage_text(&["jira", "issue"]),
            "daybook jira issue needs <key>."
        );
        assert_eq!(
            usage_text(&["journal", "open", "x"]),
            "There’s no group “x”. Pick one of:\n      Morning Pages"
        );
    }

    #[test]
    fn a_family_with_a_converter_reports_its_message() {
        assert_eq!(usage_text(&["music", "volume", "120"]), "0 to 100");
        assert_eq!(usage_text(&["music", "volume", "loud"]), "0 to 100");
    }

    #[test]
    fn unknown_words_point_to_the_group_or_to_the_help() {
        assert_eq!(
            usage_text(&["dance"]),
            "Unknown command “daybook dance”. daybook help lists everything."
        );
        let message = usage_text(&["Music", "louder"]);
        assert!(
            message.starts_with("Unknown command “daybook Music louder”. daybook music offers:\n"),
            "{message}"
        );
        assert!(message.contains("daybook music toggle"), "{message}");
    }

    #[test]
    fn without_a_catalog_only_own_words_and_help_work() {
        assert_eq!(
            parse_cli(&["journal".to_string(), "open".to_string()], &[]),
            Plan::Usage(
                "Unknown command “daybook journal open”. daybook help lists everything.".into()
            )
        );
        assert!(matches!(
            parse_cli(&["help".to_string()], &[]),
            Plan::Print(_)
        ));
    }
}
