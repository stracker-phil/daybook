//! The `daybook` command line. Without a subcommand it starts the app (or activates the running one);
//! every subcommand becomes one `IpcMessage` that the running app answers (`remote::handle`).
//! A new subcommand adds a `Command` variant, its arm in `message`, a parser test, its arm in
//! `remote::handle` and, when it prints something, its arm in `reply::render`.

pub mod reply;

use std::ffi::OsString;

use clap::{CommandFactory, Parser, Subcommand};

use crate::ipc::IpcMessage;

const EXAMPLES: &str = "Examples:
  daybook                 start Daybook or bring it to the front
  daybook --hidden        start without opening the window (launch at login)
  daybook show            show the window without taking focus
  daybook status --json   the app's state as JSON";

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

#[derive(Parser, Debug)]
#[command(
    name = "daybook",
    version,
    about = "A menu-bar companion for the day and the Obsidian vault",
    after_help = EXAMPLES
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
    /// Start without showing the window; a running instance stays as it is
    #[arg(long)]
    hidden: bool,
    /// Log level for this run
    #[arg(long, value_enum)]
    log_level: Option<LogLevel>,
    /// Print replies as JSON
    #[arg(long, global = true)]
    json: bool,
}

#[derive(Subcommand, Debug)]
enum Command {
    /// Show the window, starting Daybook if needed
    Show,
    /// Show the window and bring it to the front, starting Daybook if needed
    Focus,
    /// Hide the window; Daybook keeps running in the menu bar
    Hide,
    /// Quit Daybook
    Quit,
    /// Print the version and whether the window shows
    Status,
}

#[derive(Debug)]
#[cfg_attr(test, derive(PartialEq))]
pub enum CliOutcome {
    Run {
        hidden: bool,
        log_level: Option<LogLevel>,
    },
    /// `--help` / `--version`: print and exit without starting the app.
    Exit { message: String },
    /// A subcommand for the running app. `starts_app`: with no app running, start it with the message.
    Remote {
        message: IpcMessage,
        json: bool,
        starts_app: bool,
    },
}

#[derive(Debug, PartialEq)]
pub struct CliError {
    pub message: String,
    /// A plain launch with bad flags also reports them in the app, which may have been opened from
    /// Finder; an error with a subcommand, known or mistyped, is for the terminal only.
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
                clap::error::ErrorKind::DisplayHelp | clap::error::ErrorKind::DisplayVersion => {
                    Ok(CliOutcome::Exit {
                        message: e.to_string(),
                    })
                }
                _ => Err(CliError {
                    message: e.to_string(),
                    in_app,
                }),
            };
        }
    };
    let Some(command) = cli.command else {
        return Ok(CliOutcome::Run {
            hidden: cli.hidden,
            log_level: cli.log_level,
        });
    };
    let starts_app = matches!(command, Command::Show | Command::Focus);
    Ok(CliOutcome::Remote {
        message: message(command),
        json: cli.json,
        starts_app,
    })
}

/// Whether an argument is a word rather than a flag or a flag's value: a subcommand, known or mistyped.
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

fn message(command: Command) -> IpcMessage {
    match command {
        Command::Show => IpcMessage::Show { focus: false },
        Command::Focus => IpcMessage::Activate { hidden: false },
        Command::Hide => IpcMessage::Hide,
        Command::Quit => IpcMessage::Quit,
        Command::Status => IpcMessage::Status,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(args: &[&str]) -> Result<CliOutcome, CliError> {
        parse(std::iter::once("daybook").chain(args.iter().copied()))
    }

    fn remote(args: &[&str]) -> (IpcMessage, bool, bool) {
        match run(args).unwrap() {
            CliOutcome::Remote {
                message,
                json,
                starts_app,
            } => (message, json, starts_app),
            other => panic!("not a remote outcome: {other:?}"),
        }
    }

    /// The names in the `Commands:` section of the help.
    fn listed_commands() -> Vec<String> {
        let help = Cli::command().render_help().to_string();
        help.lines()
            .skip_while(|line| *line != "Commands:")
            .skip(1)
            .take_while(|line| line.is_empty() || line.starts_with(' '))
            .filter(|line| line.starts_with("  ") && !line.starts_with("   "))
            .filter_map(|line| line.split_whitespace().next())
            .map(String::from)
            .collect()
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
    fn help_and_version_exit_without_starting_the_app() {
        let Ok(CliOutcome::Exit { message }) = run(&["--version"]) else {
            panic!("--version should exit");
        };
        assert_eq!(
            message.trim_end(),
            format!("daybook {}", env!("CARGO_PKG_VERSION"))
        );
        assert!(matches!(run(&["--help"]), Ok(CliOutcome::Exit { .. })));
    }

    #[test]
    fn help_lists_exactly_the_subcommands() {
        assert_eq!(
            listed_commands(),
            ["show", "focus", "hide", "quit", "status", "help"]
        );
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
    fn bad_flags_of_a_subcommand_are_reported_in_the_terminal_only() {
        for args in [&["status", "--nope"][..], &["show", "today"]] {
            let err = run(args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
        }
    }

    #[test]
    fn a_mistyped_subcommand_is_reported_in_the_terminal_only() {
        for args in [
            &["stauts"][..],
            &["foo", "--json"],
            &["--json", "stauts"],
            &["--log-level", "debug", "stauts"],
        ] {
            let err = run(args).unwrap_err();
            assert!(!err.in_app, "{args:?}");
            assert!(!err.message.is_empty(), "{args:?}");
        }
    }

    #[test]
    fn subcommands_become_their_messages() {
        for (args, message, starts_app) in [
            (vec!["show"], IpcMessage::Show { focus: false }, true),
            (vec!["focus"], IpcMessage::Activate { hidden: false }, true),
            (vec!["hide"], IpcMessage::Hide, false),
            (vec!["quit"], IpcMessage::Quit, false),
            (vec!["status"], IpcMessage::Status, false),
        ] {
            assert_eq!(
                run(&args),
                Ok(CliOutcome::Remote {
                    message,
                    json: false,
                    starts_app
                }),
                "{args:?}"
            );
        }
    }

    #[test]
    fn json_flag_is_accepted_before_and_after_the_subcommand() {
        for args in [&["status", "--json"][..], &["--json", "status"]] {
            assert_eq!(remote(args), (IpcMessage::Status, true, false), "{args:?}");
        }
    }

    #[test]
    fn example_descriptions_start_in_the_same_column() {
        let help = Cli::command().render_long_help().to_string();
        let columns: Vec<(usize, &str)> = help
            .lines()
            .skip_while(|line| *line != "Examples:")
            .skip(1)
            .filter_map(|line| {
                let command = line.trim_start();
                let indent = line.len() - command.len();
                let gap = command.find("  ")?;
                let spaces = command[gap..].len() - command[gap..].trim_start().len();
                Some((indent + gap + spaces, line))
            })
            .collect();
        assert_eq!(columns.len(), 4, "{help}");
        let (first, _) = columns[0];
        for (column, line) in &columns {
            assert_eq!(*column, first, "{line}");
        }
    }
}
