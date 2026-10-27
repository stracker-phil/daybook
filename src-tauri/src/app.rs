//! The Tauri app: managed state, plugins, window and menu events, setup and the run loop.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind, TimezoneStrategy};

use crate::cli::LogLevel;
use crate::handlers::handlers;
use crate::ipc::IpcMessage;
use crate::state::AppState;
use crate::store::import::Import;
use crate::{
    action_log, activity, bus, calendar, calls, commands, contexts, deep_link, focus, http_api,
    ipc, jira, journal, launcher, links, menu, music, notify, presence, quick_notes, rail, remote,
    routines, scheduler, search, shortcuts, slack, stage, status, status_rules, timetracker, tray,
    vault, window, wizard,
};

const MAX_LOG_BYTES: u128 = 5 * 1024 * 1024;
const EXIT_RESTORE: Duration = Duration::from_secs(3);

pub fn start(
    state: AppState,
    socket: PathBuf,
    initial: IpcMessage,
    log_level: LogLevel,
    hidden: bool,
    import: Option<Import>,
) {
    tauri::Builder::default()
        .manage(state)
        .manage(remote::Bridge::default())
        .manage(window::panel::Panel::default())
        .manage(calls::Calls::default())
        .manage(wizard::Wizard::default())
        .manage(presence::Presence::default())
        .manage(slack::Slack::default())
        .manage(music::Music::default())
        .manage(timetracker::TimeTrackerService::default())
        .manage(activity::Activity::default())
        .manage(rail::RailState::default())
        .manage(commands_bus())
        .manage(shortcuts::Pause::default())
        .manage(tray::Shown::default())
        .manage(deep_link::PendingLinks::default())
        .manage(http_api::HttpApi::default())
        .plugin(log_plugin())
        .plugin(init_logging(log_level))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_deep_link::init())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--hidden"]),
        ))
        .on_menu_event(|app, event| menu::clicked(app, event.id().as_ref()))
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event
                && window::close_requested(window.app_handle(), window.label())
            {
                api.prevent_close();
            }
        })
        .setup(move |app| {
            log::info!("Daybook {} started", env!("CARGO_PKG_VERSION"));
            let state = app.state::<AppState>();
            app.set_menu(menu::build(app.handle())?)?;
            state.launches.push(initial);
            report_import(&state, import);
            action_log::install(app.handle());
            commands::apply_settings(app.handle(), None, &state.settings.get());
            vault::commands::install(app.handle());
            search::start(app.handle());
            tray::install(app.handle())?;
            shortcuts::apply(app.handle());
            calendar::install(app.handle());
            presence::install(app.handle());
            scheduler::start(app.handle());
            wizard::install(app.handle());
            status::start(app.handle());
            slack::install(app.handle());
            music::install(app.handle());
            timetracker::install(app.handle());
            activity::install(app.handle());
            // After every follower subscribed, so none misses the first check's call.
            calls::start(app.handle());
            status_rules::start(app.handle());
            notify::install(app.handle());
            window::panel::build(app.handle())?;
            window::presentation::install(app.handle());
            rail::install(app.handle());
            #[cfg(debug_assertions)]
            apply_dev_frame(app);
            start_ipc(app.handle().clone(), &socket);
            deep_link::listen(app.handle());
            stage::install(app.handle());
            http_api::install(app.handle());
            // The window starts invisible (tauri.conf.json), so a hidden start never flashes it.
            if hidden {
                window::hide_main(app.handle());
            } else {
                window::show_main(app.handle());
            }
            Ok(())
        })
        .invoke_handler(handlers())
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| match event {
            RunEvent::ExitRequested {
                code: None, api, ..
            } => api.prevent_exit(),
            RunEvent::Reopen { .. } => window::show_main(app),
            RunEvent::Exit => {
                stage::unpublish(app);
                timetracker::stop(app);
                restore_after_call(app);
                action_log::flush();
            }
            _ => {}
        });
}

/// Quitting during a call puts back the Slack status and the music volume the call replaced, within
/// `EXIT_RESTORE`; what doesn't make it is restored at the next start.
fn restore_after_call(app: &AppHandle) {
    let finished = tauri::async_runtime::block_on(tokio::time::timeout(EXIT_RESTORE, async {
        tokio::join!(slack::restore_on_exit(app), music::restore_on_exit(app));
    }));
    if finished.is_err() {
        log::warn!("Restoring Slack and the music after the call took too long");
    }
}

fn commands_bus() -> bus::Commands {
    let mut commands = bus::CommandBus::default();
    bus::register(&mut commands);
    contexts::register(&mut commands);
    status::register(&mut commands);
    calls::register(&mut commands);
    focus::register(&mut commands);
    journal::register(&mut commands);
    quick_notes::register(&mut commands);
    routines::register(&mut commands);
    search::register(&mut commands);
    links::register(&mut commands);
    jira::register(&mut commands);
    launcher::register(&mut commands);
    music::register(&mut commands);
    timetracker::register(&mut commands);
    activity::register(&mut commands);
    rail::register(&mut commands);
    bus::register_aliases(&mut commands);
    bus::Commands(commands)
}

/// Logs the prototype import and tells the page what came over.
fn report_import(state: &AppState, import: Option<Import>) {
    match import {
        Some(Import::Imported(report)) => {
            log::info!("{}", report.log_text());
            state.launches.notice(report.notice());
        }
        Some(Import::Failed(e)) => log::warn!("Importing the prototype's settings failed: {e}"),
        None => {}
    }
}

fn log_plugin() -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri_plugin_log::Builder::new()
        .targets([
            Target::new(TargetKind::Stdout),
            Target::new(TargetKind::LogDir { file_name: None }),
        ])
        .rotation_strategy(RotationStrategy::KeepSome(3))
        .max_file_size(MAX_LOG_BYTES)
        .timezone_strategy(TimezoneStrategy::UseLocal)
        // The dispatch caps every later `log::set_max_level`, so it lets everything through;
        // `init_logging` sets the effective level.
        .level(log::LevelFilter::Trace)
        .level_for("tao", log::LevelFilter::Warn)
        .level_for("wry", log::LevelFilter::Warn)
        .level_for("tauri_runtime_wry", log::LevelFilter::Info)
        .build()
}

/// Runs after `log_plugin`'s setup, which raises the max level to `Trace`, and applies the effective level.
fn init_logging(level: LogLevel) -> tauri::plugin::TauriPlugin<tauri::Wry> {
    tauri::plugin::Builder::new("log-level")
        .setup(move |_, _| {
            log::set_max_level(level.into());
            Ok(())
        })
        .build()
}

#[cfg(debug_assertions)]
fn apply_dev_frame(app: &tauri::App) {
    let (Some((x, y, w, h)), Some(window)) =
        (crate::dev::window_frame(), app.get_webview_window("main"))
    else {
        return;
    };
    let _ = window.set_position(tauri::LogicalPosition::new(x, y));
    let _ = window.set_size(tauri::LogicalSize::new(w, h));
}

/// Without the socket the app still runs, only without single-instance handoff.
fn start_ipc(handle: AppHandle, socket: &Path) {
    let listener = match ipc::bind(socket) {
        Ok(listener) => listener,
        Err(e) => {
            log::warn!("Cannot listen on {}: {e}", socket.display());
            return;
        }
    };
    tauri::async_runtime::spawn(ipc::serve(listener, move |message| {
        remote::handle(handle.clone(), message)
    }));
}
