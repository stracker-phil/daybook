//! The windows Stage sees: the Daybook window (`hub`), Settings, the overlay (`panel`), the rail and
//! the quick-notes editor, in `describe` order, and how each is described. Pure.

use std::collections::BTreeMap;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::page::{PageAnchor, PageMeta, PanelState, Role};
use super::protocol::{ScreenRect, to_screen_rect};
use crate::automation::AutomationError;
use crate::window::settings::{PAGES, page_of};

/// A window as the host sees it, in screen points.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub title: String,
    pub focused: bool,
    pub bounds: ScreenRect,
    /// The page's area; anchors are measured from its top-left.
    pub content: ScreenRect,
}

/// The windows the hook exposes, declared in `describe` order, which `Ord` follows. The ids are the
/// prototype's, so its scripts keep working.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, TS,
)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "kebab-case")]
pub enum StagePage {
    #[default]
    Hub,
    Settings,
    Panel,
    Rail,
    QuickNotes,
}

impl StagePage {
    pub const ALL: [StagePage; 5] = [
        StagePage::Hub,
        StagePage::Settings,
        StagePage::Panel,
        StagePage::Rail,
        StagePage::QuickNotes,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            StagePage::Hub => "hub",
            StagePage::Settings => "settings",
            StagePage::Panel => "panel",
            StagePage::Rail => "rail",
            StagePage::QuickNotes => "quick-notes",
        }
    }

    /// Its Tauri window label.
    pub fn label(self) -> &'static str {
        match self {
            StagePage::Hub => crate::window::MAIN,
            StagePage::Settings => crate::window::settings::LABEL,
            StagePage::Panel => crate::window::panel::LABEL,
            StagePage::Rail => crate::rail::RAIL,
            StagePage::QuickNotes => crate::window::quick_notes::LABEL,
        }
    }

    /// The page shown in the window with this Tauri label.
    pub fn of_label(label: &str) -> Option<StagePage> {
        StagePage::ALL
            .into_iter()
            .find(|page| page.label() == label)
    }

    /// The title while the window has none.
    fn fallback_title(self) -> &'static str {
        match self {
            StagePage::Hub | StagePage::Panel => "Daybook",
            StagePage::Settings => "Settings",
            StagePage::Rail => "Rail",
            StagePage::QuickNotes => "Quick Notes",
        }
    }

    /// Whether the user can place this window; the overlay and the rail can't.
    pub fn movable(self) -> bool {
        matches!(
            self,
            StagePage::Hub | StagePage::Settings | StagePage::QuickNotes
        )
    }
}

impl FromStr for StagePage {
    type Err = AutomationError;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        StagePage::ALL
            .into_iter()
            .find(|page| page.as_str() == id)
            .ok_or_else(|| {
                let all: Vec<_> = StagePage::ALL.iter().map(|p| p.as_str()).collect();
                AutomationError::new(404, format!("“window” must be one of {}.", all.join(", ")))
            })
    }
}

/// A Settings page as `sections` lists it.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Section {
    pub id: &'static str,
    pub title: &'static str,
}

/// Settings' pages, in the page list's order.
pub fn sections() -> Vec<Section> {
    PAGES
        .iter()
        .map(|&(id, title)| Section { id, title })
        .collect()
}

/// A page, or a former page that is now a group of one (`window::settings::SECTIONS`).
pub fn is_section(id: &str) -> bool {
    page_of(id).is_some()
}

/// One window in `describe` and the `window` verb's answer.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct WindowInfo {
    pub id: StagePage,
    pub title: String,
    pub visible: bool,
    pub focused: bool,
    pub movable: bool,
    pub bounds: Option<ScreenRect>,
    pub content: Option<ScreenRect>,
    pub view: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sections: Option<Vec<Section>>,
    /// Only Settings has it; None while hidden.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub section: Option<Option<String>>,
    /// Kept while the window is hidden.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ui: Option<BTreeMap<String, PanelState>>,
}

/// What the host and the pages tell about one window.
#[derive(Debug, Clone, Default)]
pub struct WindowState<'a> {
    /// None while the window doesn't exist.
    pub frame: Option<&'a Frame>,
    /// On screen as the app means it (the overlay open, the rail slid in).
    pub shown: bool,
    /// What its page last reported.
    pub meta: Option<&'a PageMeta>,
    /// The open overlay's view, from the backend.
    pub panel_view: Option<String>,
}

