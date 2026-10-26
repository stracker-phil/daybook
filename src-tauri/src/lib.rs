mod app;
pub mod autostart;
pub mod bus;
pub mod calendar;
pub mod calls;
pub mod cli;
pub mod clock;
pub mod commands;
pub mod contexts;
pub mod deep_link;
#[cfg(debug_assertions)]
mod dev;
pub mod error;
pub mod events;
pub mod focus;
mod handlers;
pub mod help;
pub mod hub;
mod instance;
pub mod ipc;
pub mod launch;
pub mod lock;
pub mod menu;
pub mod net;
pub mod notify;
pub mod palette;
pub mod pending;
pub mod presence;
pub mod process;
pub mod projects;
pub mod reminders;
pub mod remote;
pub mod secrets;
pub mod shortcuts;
pub mod slack;
pub mod state;
pub mod status;
pub mod status_rules;
pub mod store;
#[cfg(any(test, feature = "test-util"))]
pub mod test_support;
pub mod tools;
pub mod tray;
pub mod vault;
pub mod window;

use std::path::PathBuf;

use crate::cli::{CliError, CliOutcome};
use crate::ipc::IpcMessage;
use crate::state::AppState;
use crate::store::import;
use crate::store::json_file::create_private_dir;

/// The bundle identifier and config folder name; the debug build's own keeps its data apart (ADR 0014).
const IDENTIFIER: &str = if cfg!(debug_assertions) {
    "com.philippstracker.daybook.debug"
} else {
    "com.philippstracker.daybook"
};
/// Debug builds go by their own name in the menu bar and the tray.
pub const APP_NAME: &str = if cfg!(debug_assertions) {
    "Daybook (Debug)"
} else {
    "Daybook"
};
const SOCKET_NAME: &str = "daybook.sock";

/// Same directory Tauri resolves as `app_config_dir`, but available before the app starts.
fn config_dir() -> PathBuf {
    let Some(home) = std::env::home_dir() else {
        fail("Cannot locate the home directory for Daybook's settings");
    };
    home.join("Library/Application Support").join(IDENTIFIER)
}

fn fail(message: &str) -> ! {
    eprintln!("{message}");
    std::process::exit(2);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config_dir = config_dir();
    if let Err(e) = create_private_dir(&config_dir) {
        eprintln!("Cannot create {}: {e}", config_dir.display());
    }
    let socket = config_dir.join(SOCKET_NAME);

    let outcome = cli::parse(std::env::args_os());
    let (message, log_level, hidden) = match outcome {
        Ok(CliOutcome::Exit { message }) => {
            print!("{message}");
            std::process::exit(0);
        }
        Ok(CliOutcome::Run { hidden, log_level }) => {
            (IpcMessage::Activate { hidden }, log_level, hidden)
        }
        Ok(CliOutcome::Remote {
            message,
            json,
            starts_app,
        }) => {
            if let Some(code) = instance::ask(&socket, &message, json) {
                std::process::exit(code);
            }
            if !starts_app {
                fail("Daybook is not running");
            }
            (message, None, false)
        }
        Err(CliError {
            message,
            in_app: false,
        }) => fail(&message),
        Err(CliError { message, .. }) => {
            eprintln!("{message}");
            (IpcMessage::Error { message }, None, false)
        }
    };
    let exit_code = if matches!(message, IpcMessage::Error { .. }) {
        2
    } else {
        0
    };

    let _instance_lock = instance::forward_or_lock(&config_dir, &socket, &message, exit_code);
    let prototype_import =
        import::prototype_dir().and_then(|prototype| import::run(&prototype, &config_dir));

    app::start(
        AppState::open(&config_dir),
        socket,
        message,
        log_level.unwrap_or_default(),
        hidden,
        prototype_import,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_matches_the_tauri_config() {
        let release: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let debug: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.debug.conf.json")).unwrap();
        let expected = if cfg!(debug_assertions) {
            &debug
        } else {
            &release
        };
        assert_eq!(expected["identifier"], serde_json::json!(IDENTIFIER));
    }

    #[test]
    fn the_debug_identifier_differs_from_the_release_one() {
        let release: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let debug: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.debug.conf.json")).unwrap();
        assert_ne!(debug["identifier"], release["identifier"]);
        assert_eq!(
            debug["identifier"],
            serde_json::json!(format!("{}.debug", release["identifier"].as_str().unwrap()))
        );
    }
}
