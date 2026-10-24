//! Every event the backend emits to the page, with its payload type.

/// No payload; the page then calls `take_launch_events`.
pub const LAUNCH_EVENT: &str = "launch";
/// Payload: a command id.
pub const COMMAND_EVENT: &str = "command";
/// Payload: a `RemoteEvent`.
pub const REMOTE_EVENT: &str = "remote";
/// No payload; the hidden main window was shown again.
pub const WINDOW_OPENED_EVENT: &str = "window_opened";
/// Payload: the saved `Settings`.
pub const SETTINGS_CHANGED_EVENT: &str = "settings_changed";