pub fn window_info(page: StagePage, state: WindowState) -> WindowInfo {
    let visible_frame = state.shown.then_some(state.frame).flatten();
    let view = match page {
        StagePage::Panel => state.panel_view,
        StagePage::Hub => state.meta.and_then(|m| m.view.clone()),
        _ => None,
    };
    let settings = page == StagePage::Settings;
    WindowInfo {
        id: page,
        title: state
            .frame
            .map(|f| f.title.as_str())
            .filter(|t| !t.is_empty())
            .unwrap_or(page.fallback_title())
            .into(),
        visible: visible_frame.is_some(),
        focused: visible_frame.is_some_and(|f| f.focused),
        movable: page.movable(),
        bounds: visible_frame.map(|f| f.bounds),
        content: visible_frame.map(|f| f.content),
        view: visible_frame.and(view),
        sections: settings.then(sections),
        section: settings.then(|| visible_frame.and(state.meta.and_then(|m| m.section.clone()))),
        ui: state.meta.and_then(|m| m.ui.clone()),
    }
}

/// One anchor in `describe` and `wait`, in screen points.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnchorInfo {
    pub id: String,
    pub label: String,
    pub window: StagePage,
    pub rect: ScreenRect,
    pub in_view: bool,
    pub visible: Option<ScreenRect>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selected: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checked: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expanded: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

/// A page's anchors moved to its window's content origin, rounded.
pub fn to_screen_anchors(
    page: StagePage,
    anchors: Vec<PageAnchor>,
    content: ScreenRect,
) -> Vec<AnchorInfo> {
    anchors
        .into_iter()
        .map(|anchor| AnchorInfo {
            window: page,
            rect: to_screen_rect(anchor.rect, content),
            visible: anchor.visible.map(|v| to_screen_rect(v, content)),
            in_view: anchor.in_view,
            role: anchor.role,
            list: anchor.list,
            selected: anchor.selected,
            checked: anchor.checked,
            expanded: anchor.expanded,
            disabled: anchor.disabled,
            id: anchor.id,
            label: anchor.label,
        })
        .collect()
}

