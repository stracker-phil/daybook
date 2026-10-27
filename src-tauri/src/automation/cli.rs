//! The command's words in the running app: parsed against the commands as they are now, then run as
//! an action or a command.

use std::path::PathBuf;

use serde::Serialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use super::{Action, Params};
use crate::bus::{self, Listed};
use crate::cli::grammar::{AppWord, CliCommand, Plan, parse_cli};
use crate::cli::help::command_entries;
use crate::cli::reply::{CliReply, action_reply, report_text};
use crate::{remote, window};

/// The visible commands, families with their choices right now.
pub fn catalog(app: &AppHandle) -> Vec<CliCommand> {
    let commands = app.state::<bus::Commands>();
    commands
        .list()
        .filter(|listed| !listed.info().hidden)
        .map(|listed| match listed {
            Listed::Command { id, info } => CliCommand {
                id: id.to_string(),
                help: info.help.to_string(),
                ..CliCommand::default()
            },
            Listed::Family(family) => CliCommand {
                id: listed.id().to_string(),
                help: listed.info().help.to_string(),
                arg: Some(family.arg.to_string()),
                choices: family.choices(app),
                from_words: family.from_words,
            },
        })
        .collect()
}

/// `--json` turns the command list into `[{ id, help }]`.
fn entries_json(catalog: &[CliCommand]) -> Value {
    Value::Array(
        command_entries(catalog)
            .into_iter()
            .map(|(id, help)| json!({ "id": id, "help": help }))
            .collect(),
    )
}

async fn run_action(app: &AppHandle, action: Action, params: &Params, json: bool) -> CliReply {
    match super::run(app, action.name(), params).await {
        Ok(outcome) => action_reply(action, &outcome, json),
        Err(error) => CliReply::failed(&error, json),
    }
}

fn report_reply(app: &AppHandle, json: bool) -> CliReply {
    let report = remote::status_report(app);
    if json {
        CliReply::json(&json!(report))
    } else {
        CliReply::print(report_text(&report))
    }
}

fn app_word(app: &AppHandle, word: AppWord, json: bool) -> CliReply {
    match word {
        AppWord::Focus => {
            window::show_main(app);
            CliReply::default()
        }
        AppWord::Hide => {
            window::hide_main(app);
            CliReply::default()
        }
        AppWord::Quit => {
            remote::quit_after_reply(app.clone());
            CliReply::default()
        }
        AppWord::Report => report_reply(app, json),
    }
}

/// Answers the words of one `daybook` call.
pub async fn answer(app: &AppHandle, words: &[String], json: bool) -> CliReply {
    let catalog = catalog(app);
    match parse_cli(words, &catalog) {
        Plan::Print(_) if json => CliReply::json(&entries_json(&catalog)),
        Plan::Print(text) => CliReply::print(text),
        Plan::Usage(message) => CliReply::usage(&message, json),
        Plan::Action { action, params } => run_action(app, action, &params, json).await,
        Plan::Command(id) => match super::command(app, &id).await {
            Ok(()) if json => CliReply::json(&json!({})),
            Ok(()) => CliReply::default(),
            Err(error) => CliReply::failed(&error, json),
        },
        Plan::App(word) => app_word(app, word, json),
    }
}

/// The installed `daybook` command (`scripts/install.sh`).
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct CliInfo {
    pub path: String,
    /// It runs this app.
    pub linked: bool,
}

pub fn cli_info() -> CliInfo {
    let path = std::env::home_dir()
        .unwrap_or_default()
        .join(".local/bin/daybook");
    let resolved = |path: PathBuf| std::fs::canonicalize(path).ok();
    let linked = resolved(path.clone()).is_some_and(|target| {
        std::env::current_exe()
            .ok()
            .and_then(resolved)
            .is_some_and(|exe| exe == target)
    });
    CliInfo {
        path: path.display().to_string(),
        linked,
    }
}
