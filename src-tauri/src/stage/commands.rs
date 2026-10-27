//! The pages' answers to the hub's `stage:collect` and `stage:act` (src/stage/), and Show in Finder
//! for the discovery file (Settings → Automation).

use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

use super::page::{ActReply, AnchorsReply};
use crate::error::{AppError, AppResult};

#[tauri::command]
pub fn stage_anchors(app: AppHandle, reply: AnchorsReply) {
    super::stage(&app).hub.receive_anchors(reply);
}

#[tauri::command]
pub fn stage_done(app: AppHandle, reply: ActReply) {
    super::stage(&app).hub.receive_act(reply);
}

#[tauri::command]
pub fn stage_reveal_file(app: AppHandle) -> AppResult<()> {
    let file = super::stage_file(&app)
        .ok_or_else(|| AppError::NotFound("Finder couldn't show the file.".into()))?;
    app.opener()
        .reveal_item_in_dir(file)
        .map_err(|e| AppError::Io(e.to_string()))
}
