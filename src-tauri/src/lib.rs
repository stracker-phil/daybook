mod app;
pub mod autostart;
pub mod cli;
pub mod commands;
pub mod deep_link;
#[cfg(debug_assertions)]
mod dev;
pub mod error;
pub mod events;
mod handlers;
pub mod help;
mod instance;
pub mod ipc;
pub mod launch;
pub mod lock;
pub mod menu;
pub mod pending;
pub mod remote;
pub mod state;
pub mod store;
#[cfg(any(test, feature = "test-util"))]
pub mod test_support;
pub mod tray;
pub mod window;

use std::path::PathBuf;

use crate::cli::{CliError, CliOutcome};
use crate::ipc::IpcMessage;
use crate::state::AppState;
use crate::store::json_file::create_private_dir;

/// The config folder's name; also the release bundle's identifier. The debug bundle has its own identifier
/// (`tauri.debug.conf.json`), so macOS grants it apart, but shares this folder.
const IDENTIFIER: &str = "com.philippstracker.daybook";
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

    app::start(
        AppState::open(&config_dir),
        socket,
        message,
        log_level.unwrap_or_default(),
        hidden,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifier_matches_the_tauri_config() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert_eq!(conf["identifier"], serde_json::json!(IDENTIFIER));
    }
}
