//! The `daybook` command line. Without words it starts the app (or activates the running one); `show`
//! and `focus` also start it. Every other call hands its words to the running app, which parses them
//! (`grammar`) against its commands and answers with what to print (`automation::cli`).

pub mod grammar;
pub mod help;
pub mod reply;

use std::ffi::OsString;

use clap::{CommandFactory, Parser};

use crate::ipc::IpcMessage;
use grammar::{Plan, parse_cli};

/// The `--log-level` of the CLI; without it the app logs at `Info`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, clap::ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    #[default]
    Info,
    Debug,
    Trace,
}

impl From<LogLevel> for log::LevelFilter {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Error => log::LevelFilter::Error,
            LogLevel::Warn => log::LevelFilter::Warn,
            LogLevel::Info => log::LevelFilter::Info,
            LogLevel::Debug => log::LevelFilter::Debug,
            LogLevel::Trace => log::LevelFilter::Trace,
        }
    }
}

/// Only the flags of a launch; the words go to the app, `--help` with them.
#[derive(Parser, Debug)]
#[command(
    name = "daybook",
    version,
    about = "A menu-bar companion for the day and the Obsidian vault",
    disable_help_flag = true,
    disable_help_subcommand = true
)]
struct Cli {
    /// Start without showing the window; a running instance stays as it is
    #[arg(long)]
    hidden: bool,
    /// Log level for this run
    #[arg(long, value_enum)]
    log_level: Option<LogLevel>,
    /// Print replies as JSON
    #[arg(long)]
    json: bool,
    #[arg(short = 'h', long)]
    help: bool,
    /// An action or command, e.g. `status break lunch` or `journal open`
    #[arg(trailing_var_arg = true)]
    words: Vec<String>,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum CliOutcome {
    Run {
        hidden: bool,
        log_level: Option<LogLevel>,
    },
    /// `--version`: print and exit without starting the app.
    Exit { message: String },
    /// `show` or `focus`: with no app running, start it with the message.
    Remote { message: IpcMessage, json: bool },
    /// A lone `hide`, `quit` or `status`: the messages from before the words went to the app, so a
    /// newer command still reaches an older running app (`scripts/install.sh`). Never starts the app.
    Ask { message: IpcMessage, json: bool },
    /// Words for the running app.
    Words { words: Vec<String>, json: bool },
}

#[derive(Debug, PartialEq)]
pub struct CliError {
    pub message: String,
    /// A plain launch with bad flags also reports them in the app, which may have been opened from
    /// Finder; an error with words is for the terminal only.
    pub in_app: bool,
}

/// Parses the full argv (including the program name).
pub fn parse<I, S>(args: I) -> Result<CliOutcome, CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<OsString> + Clone,
{
    let args: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let in_app = !names_command(&args);
    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(e) => {
            return match e.kind() {
                clap::error::ErrorKind::DisplayVersion => Ok(CliOutcome::Exit {
                    message: e.to_string(),
                }),
                _ => Err(CliError {
                    message: e.to_string(),
                    in_app,
                }),
            };
        }
    };
    // `--json` may also come after the words.
    let json = cli.json || cli.words.iter().any(|word| word == "--json");
    let mut words: Vec<String> = cli
        .words
        .into_iter()
        .filter(|word| word != "--json")
        .collect();
    if cli.help {
        words.push("--help".into());
    }
    let alone = |word: &str| words.len() == 1 && words[0].eq_ignore_ascii_case(word);
    let message = if alone("show") {
        IpcMessage::Show { focus: false }
    } else if alone("focus") {
        IpcMessage::Activate { hidden: false }
    } else if let Some(message) = [
        ("hide", IpcMessage::Hide),
        ("quit", IpcMessage::Quit),
        ("status", IpcMessage::Status),
    ]
    .into_iter()
    .find_map(|(word, message)| alone(word).then_some(message))
    {
        return Ok(CliOutcome::Ask { message, json });
    } else if words.is_empty() {
        return Ok(CliOutcome::Run {
            hidden: cli.hidden,
            log_level: cli.log_level,
        });
    } else {
        return Ok(CliOutcome::Words { words, json });
    };
    Ok(CliOutcome::Remote { message, json })
}

