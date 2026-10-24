//! The command palette overlay: what it lists, how typing ranks it, and running a pick.

mod entries;
mod rank;
mod search;

pub use entries::{PaletteEntry, PaletteKind, palette_entries};
pub use search::{PaletteGroup, PalettePrefix, PaletteResults, search};

use tauri::{AppHandle, Manager};

use crate::bus;
use crate::error::{AppError, AppResult};
use crate::window::panel;
use crate::{shortcuts, tools};

/// Picks that bring a window or another app forward, which must keep focus once the palette closes.
fn comes_forward(command: &str) -> bool {
    const PREFIXES: [&str; 6] = [
        "hub:",
        "settings:",
        "daily:",
        "notes:topic:",
        "jira:issue:",
        "link:",
    ];
    const EXACT: [&str; 2] = ["app:settings", "notes:quick"];
    PREFIXES.iter().any(|prefix| command.starts_with(prefix)) || EXACT.contains(&command)
}

/// The palette's results for what was typed, from the app's current state.
pub fn query(app: &AppHandle, typed: &str) -> PaletteResults {
    let commands = app.state::<bus::Commands>();
    let entries = palette_entries(
        &tools::snapshot(app),
        &shortcuts::shown(app),
        &commands.available(app),
    );
    search(&entries, typed)
}

#[tauri::command]
pub fn palette_query(app: AppHandle, query: String) -> PaletteResults {
    self::query(&app, &query)
}

/// A pick that opens another overlay replaces the palette; for anything else the palette closes and
/// hands focus back to where you were.
#[tauri::command]
pub async fn palette_run(app: AppHandle, command: String) -> AppResult<()> {
    let commands = app.state::<bus::Commands>();
    if !commands.knows(&command) {
        return Err(AppError::NotFound(format!("Unknown command “{command}”.")));
    }
    let forward = comes_forward(&command);
    let session = panel::session_id(&app);
    if forward {
        panel::close(&app, true);
    }
    commands.run(app.clone(), &command).await;
    if !forward && session.is_some() && panel::session_id(&app) == session {
        panel::close(&app, false);
    }
    Ok(())
}
