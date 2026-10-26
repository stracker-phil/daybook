//! The search overlay's Tauri commands.

use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_plugin_opener::OpenerExt;
use ts_rs::TS;

use super::preview::{self, NotePreview, guard};
use super::rank::TextPart;
use super::{run_query, scopes};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use crate::vault::commands::{blocking, open_target};
use crate::vault::vault;
use crate::window;

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub path: String,
    pub name: String,
    /// `""` at the vault root.
    pub folder: String,
    /// The name, split at the terms.
    pub name_parts: Vec<TextPart>,
    /// The snippet, split at the terms; empty when the note has no text line.
    pub line_parts: Vec<TextPart>,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct SearchResponse {
    /// Best first, at most 50; none without terms.
    pub results: Vec<SearchResult>,
    pub terms: Vec<String>,
    /// The chips' scopes, then the typed `@scope`s, once each; unknown names included.
    pub scopes: Vec<String>,
    pub indexing: bool,
    pub notes: u32,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct SearchScopes {
    /// Settings order, then `all`.
    pub names: Vec<String>,
    /// Unknown scopes and cycles in Settings → Search.
    pub problems: Vec<String>,
}

/// The notes matching `text` within `scopes` (the chips; none: the whole vault).
#[tauri::command]
pub async fn search_query(
    app: AppHandle,
    text: String,
    scopes: Vec<String>,
) -> AppResult<SearchResponse> {
    blocking(move || Ok(run_query(&app, &text, scopes))).await
}

#[tauri::command]
pub fn search_scopes(app: AppHandle) -> SearchScopes {
    let settings = app.state::<AppState>().settings.get();
    let resolved = scopes::resolve(&settings.search.scopes);
    SearchScopes {
        names: resolved.names(),
        problems: resolved.problems,
    }
}

/// The note at `path` (vault-relative) for the preview.
#[tauri::command]
pub async fn search_note(app: AppHandle, path: String) -> AppResult<NotePreview> {
    guard(&path)?;
    blocking(move || {
        let state = app.state::<AppState>();
        preview::note_preview(&vault(&state.settings.get())?, &path, &*state.search.index)
    })
    .await
}

/// The image at `path` as a `data:` URL.
#[tauri::command]
pub async fn search_asset(app: AppHandle, path: String) -> AppResult<String> {
    guard(&path)?;
    blocking(move || {
        let settings = app.state::<AppState>().settings.get();
        preview::asset_data_url(&vault(&settings)?, &path)
    })
    .await
}

/// Closes the overlay, leaving focus to Obsidian, and opens the note there.
#[tauri::command]
pub fn search_open(app: AppHandle, path: String) -> AppResult<()> {
    guard(&path)?;
    window::panel::close(&app, true);
    open_target(&app, &path)
}

/// The note's wikilink, e.g. `[[Plan]]`.
#[tauri::command]
pub fn search_link(app: AppHandle, path: String) -> AppResult<String> {
    guard(&path)?;
    Ok(app.state::<AppState>().search.index.wikilink_for(&path))
}

/// Whether the preview may open `url`: web, mail and Obsidian links.
fn is_openable(url: &str) -> bool {
    url.split_once(':').is_some_and(|(scheme, _)| {
        ["http", "https", "mailto", "obsidian"]
            .iter()
            .any(|known| scheme.eq_ignore_ascii_case(known))
    })
}

#[tauri::command]
pub fn search_open_url(app: AppHandle, url: String) -> AppResult<()> {
    if !is_openable(&url) {
        return Err(AppError::InvalidInput("Unsupported link".into()));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Io(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_mail_and_obsidian_links_are_openable() {
        for (url, expected) in [
            ("https://example.com", true),
            ("http://example.com", true),
            ("HTTPS://example.com", true),
            ("mailto:me@example.com", true),
            ("obsidian://open?vault=Vault", true),
            ("file:///etc/passwd", false),
            ("javascript:alert(1)", false),
            ("x-apple.systempreferences:com.apple", false),
            ("#note=Plan", false),
            ("example.com", false),
            ("", false),
        ] {
            assert_eq!(is_openable(url), expected, "{url}");
        }
    }
}