/// In window order, then top to bottom, then left to right.
pub fn sort_anchors(anchors: &mut [AnchorInfo]) {
    anchors.sort_by_key(|a| (a.window, a.rect.y, a.rect.x));
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::stage::protocol::PageRect;
    use crate::window::settings::SECTIONS;

    fn screen(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
        ScreenRect {
            x,
            y,
            width,
            height,
        }
    }

    fn frame(title: &str, focused: bool) -> Frame {
        Frame {
            title: title.into(),
            focused,
            bounds: screen(100, 80, 1280, 760),
            content: screen(100, 108, 1280, 732),
        }
    }

    fn page_anchor(id: &str, x: f64, y: f64) -> PageAnchor {
        PageAnchor {
            id: id.into(),
            label: id.into(),
            rect: PageRect {
                x,
                y,
                width: 10.0,
                height: 10.0,
            },
            in_view: true,
            visible: None,
            role: None,
            list: None,
            selected: None,
            checked: None,
            expanded: None,
            disabled: None,
        }
    }

    fn info_json(page: StagePage, state: WindowState) -> serde_json::Value {
        serde_json::to_value(window_info(page, state)).unwrap()
    }

    #[test]
    fn a_tauri_label_names_its_page() {
        for (label, page) in [
            ("main", Some(StagePage::Hub)),
            ("settings", Some(StagePage::Settings)),
            ("panel", Some(StagePage::Panel)),
            ("rail", Some(StagePage::Rail)),
            ("quicknotes", Some(StagePage::QuickNotes)),
            ("railtip", None),
            ("hub", None),
            ("", None),
        ] {
            assert_eq!(StagePage::of_label(label), page, "{label:?}");
        }
    }

    #[test]
    fn every_page_id_parses_back_to_its_page() {
        let ids: Vec<&str> = StagePage::ALL.iter().map(|p| p.as_str()).collect();

        assert_eq!(ids, ["hub", "settings", "panel", "rail", "quick-notes"]);
        for page in StagePage::ALL {
            assert_eq!(page.as_str().parse::<StagePage>(), Ok(page));
        }
    }

    #[test]
    fn an_unknown_page_id_is_not_found_and_lists_the_ids() {
        for id in ["main", "quicknotes", "Hub", ""] {
            assert_eq!(
                id.parse::<StagePage>(),
                Err(AutomationError::new(
                    404,
                    "“window” must be one of hub, settings, panel, rail, quick-notes."
                )),
                "{id:?}"
            );
        }
    }

    #[test]
    fn a_page_serializes_as_its_id() {
        for page in StagePage::ALL {
            assert_eq!(serde_json::to_value(page).unwrap(), json!(page.as_str()));
        }
    }

    #[test]
    fn pages_follow_the_describe_order() {
        let mut shuffled = [
            StagePage::QuickNotes,
            StagePage::Hub,
            StagePage::Rail,
            StagePage::Settings,
            StagePage::Panel,
        ];

        shuffled.sort();

        assert_eq!(shuffled, StagePage::ALL);
    }

    #[test]
    fn only_the_windows_with_a_frame_of_their_own_are_movable() {
        for (page, movable) in [
            (StagePage::Hub, true),
            (StagePage::Settings, true),
            (StagePage::QuickNotes, true),
            (StagePage::Panel, false),
            (StagePage::Rail, false),
        ] {
            assert_eq!(page.movable(), movable, "{page:?}");
        }
    }

    #[test]
    fn the_sections_are_the_settings_pages_in_order() {
        let sections = sections();

        assert_eq!(sections.len(), PAGES.len());
        assert_eq!(
            sections[0],
            Section {
                id: "connections",
                title: "Connections",
            }
        );
        assert_eq!(
            serde_json::to_value(&sections[0]).unwrap(),
            json!({ "id": "connections", "title": "Connections" })
        );
        assert!(is_section("automation"));
        assert!(!is_section("nope"));
        assert!(!is_section("Connections"));
    }

    #[test]
    fn a_settings_section_id_is_a_section() {
        for (id, ..) in SECTIONS {
            assert!(is_section(id), "{id}");
        }
    }

    #[test]
    fn a_shown_hub_is_described_with_its_frame_view_and_panels() {
        let frame = frame("Daybook", true);
        let meta = PageMeta {
            view: Some("today".into()),
            section: None,
            ui: Some(BTreeMap::from([(
                "inspector".to_string(),
                PanelState::Open,
            )])),
        };

        let described = info_json(
            StagePage::Hub,
            WindowState {
                frame: Some(&frame),
                shown: true,
                meta: Some(&meta),
                panel_view: Some("ignored".into()),
            },
        );

        assert_eq!(
            described,
            json!({
                "id": "hub",
                "title": "Daybook",
                "visible": true,
                "focused": true,
                "movable": true,
                "bounds": { "x": 100, "y": 80, "width": 1280, "height": 760 },
                "content": { "x": 100, "y": 108, "width": 1280, "height": 732 },
                "view": "today",
                "ui": { "inspector": "open" },
            })
        );
    }

    #[test]
    fn a_hidden_hub_has_no_geometry_or_view_but_keeps_its_panels() {
        let frame = frame("Daybook", true);
        let meta = PageMeta {
            view: Some("today".into()),
            section: None,
            ui: Some(BTreeMap::from([(
                "inspector".to_string(),
                PanelState::Closed,
            )])),
        };

        let described = info_json(
            StagePage::Hub,
            WindowState {
                frame: Some(&frame),
                shown: false,
                meta: Some(&meta),
                panel_view: None,
            },
        );

        assert_eq!(
            described,
            json!({
                "id": "hub",
                "title": "Daybook",
                "visible": false,
                "focused": false,
                "movable": true,
                "bounds": null,
                "content": null,
                "view": null,
                "ui": { "inspector": "closed" },
            })
        );
    }

    #[test]
    fn a_shown_window_without_a_frame_counts_as_hidden() {
        let described = window_info(
            StagePage::Hub,
            WindowState {
                shown: true,
                ..WindowState::default()
            },
        );

        assert!(!described.visible);
        assert_eq!(described.bounds, None);
    }

    #[test]
    fn a_window_without_a_title_gets_its_pages_fallback() {
        let untitled = frame("", false);

        for (page, title) in [
            (StagePage::Hub, "Daybook"),
            (StagePage::Settings, "Settings"),
            (StagePage::Panel, "Daybook"),
            (StagePage::Rail, "Rail"),
            (StagePage::QuickNotes, "Quick Notes"),
        ] {
            let without_window = window_info(page, WindowState::default());
            let with_empty_title = window_info(
                page,
                WindowState {
                    frame: Some(&untitled),
                    shown: true,
                    ..WindowState::default()
                },
            );

            assert_eq!(without_window.title, title, "{page:?}");
            assert_eq!(with_empty_title.title, title, "{page:?}");
        }
    }

    #[test]
    fn a_windows_own_title_wins_over_the_fallback() {
        let titled = frame("Daybook — Friday", false);

        let described = window_info(
            StagePage::Hub,
            WindowState {
                frame: Some(&titled),
                ..WindowState::default()
            },
        );

        assert_eq!(described.title, "Daybook — Friday");
    }

    #[test]
    fn settings_lists_its_sections_and_the_one_showing() {
        let frame = frame("Settings", false);
        let meta = PageMeta {
            section: Some("slack".into()),
            ..PageMeta::default()
        };

        let shown = info_json(
            StagePage::Settings,
            WindowState {
                frame: Some(&frame),
                shown: true,
                meta: Some(&meta),
                panel_view: None,
            },
        );
        let hidden = info_json(
            StagePage::Settings,
            WindowState {
                frame: Some(&frame),
                shown: false,
                meta: Some(&meta),
                panel_view: None,
            },
        );

        assert_eq!(shown["section"], json!("slack"));
        assert_eq!(
            shown["sections"][0],
            json!({ "id": "connections", "title": "Connections" })
        );
        assert_eq!(shown["sections"].as_array().unwrap().len(), PAGES.len());
        assert_eq!(shown["view"], json!(null));
        assert_eq!(hidden["section"], json!(null));
        assert!(hidden["sections"].is_array());
    }

    #[test]
    fn only_settings_has_sections() {
        for page in [
            StagePage::Hub,
            StagePage::Panel,
            StagePage::Rail,
            StagePage::QuickNotes,
        ] {
            let described = info_json(page, WindowState::default());

            assert!(described.get("sections").is_none(), "{page:?}");
            assert!(described.get("section").is_none(), "{page:?}");
            assert!(described.get("ui").is_none(), "{page:?}");
        }
    }

    #[test]
    fn the_overlays_view_comes_from_the_backend_while_it_is_shown() {
        let frame = frame("Daybook", true);

        let shown = window_info(
            StagePage::Panel,
            WindowState {
                frame: Some(&frame),
                shown: true,
                meta: None,
                panel_view: Some("palette".into()),
            },
        );
        let closed = window_info(
            StagePage::Panel,
            WindowState {
                frame: Some(&frame),
                shown: false,
                meta: None,
                panel_view: Some("palette".into()),
            },
        );

        assert_eq!(shown.view.as_deref(), Some("palette"));
        assert_eq!(closed.view, None);
    }

    #[test]
    fn page_anchors_move_to_the_content_origin_and_round() {
        let anchors = vec![PageAnchor {
            id: "tool:activity".into(),
            label: "Log activity…".into(),
            rect: PageRect {
                x: 16.4,
                y: 100.6,
                width: 208.2,
                height: 32.0,
            },
            in_view: true,
            visible: Some(PageRect {
                x: 16.4,
                y: 100.6,
                width: 208.2,
                height: 20.0,
            }),
            role: Some(Role::Button),
            ..page_anchor("", 0.0, 0.0)
        }];

        let screen_anchors = to_screen_anchors(StagePage::Hub, anchors, screen(120, 88, 1240, 692));

        assert_eq!(
            serde_json::to_value(&screen_anchors).unwrap(),
            json!([{
                "id": "tool:activity",
                "label": "Log activity…",
                "window": "hub",
                "rect": { "x": 136, "y": 189, "width": 208, "height": 32 },
                "inView": true,
                "visible": { "x": 136, "y": 189, "width": 208, "height": 20 },
                "role": "button",
            }])
        );
    }

    #[test]
    fn an_anchor_with_nothing_visible_and_its_states_keeps_them() {
        let anchors = vec![PageAnchor {
            in_view: false,
            list: Some("tools".into()),
            selected: Some(true),
            checked: Some(false),
            expanded: Some(true),
            disabled: Some(false),
            ..page_anchor("tool:a", 0.0, 0.0)
        }];

        let screen_anchors = to_screen_anchors(StagePage::Panel, anchors, screen(0, 0, 100, 100));

        assert_eq!(
            serde_json::to_value(&screen_anchors[0]).unwrap(),
            json!({
                "id": "tool:a",
                "label": "tool:a",
                "window": "panel",
                "rect": { "x": 0, "y": 0, "width": 10, "height": 10 },
                "inView": false,
                "visible": null,
                "list": "tools",
                "selected": true,
                "checked": false,
                "expanded": true,
                "disabled": false,
            })
        );
    }

    #[test]
    fn anchors_sort_by_window_then_top_to_bottom_then_left_to_right() {
        let content = screen(0, 0, 100, 100);
        let mut anchors = to_screen_anchors(
            StagePage::Panel,
            vec![page_anchor("panel", 0.0, 0.0)],
            content,
        );
        anchors.extend(to_screen_anchors(
            StagePage::Hub,
            vec![
                page_anchor("hub-low", 0.0, 50.0),
                page_anchor("hub-right", 40.0, 10.0),
                page_anchor("hub-left", 5.0, 10.0),
            ],
            content,
        ));
        anchors.extend(to_screen_anchors(
            StagePage::Settings,
            vec![page_anchor("settings", 90.0, 90.0)],
            content,
        ));

        sort_anchors(&mut anchors);

        let ids: Vec<&str> = anchors.iter().map(|a| a.id.as_str()).collect();
        assert_eq!(
            ids,
            ["hub-left", "hub-right", "hub-low", "settings", "panel"]
        );
    }
}