/// Whether an argument is a word rather than a flag or a flag's value.
fn names_command(args: &[OsString]) -> bool {
    let cli = Cli::command();
    let takes_value = |arg: &str| {
        cli.get_arguments().any(|a| {
            a.get_action().takes_values()
                && a.get_long().is_some_and(|long| arg == format!("--{long}"))
        })
    };
    let mut args = args.iter().skip(1).filter_map(|arg| arg.to_str());
    while let Some(arg) = args.next() {
        if takes_value(arg) {
            args.next();
        } else if !arg.starts_with('-') {
            return true;
        }
    }
    false
}

/// The help without the app: the own words only, and that commands need the running app. `None`
/// when the words aren't a call for help.
pub fn help_without_app(words: &[String]) -> Option<String> {
    let asks = words
        .first()
        .is_some_and(|w| w.eq_ignore_ascii_case("help"))
        || words.iter().any(|w| w == "--help" || w == "-h");
    if !asks {
        return None;
    }
    match parse_cli(words, &[]) {
        Plan::Print(text) => Some(format!(
            "{text}\n\nDaybook isn't running: start it to run and list its commands."
        )),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<CliOutcome, CliError> {
        parse(std::iter::once("daybook").chain(args.iter().copied()))
    }

    fn remote(args: &[&str]) -> (IpcMessage, bool) {
        match run(args).unwrap() {
            CliOutcome::Remote { message, json } => (message, json),
            other => panic!("not a remote outcome: {other:?}"),
        }
    }

    fn words(args: &[&str], json: bool) -> Result<CliOutcome, CliError> {
        Ok(CliOutcome::Words {
            words: args.iter().map(|w| w.to_string()).collect(),
            json,
        })
    }

    #[test]
    fn no_arguments_start_the_app() {
        assert_eq!(
            run(&[]),
            Ok(CliOutcome::Run {
                hidden: false,
                log_level: None
            })
        );
    }

    #[test]
    fn hidden_flag_starts_the_app_without_a_window() {
        assert_eq!(
            run(&["--hidden"]),
            Ok(CliOutcome::Run {
                hidden: true,
                log_level: None
            })
        );
    }

    #[test]
    fn log_level_flag_is_parsed_for_each_level() {
        for (arg, level) in [
            ("error", LogLevel::Error),
            ("warn", LogLevel::Warn),
            ("info", LogLevel::Info),
            ("debug", LogLevel::Debug),
            ("trace", LogLevel::Trace),
        ] {
            assert_eq!(
                run(&["--log-level", arg]),
                Ok(CliOutcome::Run {
                    hidden: false,
                    log_level: Some(level)
                }),
                "{arg}"
            );
        }
    }

    #[test]
    fn version_exits_without_starting_the_app() {
        let Ok(CliOutcome::Exit { message }) = run(&["--version"]) else {
            panic!("--version should exit");
        };
        assert_eq!(
            message.trim_end(),
            format!("daybook {}", env!("CARGO_PKG_VERSION"))
        );
    }

    #[test]
    fn help_goes_to_the_app_as_a_word() {
        for args in [&["--help"][..], &["-h"], &["--json", "-h"]] {
            let json = args.contains(&"--json");
            assert_eq!(run(args), words(&["--help"], json), "{args:?}");
        }
    }

    #[test]
    fn help_after_words_stays_with_the_words() {
        assert_eq!(
            run(&["music", "play", "-h"]),
            words(&["music", "play", "-h"], false)
        );
        assert_eq!(
            run(&["status", "--help"]),
            words(&["status", "--help"], false)
        );
    }

    #[test]
    fn words_after_the_flags_go_to_the_app_as_typed() {
        for (args, expected) in [
            (
                vec!["status", "break", "lunch"],
                vec!["status", "break", "lunch"],
            ),
            (vec!["journal", "open"], vec!["journal", "open"]),
            (vec!["Music", "PLAY"], vec!["Music", "PLAY"]),
            (vec!["help"], vec!["help"]),
            (vec!["--hidden", "state"], vec!["state"]),
            (vec!["--log-level", "debug", "state"], vec!["state"]),
        ] {
            assert_eq!(run(&args), words(&expected, false), "{args:?}");
        }
    }

    #[test]
    fn bad_flags_without_a_subcommand_are_reported_in_the_app() {
        for args in [&["--nope"][..], &["--log-level", "loud"]] {
            let err = run(args).unwrap_err();
            assert!(err.in_app, "{args:?}");
            assert!(!err.message.is_empty(), "{args:?}");
        }
    }

    #[test]
    fn a_flag_after_a_word_belongs_to_the_words() {
        assert_eq!(
            run(&["status", "--nope"]),
            words(&["status", "--nope"], false)
        );
    }

    #[test]
    fn unknown_flags_before_the_words_are_reported_in_the_terminal_only() {
        for args in [&["--nope", "status"][..], &["--json", "--nope", "state"]] {
            let err = run(args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
            assert!(!err.message.is_empty(), "{args:?}");
        }
    }

    #[test]
    fn a_mistyped_word_is_left_to_the_app() {
        for (args, expected, json) in [
            (vec!["stauts"], vec!["stauts"], false),
            (vec!["foo", "--json"], vec!["foo"], true),
            (vec!["--json", "stauts"], vec!["stauts"], true),
            (
                vec!["--log-level", "debug", "stauts"],
                vec!["stauts"],
                false,
            ),
        ] {
            assert_eq!(run(&args), words(&expected, json), "{args:?}");
        }
    }

    #[test]
    fn a_lone_show_or_focus_starts_the_app() {
        for (args, message) in [
            (vec!["show"], IpcMessage::Show { focus: false }),
            (vec!["SHOW"], IpcMessage::Show { focus: false }),
            (vec!["focus"], IpcMessage::Activate { hidden: false }),
        ] {
            assert_eq!(remote(&args), (message, false), "{args:?}");
        }
    }

    #[test]
    fn a_lone_hide_quit_or_status_asks_the_running_app_directly() {
        for (args, message) in [
            (vec!["hide"], IpcMessage::Hide),
            (vec!["HIDE"], IpcMessage::Hide),
            (vec!["quit"], IpcMessage::Quit),
            (vec!["Quit"], IpcMessage::Quit),
            (vec!["status"], IpcMessage::Status),
            (vec!["STATUS"], IpcMessage::Status),
        ] {
            assert_eq!(
                run(&args),
                Ok(CliOutcome::Ask {
                    message,
                    json: false
                }),
                "{args:?}"
            );
        }
        assert_eq!(
            run(&["status", "break"]),
            words(&["status", "break"], false)
        );
    }

    #[test]
    fn show_or_focus_with_more_words_go_to_the_app() {
        assert_eq!(run(&["show", "today"]), words(&["show", "today"], false));
        assert_eq!(run(&["focus", "open"]), words(&["focus", "open"], false));
    }

    #[test]
    fn json_flag_is_accepted_before_and_after_the_words() {
        for args in [&["status", "--json"][..], &["--json", "status"]] {
            assert_eq!(
                run(args),
                Ok(CliOutcome::Ask {
                    message: IpcMessage::Status,
                    json: true
                }),
                "{args:?}"
            );
        }
        assert_eq!(
            remote(&["show", "--json"]),
            (IpcMessage::Show { focus: false }, true)
        );
    }

    #[test]
    fn help_without_the_app_lists_the_own_words_and_says_the_app_is_off() {
        for args in [
            &["help"][..],
            &["--help"],
            &["help", "slack"],
            &["slack", "-h"],
        ] {
            let words: Vec<String> = args.iter().map(|w| w.to_string()).collect();
            let text = help_without_app(&words).unwrap_or_default();
            assert!(
                text.ends_with("\n\nDaybook isn't running: start it to run and list its commands."),
                "{args:?}: {text}"
            );
        }
    }

    #[test]
    fn help_without_the_app_is_none_for_other_words() {
        for args in [
            &["status"][..],
            &["music", "--help"],
            &["help", "nope"],
            &[],
        ] {
            let words: Vec<String> = args.iter().map(|w| w.to_string()).collect();
            assert_eq!(help_without_app(&words), None, "{args:?}");
        }
    }
}
