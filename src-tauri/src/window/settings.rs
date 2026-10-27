//! The Settings window: a regular window of its own, built on first use and hidden on close, so it
//! reopens on the page it showed.

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use super::native;
use crate::error::{AppError, AppResult};
use crate::events::OPEN_SETTINGS_EVENT;

pub const LABEL: &str = "settings";

/// Every Settings page, in the page list's order: the ids `settings:<page>` opens and the palette lists.
/// The page registers its body under the same id (`src/components/settings/pages.tsx`).
pub const PAGES: [(&str, &str); 16] = [
    ("connections", "Connections"),
    ("contexts", "Contexts"),
    ("projects", "Projects"),
    ("daily-note", "Daily Note"),
    ("routines", "Routines"),
    ("journal", "Journal"),
    ("search", "Search"),
    ("workflows", "Workflows"),
    ("reminders", "Reminders"),
    ("links", "Links"),
    ("music", "Music"),
    ("general", "General"),
    ("appearance", "Appearance"),
    ("shortcuts", "Shortcuts"),
    ("automation", "Automation"),
    ("status", "Status"),
];

/// Former pages that are now a group of another page: `(id, page, title)`. `settings:<id>` opens the page
/// and the page scrolls to the group registered under the same id.
pub const SECTIONS: [(&str, &str, &str); 8] = [
    ("vault", "connections", "Vault"),
    ("calendar", "connections", "Calendar"),
    ("slack", "connections", "Slack"),
    ("jira", "connections", "Jira"),
    ("timetracker", "connections", "TimeTracker"),
    ("calls", "workflows", "Calls"),
    ("tools", "general", "Tools"),
    ("rail", "appearance", "Rail"),
];

pub fn page_title(id: &str) -> Option<&'static str> {
    PAGES
        .iter()
        .find(|(page, _)| *page == id)
        .map(|(_, title)| *title)
}

/// The page that shows `id`: the page itself, or the page holding that section.
pub fn page_of(id: &str) -> Option<&'static str> {
    PAGES
        .iter()
        .map(|(page, _)| *page)
        .find(|page| *page == id)
        .or_else(|| {
            SECTIONS
                .iter()
                .find(|(section, ..)| *section == id)
                .map(|(_, page, _)| *page)
        })
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

/// `settings:<page>`: a page or a section of one; refuses anything else.
pub fn open_page(app: &AppHandle, page: &str) -> AppResult<()> {
    if page_of(page).is_none() {
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
    }

    #[test]
    fn a_section_id_has_no_page_title() {
        for (id, ..) in SECTIONS {
            assert_eq!(page_title(id), None, "{id}");
        }
    }

    #[test]
    fn the_pages_are_listed_in_order() {
        let ids: Vec<&str> = PAGES.iter().map(|(id, _)| *id).collect();
        assert_eq!(
            ids,
            [
                "connections",
                "contexts",
                "projects",
                "daily-note",
                "routines",
                "journal",
                "search",
                "workflows",
                "reminders",
                "links",
                "music",
                "general",
                "appearance",
                "shortcuts",
                "automation",
                "status",
            ]
        );
    }

    #[test]
    fn a_page_is_its_own_page_and_a_section_belongs_to_its_page() {
        for (id, _) in PAGES {
            assert_eq!(page_of(id), Some(id), "{id}");
        }
        for (id, page, _) in SECTIONS {
            assert_eq!(page_of(id), Some(page), "{id}");
        }
        assert_eq!(page_of("calendar"), Some("connections"));
        assert_eq!(page_of("calls"), Some("workflows"));
    }

    #[test]
    fn an_unknown_id_has_no_page() {
        for id in ["", "nope", "Calendar", "daily note"] {
            assert_eq!(page_of(id), None, "{id}");
        }
    }

    #[test]
    fn every_section_lives_on_a_listed_page() {
        for (id, page, _) in SECTIONS {
            assert!(page_title(page).is_some(), "{id} -> {page}");
        }
    }

    #[test]
    fn no_section_id_is_also_a_page_id() {
        for (id, ..) in SECTIONS {
            assert!(PAGES.iter().all(|(page, _)| *page != id), "{id}");
        }
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
