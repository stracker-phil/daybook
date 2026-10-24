//! Every event the backend emits to the page, with its payload type.

/// No payload; the page then calls `take_launch_events`.
pub const LAUNCH_EVENT: &str = "launch";
/// Payload: a `RemoteEvent`.
pub const REMOTE_EVENT: &str = "remote";
/// No payload; the hidden main window was shown again.
pub const WINDOW_OPENED_EVENT: &str = "window_opened";
/// Payload: a `NoteChanged`; a daily note or the changes waiting for today's note changed.
pub const NOTE_CHANGED_EVENT: &str = "note_changed";
/// To the overlay window. Payload: the `PanelSession` to render, `null` once it closed.
pub const PANEL_SESSION_EVENT: &str = "panel_session";
/// Payload: the saved `Settings`.
pub const SETTINGS_CHANGED_EVENT: &str = "settings_changed";
/// To the Daybook window. Payload: the Settings page to open, `null` for the first.
pub const OPEN_SETTINGS_EVENT: &str = "open_settings";
