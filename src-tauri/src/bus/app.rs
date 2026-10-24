//! The bus inside the app: the app is its host, menus and shortcuts spawn commands, the page runs them
//! with `run_command`.

use tauri::{AppHandle, Manager};

use super::{CommandBus, Host};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::{notify, window};

/// The app's commands, registered once at startup.
pub struct Commands(pub CommandBus<AppHandle>);

impl std::ops::Deref for Commands {
    type Target = CommandBus<AppHandle>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Host for AppHandle {
    fn has_vault(&self) -> bool {
        self.state::<AppState>().settings.get().vault.path.is_some()
    }

    fn choose_vault(&self) {
        window::open_settings(self, Some("vault"));
    }

    fn report_failure(&self, _id: &str, error: &AppError) {
        notify::show(self, crate::APP_NAME, &error.to_string());
    }
}

/// Runs a command without waiting for it (menu items, global shortcuts).
pub fn spawn(app: &AppHandle, id: &str) {
    let app = app.clone();
    let id = id.to_string();
    tauri::async_runtime::spawn(async move {
        let commands = app.state::<Commands>();
        commands.run(app.clone(), &id).await;
    });
}

/// The page runs window-level commands by id; a failure comes back as the error.
#[tauri::command]
pub async fn run_command(app: AppHandle, id: String) -> AppResult<()> {
    let work = app.state::<Commands>().start(app.clone(), &id)?;
    work.await
}
