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
/// Payload: a `ContextChanged`; another context became current.
pub const CONTEXT_CHANGED_EVENT: &str = "context_changed";
/// Payload: a `StatusChanged`; today's status changed. Rust listens too (docs/status.md).
pub const STATUS_CHANGED_EVENT: &str = "status_changed";
/// No payload; an event, a calendar or the calendar access changed.
pub const CALENDAR_CHANGED_EVENT: &str = "calendar_changed";
/// Payload: the saved `Settings`.
pub const SETTINGS_CHANGED_EVENT: &str = "settings_changed";
/// Payload: the saved `Keybindings`.
pub const KEYBINDINGS_CHANGED_EVENT: &str = "keybindings_changed";
/// To the Settings window. Payload: the id of the page to show.
pub const OPEN_SETTINGS_EVENT: &str = "open_settings";
/// To the Daybook window. No payload; shows or hides the inspector column.
pub const TOGGLE_INSPECTOR_EVENT: &str = "toggle_inspector";
