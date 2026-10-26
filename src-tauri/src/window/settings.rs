//! The Settings window: a regular window of its own, built on first use and hidden on close, so it
//! reopens on the page it showed.

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::native;
use crate::error::{AppError, AppResult};
use crate::events::OPEN_SETTINGS_EVENT;

pub const LABEL: &str = "settings";

/// Every Settings page, in the page list's order: the ids `settings:<page>` opens and the palette lists.
/// The page registers its body under the same id (`src/components/settings/pages.tsx`).
pub const PAGES: [(&str, &str); 18] = [
    ("vault", "Vault"),
    ("calendar", "Calendar"),
    ("slack", "Slack"),
    ("jira", "Jira"),
    ("contexts", "Contexts"),
    ("projects", "Projects"),
    ("daily-note", "Daily Note"),
    ("routines", "Routines"),
    ("journal", "Journal"),
    ("search", "Search"),
    ("workflows", "Workflows"),
    ("reminders", "Reminders"),
    ("calls", "Calls"),
    ("links", "Links"),
    ("general", "General"),
    ("tools", "Tools"),
    ("shortcuts", "Shortcuts"),
    ("appearance", "Appearance"),
];

pub fn page_title(id: &str) -> Option<&'static str> {
    PAGES
        .iter()
        .find(|(page, _)| *page == id)
        .map(|(_, title)| *title)
}

fn url(page: Option<&str>) -> WebviewUrl {
    let query = page.map(|page| format!("?page={page}")).unwrap_or_default();
    WebviewUrl::App(format!("src/windows/settings/index.html{query}").into())
}

fn build(app: &AppHandle, page: Option<&str>) -> tauri::Result<WebviewWindow> {
    WebviewWindowBuilder::new(app, LABEL, url(page))
        .title("Settings")
        .inner_size(880.0, 720.0)
        .min_inner_size(760.0, 560.0)
        .maximizable(false)
        .visible(false)
        .title_bar_style(tauri::TitleBarStyle::Overlay)
        .hidden_title(true)
        .traffic_light_position(tauri::LogicalPosition::new(20.0, 22.0))
        .build()
}

fn find_or_build(app: &AppHandle, page: Option<&str>) -> Option<WebviewWindow> {
    if let Some(window) = app.get_webview_window(LABEL) {
        if let Some(page) = page
            && let Err(e) = window.emit_to(LABEL, OPEN_SETTINGS_EVENT, page)
        {
            log::warn!("Couldn't show the Settings page {page}: {e}");
        }
        return Some(window);
    }
    build(app, page)
        .inspect_err(|e| log::warn!("Couldn't open Settings: {e}"))
        .ok()
}

fn show_on_pointer_screen(app: &AppHandle, window: WebviewWindow) {
    let shown = app.run_on_main_thread(move || {
        if let Ok(ns_window) = window.ns_window() {
            // SAFETY: run_on_main_thread runs this on the main thread; the window is alive.
            unsafe { native::move_to_pointer_screen(ns_window) };
        }
        if let Err(e) = window.show() {
            log::warn!("Couldn't show Settings: {e}");
        }
        super::bring_to_front(&window);
    });
    if let Err(e) = shown {
        log::warn!("Couldn't show Settings: {e}");
    }
}

/// Shows Settings on the pointer's screen, at `page` when given (else where it was), and brings it forward.
pub fn open(app: &AppHandle, page: Option<&str>) {
    let Some(window) = find_or_build(app, page) else {
        return;
    };
    super::set_dock_icon(app, true);
    show_on_pointer_screen(app, window);
}

/// `settings:<page>`: refuses a page that doesn't exist.
pub fn open_page(app: &AppHandle, page: &str) -> AppResult<()> {
    if page_title(page).is_none() {
        return Err(AppError::NotFound(format!("No Settings page {page}")));
    }
    open(app, Some(page));
    Ok(())
}

pub fn visible(app: &AppHandle) -> bool {
    super::is_visible(app, LABEL)
}

pub fn hide(app: &AppHandle) {
    super::hide_regular(app, LABEL);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_page_id_gives_its_title() {
        for (id, title) in PAGES {
            assert_eq!(page_title(id), Some(title), "{id}");
        }
        assert_eq!(page_title("daily-note"), Some("Daily Note"));
        assert_eq!(page_title("routines"), Some("Routines"));
        assert_eq!(page_title("journal"), Some("Journal"));
        assert_eq!(page_title("workflows"), Some("Workflows"));
        assert_eq!(page_title("calls"), Some("Calls"));
    }

    #[test]
    fn an_unknown_page_has_no_title() {
        for id in ["", "nope", "Vault", "daily note"] {
            assert_eq!(page_title(id), None, "{id}");
        }
    }

    #[test]
    fn page_ids_are_unique() {
        let ids: std::collections::HashSet<&str> = PAGES.iter().map(|(id, _)| *id).collect();
        assert_eq!(ids.len(), PAGES.len());
    }
}
