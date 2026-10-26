//! One tick for the work that falls due by the clock (docs/routines.md#the-scheduler): the context
//! switch by work hours, the changes waiting for today's note, then the wizard. Coming back to the Mac
//! ticks at once.

use std::sync::Arc;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use crate::state::AppState;
use crate::{contexts, wizard};

const TICK: Duration = Duration::from_secs(15);

async fn tick(app: &AppHandle) {
    contexts::apply_schedule(app);
    let state = app.state::<AppState>();
    let notes = Arc::clone(&state.notes);
    let settings = state.settings.get();
    if let Err(e) =
        tauri::async_runtime::spawn_blocking(move || notes.note_appeared(&settings)).await
    {
        log::warn!("Writing the waiting changes failed: {e}");
    }
    wizard::tick(app).await;
}

pub fn tick_soon(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { tick(&app).await });
}

pub fn start(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut ticks = tokio::time::interval(TICK);
        loop {
            ticks.tick().await;
            tick(&app).await;
        }
    });
}
