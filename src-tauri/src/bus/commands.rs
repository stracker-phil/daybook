//! The app's own commands; features add theirs with their own `register`.

use serde_json::Value;
use tauri::{AppHandle, Manager};

use super::{ArgChoice, Command, CommandBus};
use crate::error::AppResult;
use crate::state::AppState;
use crate::{commands, help, vault, window};

type Bus = CommandBus<AppHandle>;

async fn open_panel(app: AppHandle, view: &str) -> AppResult<()> {
    window::panel::open(&app, view, Value::Null).await;
    Ok(())
}

fn settings_pages(_: &AppHandle) -> Vec<ArgChoice> {
    window::settings::PAGES
        .iter()
        .map(|(id, title)| ArgChoice {
            title: Some(title.to_string()),
            ..ArgChoice::new(*id)
        })
        .collect()
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
    bus.add_family(
        Command::new("settings:", "Open a Settings page")
            .family("<page>")
            .choices(settings_pages)
            .run(|app, page| async move { window::settings::open_page(&app, &page) }),
    );
    // The prototype's spelling, kept for Stage scripts.
    bus.add_family(
        Command::new("settings:open:", "Open a Settings page")
            .hidden()
            .family("<page>")
            .choices(settings_pages)
            .run(|app, page| async move { window::settings::open_page(&app, &page) }),
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
        Command::new("view:toggleInspector", "Show or Hide the Inspector")
            .help("Show or hide the Daybook window's right column")
            .run_now(window::toggle_inspector),
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
            .run(|app| async move { vault::commands::open_day(&app, None, None).await }),
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

/// Conventional ids (the Stage protocol's) for commands that exist under Daybook's own names; hidden.
const ALIASES: [(&str, &str); 4] = [
    ("app:showWindow", "hub:open"),
    ("app:toggleWindow", "hub:toggle"),
    ("view:palette", "palette:open"),
    ("view:search", "search:open"),
];

/// Runs after every feature registered, so each alias finds its target.
pub fn register_aliases(bus: &mut Bus) {
    for (alias, target) in ALIASES {
        let Some(info) = bus.info(target) else {
            continue;
        };
        let (title, help) = (info.title, info.help);
        bus.add(Command::new(alias, title).help(help).hidden().run(
            move |app: AppHandle| async move {
                let commands = app.state::<super::Commands>();
                commands.run_or_err(app.clone(), target).await
            },
        ));
    }
}
