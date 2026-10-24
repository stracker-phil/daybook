//! The app's own commands; features add theirs with their own `register`.

use serde_json::Value;
use tauri::{AppHandle, Manager};

use super::{Command, CommandBus};
use crate::error::AppResult;
use crate::state::AppState;
use crate::{commands, help, vault, window};

type Bus = CommandBus<AppHandle>;

async fn open_panel(app: AppHandle, view: &str) -> AppResult<()> {
    window::panel::open(&app, view, Value::Null).await;
    Ok(())
}

fn creating_today(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    state.notes.state(&state.settings.get()).creating
}

pub fn register(bus: &mut Bus) {
    bus.add(
        Command::new("app:settings", "Settings…")
            .help("Open Settings")
            .run_now(|app: &AppHandle| window::open_settings(app, None)),
    );
    bus.add(
        Command::new("app:quit", "Quit Daybook")
            .quits()
            .run_now(|app: &AppHandle| app.exit(0)),
    );
    bus.add(
        Command::new("app:logs", "Open Log Folder")
            .run(|app| async move { commands::open_log_folder(app) }),
    );
    bus.add(
        Command::new("app:readme", "README")
            .help("Open the README")
            .run(|app| async move { help::open_readme(app) }),
    );
    bus.add(
        Command::new("window:close", "Close Window")
            .help("Close the focused window")
            .hidden()
            .run_now(window::close_focused),
    );
    bus.add(
        Command::new("hub:open", "Open Daybook")
            .help("Open the Daybook window")
            .run_now(window::show_main),
    );
    bus.add(
        Command::new("hub:toggle", "Show or Hide Daybook")
            .help("Show or hide the Daybook window")
            .run_now(window::toggle_main),
    );
    bus.add(
        Command::new("palette:open", "Command Palette…")
            .help("Open the command palette; again closes it")
            .run(|app| open_panel(app, "palette")),
    );
    bus.add(
        Command::new("daily:open", "Open Today's Note")
            .help("Open today's note in Obsidian")
            .needs_vault()
            .run(|app| async move { vault::commands::open_day(&app, None).await }),
    );
    bus.add(
        Command::new("daily:create", "Create Today's Note")
            .help("Create today's note")
            .needs_vault()
            .enabled_when(|app| !creating_today(app))
            .run(|app| async move { vault::commands::create_today(&app).await.map(|_| ()) }),
    );
    #[cfg(debug_assertions)]
    for (id, title, view) in [
        ("sample:open", "Sample Overlay", "sample"),
        ("sample:sticky", "Sample Overlay (Sticky)", "sample-sticky"),
    ] {
        bus.add(Command::new(id, title).run(move |app| open_panel(app, view)));
    }
}
