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
/// No payload; a call started, changed or ended (docs/presence-and-calls.md).
pub const CALL_CHANGED_EVENT: &str = "call_changed";
/// No payload; what Slack shows or the Slack connection changed (docs/slack.md).
pub const SLACK_CHANGED_EVENT: &str = "slack_changed";
/// Payload: a `MusicState`; what Music plays or which entry plays changed (docs/music.md).
pub const MUSIC_CHANGED_EVENT: &str = "music_changed";
/// Payload: the saved `Settings`.
pub const SETTINGS_CHANGED_EVENT: &str = "settings_changed";
/// Payload: the saved `Keybindings`.
pub const KEYBINDINGS_CHANGED_EVENT: &str = "keybindings_changed";
/// To the Settings window. Payload: the id of the page to show.
pub const OPEN_SETTINGS_EVENT: &str = "open_settings";
/// To the Daybook window. No payload; shows or hides the inspector column.
pub const TOGGLE_INSPECTOR_EVENT: &str = "toggle_inspector";
/// To the quick-notes editor. No payload; another topic was chosen, so the page saves and reloads.
pub const QUICK_NOTE_TARGET_EVENT: &str = "quick_note_target";
/// To the rail window. Payload: the `RailModel` to draw.
pub const RAIL_MODEL_EVENT: &str = "rail_model";
/// To the rail window. Payload: a `RailShown`; the rail slid in or out, changed sides or became sticky.
pub const RAIL_SHOWN_EVENT: &str = "rail_shown";
/// To the rail window. Payload: a `RailPoint` in window coordinates while the pointer is on the rail,
/// `null` once it left.
pub const RAIL_POINTER_EVENT: &str = "rail_pointer";
/// To the tooltip window. Payload: the `RailTipContent` to draw.
pub const RAIL_TIP_CONTENT_EVENT: &str = "rail_tip_content";
/// To the tooltip window. Payload: the sequence of the card now on screen; it may fade in.
pub const RAIL_TIP_PLACED_EVENT: &str = "rail_tip_placed";
/// To the tooltip window. Payload: a sequence; the card fades out.
pub const RAIL_TIP_HIDDEN_EVENT: &str = "rail_tip_hidden";
/// To the tooltip window. Payload: a `RailPoint` in window coordinates while the pointer is on the
/// card's choices, `null` once it left.
pub const RAIL_TIP_POINTER_EVENT: &str = "rail_tip_pointer";
