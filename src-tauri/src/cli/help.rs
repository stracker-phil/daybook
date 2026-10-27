//! The help the `daybook` command prints: the own actions, then the app's commands by group. Pure.

use super::grammar::{CliCommand, is_status};
use crate::bus::ArgChoice;

pub const TOOL: &str = "daybook";
const MAX_WIDTH: usize = 44;
/// In the full help; `daybook help <group>` lists all of them.
const MAX_CHOICES: usize = 8;

struct Line<'a> {
    group: String,
    usage: String,
    help: String,
    choices: Option<&'a [ArgChoice]>,
}

/// What the command does itself; everything else is one of the app's commands. (group, usage, help)
const ACTIONS: [(&str, &str, &str); 13] = [
    ("", "", "Start Daybook or bring its window to the front"),
    (
        "show",
        "show",
        "Show the Daybook window without taking focus",
    ),
    ("focus", "focus", "Bring the Daybook window to the front"),
    ("hide", "hide", "Hide the Daybook window"),
    ("quit", "quit", "Quit Daybook"),
    (
        "status",
        "status",
        "Print the version and whether the window shows",
    ),
    (
        "status",
        "status working|break|off [note...]",
        "Set your status right away, without the overlay",
    ),
    (
        "slack",
        "slack working|break|off [note...]",
        "Set Slack like that status; Daybook’s own status stays",
    ),
    (
        "slack",
        "slack custom <emoji> [text...]",
        "Set any Slack status, e.g. daybook slack custom :tomato: Focus",
    ),
    (
        "state",
        "state",
        "Print the status, context, call, event and music as JSON",
    ),
    ("command", "command <id>", "Run a command by its id"),
    (
        "commands",
        "commands",
        "List every command id (also: daybook command --list)",
    ),
    (
        "help",
        "help [group]",
        "This help, or one group with all it can pick",
    ),
];

/// Own whatever follows them.
const OWN_WORDS: [&str; 6] = ["show", "slack", "state", "command", "commands", "help"];
/// Own only on their own.
const OWN_ALONE: [&str; 4] = ["focus", "hide", "quit", "status"];

fn action_lines<'a>() -> impl Iterator<Item = Line<'a>> {
    ACTIONS.iter().map(|(group, usage, help)| Line {
        group: group.to_string(),
        usage: usage.to_string(),
        help: help.to_string(),
        choices: None,
    })
}

/// Words the command handles itself before it looks for a command.
pub fn is_own(words: &[String]) -> bool {
    let Some(first) = words.first() else {
        return false;
    };
    OWN_WORDS.contains(&first.as_str())
        || (first == "status" && words.get(1).is_some_and(|w| is_status(w)))
        || (words.len() == 1 && OWN_ALONE.contains(&first.as_str()))
}

pub fn words_of(id: &str) -> Vec<String> {
    id.split(':')
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect()
}

fn command_line(command: &CliCommand) -> Line<'_> {
    let words = words_of(&command.id);
    // A command the own words would catch is still reachable by its id.
    let usage = if is_own(&words) {
        format!("command {}", command.id)
    } else {
        words.join(" ")
    };
    Line {
        group: words.first().cloned().unwrap_or_default(),
        usage: match &command.arg {
            Some(arg) => format!("{usage} {arg}"),
            None => usage,
        },
        help: command.help.clone(),
        choices: command.choices.as_deref(),
    }
}

/// Lines of one group together, the groups in the order they first appear.
fn grouped(lines: Vec<Line<'_>>) -> Vec<Vec<Line<'_>>> {
    let mut groups: Vec<Vec<Line>> = Vec::new();
    for line in lines {
        match groups.iter_mut().find(|g| g[0].group == line.group) {
            Some(group) => group.push(line),
            None => groups.push(vec![line]),
        }
    }
    groups
}

fn usage_of(line: &Line) -> String {
    if line.usage.is_empty() {
        format!("  {TOOL}")
    } else {
        format!("  {TOOL} {}", line.usage)
    }
}

fn width_of<'a>(lines: impl Iterator<Item = &'a Line<'a>>) -> usize {
    lines
        .map(|line| usage_of(line).chars().count())
        .max()
        .unwrap_or(0)
        .min(MAX_WIDTH)
}

