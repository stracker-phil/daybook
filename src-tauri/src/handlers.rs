//! Every Tauri command the page can invoke.

use tauri::ipc::Invoke;

use crate::{autostart, bus, commands, contexts, palette, remote, shortcuts, vault, window};

pub fn handlers() -> impl Fn(Invoke) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        commands::get_settings,
        commands::update_settings,
        commands::get_keybindings,
        commands::set_keybindings,
        commands::get_ui_state,
        commands::set_ui_state,
        autostart::autostart_set,
        autostart::autostart_status,
        commands::open_log_folder,
        commands::open_url,
        commands::log_frontend_error,
        commands::take_launch_events,
        commands::choose_folder,
        commands::reveal_path,
        bus::app::run_command,
        contexts::context_state,
        palette::palette_query,
        palette::palette_run,
        shortcuts::shortcuts_pause,
        remote::remote_reply,
        vault::commands::daily_note_state,
        vault::commands::daily_note_open,
        vault::commands::daily_note_create,
        window::panel::commands::panel_open,
        window::panel::commands::panel_close,
        window::panel::commands::panel_session,
        window::panel::commands::panel_ready,
        window::panel::commands::panel_painted,
        window::panel::commands::panel_set_height,
        #[cfg(debug_assertions)]
        crate::dev::dev_edit_today_note,
    ]
}
