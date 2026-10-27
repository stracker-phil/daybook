//! Settings → Connections → Slack: connecting a token, testing and removing it. The token never goes back to the page.

use serde::Serialize;
use tauri::AppHandle;
use ts_rs::TS;

use super::api::{SlackIdentity, is_token};
use super::token::Tokens;
use super::{changed, slack, token};
use crate::error::{AppError, AppResult};
use crate::lock::MutexExt;
use crate::secrets::Secret;

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct SlackInfo {
    pub connected: bool,
    /// Who the token belongs to, once checked since the start.
    pub identity: Option<SlackIdentity>,
}

#[tauri::command]
pub async fn slack_info(app: AppHandle) -> SlackInfo {
    let connected = token(&app).await.is_some();
    SlackInfo {
        connected,
        identity: slack(&app).identity.lock_unpoisoned().clone(),
    }
}

async fn check(app: &AppHandle, secret: &Secret) -> AppResult<SlackIdentity> {
    let identity = slack(app)
        .api
        .auth_test(secret)
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;
    *slack(app).identity.lock_unpoisoned() = Some(identity.clone());
    Ok(identity)
}

/// Runs a Keychain operation on a blocking thread.
async fn keychain(
    app: &AppHandle,
    operation: impl FnOnce(&Tokens) -> Result<(), String> + Send + 'static,
    failure: &str,
) -> AppResult<()> {
    let handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || operation(&slack(&handle).tokens))
        .await
        .map_err(|e| AppError::Io(e.to_string()))?
        .map_err(|e| AppError::Io(format!("{failure}: {e}")))
}

/// Checks the token with Slack, then stores it in the Keychain.
#[tauri::command]
pub async fn slack_connect(app: AppHandle, token: String) -> AppResult<SlackIdentity> {
    let typed = token.trim();
    if !is_token(typed) {
        return Err(AppError::InvalidInput(
            "That doesn't look like a Slack token (xoxp-…).".into(),
        ));
    }
    let secret = Secret::new(typed);
    let identity = check(&app, &secret).await?;
    keychain(
        &app,
        move |tokens| tokens.save(secret),
        "Couldn't store the token in the Keychain",
    )
    .await?;
    log::info!("Slack connected: {} in {}", identity.user, identity.team);
    changed(&app);
    Ok(identity)
}

#[tauri::command]
pub async fn slack_test(app: AppHandle) -> AppResult<SlackIdentity> {
    let stored = token(&app)
        .await
        .ok_or_else(|| AppError::NotFound("No Slack token is stored.".into()))?;
    check(&app, &stored).await
}

#[tauri::command]
pub async fn slack_disconnect(app: AppHandle) -> AppResult<()> {
    keychain(
        &app,
        Tokens::remove,
        "Couldn't remove the token from the Keychain",
    )
    .await?;
    *slack(&app).identity.lock_unpoisoned() = None;
    log::info!("Slack disconnected");
    changed(&app);
    Ok(())
}