fn pad(text: &str, width: usize) -> String {
    format!("{text:<width$}")
}

pub fn choice_rows(choices: &[ArgChoice], max: usize, group: &str) -> Vec<String> {
    if choices.is_empty() {
        return vec!["      (nothing to pick right now)".into()];
    }
    let shown = &choices[..choices.len().min(max)];
    let width = shown
        .iter()
        .map(|c| c.word.chars().count())
        .max()
        .unwrap_or(0);
    let mut rows: Vec<String> = shown
        .iter()
        .map(|choice| match &choice.title {
            Some(title) if title.to_lowercase() != choice.word.to_lowercase() => {
                format!("      {}  {title}", pad(&choice.word, width))
            }
            _ => format!("      {}", choice.word),
        })
        .collect();
    let more = choices.len() - shown.len();
    if more > 0 {
        rows.push(format!("      … {more} more: {TOOL} help {group}"));
    }
    rows
}

fn rows(lines: &[Line], width: usize, max_choices: usize) -> Vec<String> {
    lines
        .iter()
        .flat_map(|line| {
            let mut out = vec![format!("{}  {}", pad(&usage_of(line), width), line.help)];
            if let Some(choices) = line.choices {
                out.extend(choice_rows(choices, max_choices, &line.group));
            }
            out
        })
        .collect()
}

pub fn help_text(catalog: &[CliCommand]) -> String {
    let actions: Vec<Line> = action_lines().collect();
    let commands: Vec<Line> = catalog.iter().map(command_line).collect();
    let width = width_of(actions.iter().chain(commands.iter()));
    let mut out = vec![
        format!("Usage: {TOOL} [command] [words...]"),
        String::new(),
        "Actions".into(),
    ];
    out.extend(rows(&actions, width, MAX_CHOICES));
    out.push(String::new());
    out.push("Commands (what the menu, the palette and the shortcuts run)".into());
    for (index, group) in grouped(commands).iter().enumerate() {
        if index > 0 {
            out.push(String::new());
        }
        out.extend(rows(group, width, MAX_CHOICES));
    }
    out.push(String::new());
    out.push(format!(
        "{TOOL} help <group> shows one group with everything it can pick, e.g. {TOOL} help music."
    ));
    out.push("Words match without regard to case; names with spaces need no quotes.".into());
    out.join("\n")
}

/// One group (the first word), with all its choices; `None` when there's no such group.
pub fn group_help(catalog: &[CliCommand], group: &str) -> Option<String> {
    let lines: Vec<Line> = action_lines()
        .chain(catalog.iter().map(command_line))
        .filter(|line| line.group == group)
        .collect();
    if lines.is_empty() {
        return None;
    }
    Some(rows(&lines, width_of(lines.iter()), usize::MAX).join("\n"))
}

/// Every command id with what it does: a family once per choice, or with its argument.
pub fn command_entries(catalog: &[CliCommand]) -> Vec<(String, String)> {
    catalog
        .iter()
        .flat_map(|command| match (&command.arg, &command.choices) {
            (None, _) => vec![(command.id.clone(), command.help.clone())],
            (Some(arg), choices) if choices.as_ref().is_none_or(Vec::is_empty) => {
                vec![(format!("{}{arg}", command.id), command.help.clone())]
            }
            (Some(_), choices) => choices
                .iter()
                .flatten()
                .map(|choice| {
                    let name = choice.title.as_deref().unwrap_or(&choice.word);
                    (
                        format!("{}{}", command.id, choice.argument),
                        format!("{}: {name}", command.help),
                    )
                })
                .collect(),
        })
        .collect()
}

