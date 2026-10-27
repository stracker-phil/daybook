//! A new process and the running Daybook: handing a launch over, taking the instance lock, and
//! asking the running app on behalf of a subcommand.

use std::fs::File;
use std::path::Path;
use std::time::Duration;

use crate::cli::reply::CliReply;
use crate::ipc::{self, Forward, IpcError, IpcMessage};
use crate::{cli, fail};

const LOCK_NAME: &str = "daybook.lock";
const LOCK_POLL: Duration = Duration::from_millis(200);
const LOCK_POLLS: u32 = 25;

/// Exits when a running instance takes the message; otherwise returns the instance lock, held for the
/// app's lifetime so two launches that both find no listener can't both start. `None` when the lock
/// cannot be taken at all.
pub fn forward_or_lock(
    config_dir: &Path,
    socket: &Path,
    message: &IpcMessage,
    exit_code: i32,
) -> Option<File> {
    forward(socket, message, exit_code);
    match lock_instance(config_dir) {
        Ok(Some(lock)) => Some(lock),
        Ok(None) => forward_to_starting_instance(socket, message, exit_code),
        Err(e) => {
            eprintln!("Cannot lock the Daybook instance: {e}");
            None
        }
    }
}

/// `None` when another process holds the lock.
fn lock_instance(dir: &Path) -> std::io::Result<Option<File>> {
    let file = File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(LOCK_NAME))?;
    match file.try_lock() {
        Ok(()) => Ok(Some(file)),
        Err(std::fs::TryLockError::WouldBlock) => Ok(None),
        Err(std::fs::TryLockError::Error(e)) => Err(e),
    }
}

/// Exits when a running instance answers or refuses the message; returns when none is listening.
fn forward(socket: &Path, message: &IpcMessage, exit_code: i32) {
    match ipc::try_forward(socket, message) {
        Ok(Forward::Delivered) => std::process::exit(exit_code),
        Ok(Forward::NoInstance) => {}
        Ok(Forward::Rejected) => fail("The running Daybook did not accept the request"),
        Err(e) => fail(&format!("Could not reach the running Daybook: {e}")),
    }
}

/// The lock holder may still be starting up and not listen yet.
fn forward_to_starting_instance(socket: &Path, message: &IpcMessage, exit_code: i32) -> ! {
    for _ in 0..LOCK_POLLS {
        std::thread::sleep(LOCK_POLL);
        forward(socket, message, exit_code);
    }
    fail("Another Daybook is starting but does not answer")
}

/// Sends a subcommand's message to the running app and prints its answer; returns the exit code, or
/// `None` when nothing listens.
pub fn ask(socket: &Path, message: &IpcMessage, json: bool) -> Option<i32> {
    let reply = match ipc::request(socket, message.clone()) {
        Ok(Some(reply)) => reply,
        Ok(None) => return None,
        Err(e) => fail(&format!("Could not reach the running Daybook: {e}")),
    };
    match reply.into_result() {
        Ok(result) => {
            print_result(message, &result, json);
            Some(0)
        }
        Err(error) => {
            print_error(&error, json);
            Some(1)
        }
    }
}

/// Hands the command's words to the running app and prints its answer; returns the exit code, or
/// `None` when nothing listens.
pub fn ask_words(socket: &Path, words: Vec<String>, json: bool) -> Option<i32> {
    let reply = match ipc::request(socket, IpcMessage::Cli { words, json }) {
        Ok(Some(reply)) => reply,
        Ok(None) => return None,
        Err(e) => fail(&format!("Could not reach the running Daybook: {e}")),
    };
    let reply: CliReply = match reply.into_result() {
        Ok(result) => serde_json::from_value(result).unwrap_or_else(|e| {
            fail(&format!(
                "The running Daybook sent an unexpected reply: {e}"
            ))
        }),
        Err(error) => {
            print_error(&error, json);
            return Some(1);
        }
    };
    if !reply.stdout.is_empty() {
        println!("{}", reply.stdout);
    }
    if !reply.stderr.is_empty() {
        eprintln!("{}", reply.stderr);
    }
    Some(reply.code)
}

fn print_error(error: &IpcError, json: bool) {
    if json {
        println!("{}", serde_json::to_value(error).unwrap_or_default());
    } else {
        eprintln!("{}", error.message);
    }
}

fn print_result(message: &IpcMessage, result: &serde_json::Value, json: bool) {
    if json {
        if !result.is_null() {
            println!("{result}");
        }
        return;
    }
    match cli::reply::render(message, result) {
        Ok(Some(text)) => println!("{text}"),
        Ok(None) => {}
        Err(e) => fail(&e),
    }
}