/// `command_entries` one per line, tab-separated, for scripts and launchers.
pub fn command_list(catalog: &[CliCommand]) -> String {
    command_entries(catalog)
        .iter()
        .map(|(id, help)| format!("{id}\t{help}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| w.to_string()).collect()
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

    fn titled(word: &str, title: &str) -> ArgChoice {
        ArgChoice {
            title: Some(title.into()),
            ..ArgChoice::new(word)
        }
    }

    fn row(width: usize, usage: &str, help: &str) -> String {
        let usage = format!("  daybook {usage}");
        format!("{:<width$}  {help}", usage.trim_end())
    }

    fn sample() -> Vec<CliCommand> {
        vec![
            command("status:break", "Change Status… with Break picked"),
            command("journal:open", "Journal…"),
            family(
                "journal:open:",
                "Journal… at one group",
                "<group>",
                Some(vec![ArgChoice::new("Morning Pages")]),
            ),
            family(
                "mode:",
                "Switch to a context",
                "<context>",
                Some(Vec::new()),
            ),
        ]
    }

    fn music_with(count: usize) -> Vec<CliCommand> {
        let choices = (0..count)
            .map(|n| ArgChoice::new(format!("c{n}")))
            .collect();
        vec![family(
            "music:play:",
            "Play an entry",
            "<entry>",
            Some(choices),
        )]
    }

    #[test]
    fn the_full_help_lists_the_actions_then_the_commands_by_group() {
        let expected = [
            "Usage: daybook [command] [words...]".to_string(),
            String::new(),
            "Actions".into(),
            row(44, "", "Start Daybook or bring its window to the front"),
            row(44, "show", "Show the Daybook window without taking focus"),
            row(44, "focus", "Bring the Daybook window to the front"),
            row(44, "hide", "Hide the Daybook window"),
            row(44, "quit", "Quit Daybook"),
            row(44, "status", "Print the version and whether the window shows"),
            row(
                44,
                "status working|break|off [note...]",
                "Set your status right away, without the overlay",
            ),
            row(
                44,
                "slack working|break|off [note...]",
                "Set Slack like that status; Daybook’s own status stays",
            ),
            row(
                44,
                "slack custom <emoji> [text...]",
                "Set any Slack status, e.g. daybook slack custom :tomato: Focus",
            ),
            row(
                44,
                "state",
                "Print the status, context, call, event and music as JSON",
            ),
            row(44, "command <id>", "Run a command by its id"),
            row(
                44,
                "commands",
                "List every command id (also: daybook command --list)",
            ),
            row(44, "help [group]", "This help, or one group with all it can pick"),
            String::new(),
            "Commands (what the menu, the palette and the shortcuts run)".into(),
            row(44, "command status:break", "Change Status… with Break picked"),
            String::new(),
            row(44, "journal open", "Journal…"),
            row(44, "journal open <group>", "Journal… at one group"),
            "      Morning Pages".into(),
            String::new(),
            row(44, "mode <context>", "Switch to a context"),
            "      (nothing to pick right now)".into(),
            String::new(),
            "daybook help <group> shows one group with everything it can pick, e.g. daybook help music.".into(),
            "Words match without regard to case; names with spaces need no quotes.".into(),
        ]
        .join("\n");
        assert_eq!(help_text(&sample()), expected);
    }

    #[test]
    fn a_command_the_own_words_would_catch_is_shown_with_the_command_word() {
        let catalog = vec![
            command("status:break", "a"),
            command("status:toggle", "b"),
            command("state:reset", "c"),
            command("focus:open", "d"),
            command("app:quit", "e"),
        ];
        let text = help_text(&catalog);
        for (usage, help) in [
            ("command status:break", "a"),
            ("status toggle", "b"),
            ("command state:reset", "c"),
            ("focus open", "d"),
            ("app quit", "e"),
        ] {
            assert!(text.lines().any(|l| l == row(44, usage, help)), "{usage}");
        }
    }

    #[test]
    fn the_full_help_shows_eight_choices_and_counts_the_rest() {
        let text = help_text(&music_with(10));
        assert!(text.contains("      c7\n      … 2 more: daybook help music\n"));
        assert!(!text.contains("      c8"));
    }

    #[test]
    fn a_group_shows_all_its_choices() {
        let text = group_help(&music_with(10), "music").unwrap();
        assert_eq!(
            text,
            std::iter::once(row(28, "music play <entry>", "Play an entry"))
                .chain((0..10).map(|n| format!("      c{n}")))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn a_group_collects_the_actions_and_commands_of_its_first_word() {
        let catalog = vec![command("slack:away", "Go away on Slack")];
        assert_eq!(
            group_help(&catalog, "slack").unwrap(),
            [
                row(
                    43,
                    "slack working|break|off [note...]",
                    "Set Slack like that status; Daybook’s own status stays"
                ),
                row(
                    43,
                    "slack custom <emoji> [text...]",
                    "Set any Slack status, e.g. daybook slack custom :tomato: Focus"
                ),
                row(43, "command slack:away", "Go away on Slack"),
            ]
            .join("\n")
        );
    }

    #[test]
    fn a_group_that_does_not_exist_has_no_help() {
        assert_eq!(group_help(&sample(), "nope"), None);
    }

    #[test]
    fn choices_line_up_their_titles() {
        let choices = [
            titled("focus", "Focus Mix"),
            titled("a", "A"),
            titled("deep work", "Deep"),
        ];
        assert_eq!(
            choice_rows(&choices, usize::MAX, "music"),
            [
                "      focus      Focus Mix",
                "      a",
                "      deep work  Deep",
            ]
        );
    }

    #[test]
    fn choices_beyond_the_limit_are_counted() {
        let choices = [
            titled("focus", "Focus Mix"),
            titled("a", "Alpha"),
            titled("b", "Beta"),
        ];
        assert_eq!(
            choice_rows(&choices, 2, "music"),
            [
                "      focus  Focus Mix",
                "      a      Alpha",
                "      … 1 more: daybook help music",
            ]
        );
    }

    #[test]
    fn no_choices_say_there_is_nothing_to_pick() {
        assert_eq!(
            choice_rows(&[], 8, "mode"),
            ["      (nothing to pick right now)"]
        );
    }

    #[test]
    fn the_command_list_has_one_tab_separated_line_per_id() {
        let catalog = vec![
            command("music:toggle", "Play or pause Music"),
            family(
                "music:play:",
                "Play an entry",
                "<entry>",
                Some(vec![titled("focus", "Focus Mix"), ArgChoice::new("calm")]),
            ),
            family("jira:issue:", "Open a Jira issue", "<key>", None),
            family(
                "mode:",
                "Switch to a context",
                "<context>",
                Some(Vec::new()),
            ),
        ];
        assert_eq!(
            command_entries(&catalog),
            [
                ("music:toggle", "Play or pause Music"),
                ("music:play:focus", "Play an entry: Focus Mix"),
                ("music:play:calm", "Play an entry: calm"),
                ("jira:issue:<key>", "Open a Jira issue"),
                ("mode:<context>", "Switch to a context"),
            ]
            .map(|(id, help)| (id.to_string(), help.to_string()))
        );
        assert_eq!(
            command_list(&catalog),
            "music:toggle\tPlay or pause Music\n\
             music:play:focus\tPlay an entry: Focus Mix\n\
             music:play:calm\tPlay an entry: calm\n\
             jira:issue:<key>\tOpen a Jira issue\n\
             mode:<context>\tSwitch to a context"
        );
    }

    #[test]
    fn an_empty_catalog_lists_nothing() {
        assert_eq!(command_list(&[]), "");
    }

    #[test]
    fn own_words_are_caught_before_commands() {
        for (words, expected) in [
            (vec!["show"], true),
            (vec!["show", "x"], true),
            (vec!["slack", "custom"], true),
            (vec!["state"], true),
            (vec!["command", "x"], true),
            (vec!["commands"], true),
            (vec!["help", "music"], true),
            (vec!["status"], true),
            (vec!["status", "break"], true),
            (vec!["status", "toggle"], false),
            (vec!["focus"], true),
            (vec!["focus", "open"], false),
            (vec!["hide"], true),
            (vec!["quit"], true),
            (vec!["quit", "now"], false),
            (vec!["app", "quit"], false),
            (vec!["music"], false),
            (vec![], false),
        ] {
            assert_eq!(is_own(&strings(&words)), expected, "{words:?}");
        }
    }

    #[test]
    fn the_words_of_an_id_skip_empty_parts() {
        for (id, expected) in [
            ("journal:open", vec!["journal", "open"]),
            ("journal:open:", vec!["journal", "open"]),
            ("music", vec!["music"]),
            ("", vec![]),
        ] {
            assert_eq!(words_of(id), strings(&expected), "{id:?}");
        }
    }
}
