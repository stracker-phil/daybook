//! The messages between the hub and the pages (src/stage/): what Rust asks a page to do (`ActEvent`
//! on `stage:act`) and the pages' replies to `stage:collect` and `stage:act`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::keys::StageKeys;
use super::protocol::PageRect;
use super::windows::StagePage;

const MAX_TEXT: usize = 200;
const MAX_UI_KEY: usize = 100;

/// How `type` puts its text into the field.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "StageTypeMode")]
#[serde(rename_all = "lowercase")]
pub enum TypeMode {
    #[default]
    Replace,
    Append,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "StageScrollTo")]
#[serde(rename_all = "lowercase")]
pub enum ScrollTo {
    Top,
    Bottom,
}

/// One act for a page, validated by the hub. `command` and `wait` never reach a page.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "StageAct")]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ActRequest {
    Reveal {
        anchor: String,
    },
    Click {
        anchor: String,
    },
    Type {
        anchor: String,
        text: String,
        mode: TypeMode,
    },
    /// Either `by` (points, positive downwards) or `to`.
    Scroll {
        anchor: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        by: Option<f64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        to: Option<ScrollTo>,
    },
    /// `page` set: that window was focused for the keys; else the page whose document has focus takes them.
    Keys {
        keys: StageKeys,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[ts(optional)]
        page: Option<StagePage>,
    },
}

/// The `stage:act` event: an act and the id its reply carries.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "StageActRequest")]
pub struct ActEvent {
    pub request: String,
    #[serde(flatten)]
    pub act: ActRequest,
}

/// The `stage:collect` event; each page answers with an `AnchorsReply` through `stage_anchors`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(
    export,
    export_to = "src/api/types.gen.ts",
    rename = "StageCollectRequest"
)]
pub struct CollectEvent {
    pub request: String,
}

/// What an anchor is, for a presenter.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "StageRole")]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Region,
    Button,
    Field,
    Item,
    Tab,
    Toggle,
    Link,
    Menu,
    Text,
}

/// An element marked `data-stage="<id>"`, as its page measures it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "StagePageAnchor")]
#[serde(rename_all = "camelCase")]
pub struct PageAnchor {
    pub id: String,
    /// Cut to 80 characters by the page.
    pub label: String,
    pub rect: PageRect,
    pub in_view: bool,
    /// The part its scroll areas and the window let through; None when nothing shows.
    #[serde(default)]
    pub visible: Option<PageRect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub role: Option<Role>,
    /// Anchors with the same list are rows, in visual order.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub list: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub selected: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub checked: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub expanded: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub disabled: Option<bool>,
}

impl PageAnchor {
    fn sane(&self) -> bool {
        let id = self.id.trim();
        !id.is_empty()
            && id.chars().count() <= MAX_TEXT
            && self.label.chars().count() <= MAX_TEXT
            && self
                .list
                .as_ref()
                .is_none_or(|l| l.chars().count() <= MAX_TEXT)
            && self.rect.width > 0.0
            && self.rect.height > 0.0
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "StagePanelState")]
#[serde(rename_all = "lowercase")]
pub enum PanelState {
    Open,
    Closed,
}

/// What a page says about itself: the hub's view (`today`, `day:<date>`), the Settings page, open panels.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "StagePageMeta")]
pub struct PageMeta {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub view: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub section: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub ui: Option<BTreeMap<String, PanelState>>,
}

impl PageMeta {
    fn sane(&self) -> bool {
        let short = |t: &Option<String>| t.as_ref().is_none_or(|t| t.chars().count() <= MAX_TEXT);
        short(&self.view)
            && short(&self.section)
            && self
                .ui
                .as_ref()
                .is_none_or(|ui| ui.keys().all(|k| k.chars().count() <= MAX_UI_KEY))
    }
}

/// A page's answer to `stage:collect`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, TS)]
#[ts(
    export,
    export_to = "src/api/types.gen.ts",
    rename = "StageAnchorsReply"
)]
pub struct AnchorsReply {
    pub request: String,
    pub page: StagePage,
    pub anchors: Vec<PageAnchor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub meta: Option<PageMeta>,
}

impl AnchorsReply {
    /// Anchors without an id or a size, or with overlong texts, dropped; overlong meta dropped.
    pub fn cleaned(mut self) -> Self {
        self.anchors.retain(PageAnchor::sane);
        for anchor in &mut self.anchors {
            anchor.id = anchor.id.trim().to_string();
        }
        self.meta = self.meta.filter(PageMeta::sane);
        self
    }
}

/// A page's answer to `stage:act`; `handled` is false when the anchor isn't on that page (or the
/// keys aren't for it).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "StageActReply")]
pub struct ActReply {
    pub request: String,
    pub page: StagePage,
    pub handled: bool,
    pub ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub error: Option<String>,
    /// The HTTP status of a failure when it isn't 500 (e.g. 409: nothing scrolls).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub status: Option<u16>,
    /// The anchor's rectangle, in view, before acting.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub rect: Option<PageRect>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional = nullable)]
    pub visible: Option<PageRect>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn rect(width: f64, height: f64) -> PageRect {
        PageRect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        }
    }

    fn anchor(id: &str) -> PageAnchor {
        PageAnchor {
            id: id.into(),
            label: "Label".into(),
            rect: rect(10.0, 10.0),
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

    fn reply(anchors: Vec<PageAnchor>, meta: Option<PageMeta>) -> AnchorsReply {
        AnchorsReply {
            request: "r1".into(),
            page: StagePage::Hub,
            anchors,
            meta,
        }
    }

    fn cleaned_ids(anchors: Vec<PageAnchor>) -> Vec<String> {
        reply(anchors, None)
            .cleaned()
            .anchors
            .into_iter()
            .map(|a| a.id)
            .collect()
    }

    fn event(act: ActRequest) -> serde_json::Value {
        serde_json::to_value(ActEvent {
            request: "r1".into(),
            act,
        })
        .unwrap()
    }

    #[test]
    fn an_act_event_flattens_the_act_next_to_the_request_id() {
        assert_eq!(
            event(ActRequest::Reveal {
                anchor: "tool:a".into()
            }),
            json!({ "request": "r1", "kind": "reveal", "anchor": "tool:a" })
        );
        assert_eq!(
            event(ActRequest::Click {
                anchor: "tool:a".into()
            }),
            json!({ "request": "r1", "kind": "click", "anchor": "tool:a" })
        );
        assert_eq!(
            event(ActRequest::Type {
                anchor: "panel:input".into(),
                text: "hello".into(),
                mode: TypeMode::Append,
            }),
            json!({
                "request": "r1",
                "kind": "type",
                "anchor": "panel:input",
                "text": "hello",
                "mode": "append",
            })
        );
    }

    #[test]
    fn a_scroll_act_carries_only_the_distance_or_the_end_it_was_given() {
        assert_eq!(
            event(ActRequest::Scroll {
                anchor: "hub:notes".into(),
                by: Some(-240.0),
                to: None,
            }),
            json!({ "request": "r1", "kind": "scroll", "anchor": "hub:notes", "by": -240.0 })
        );
        assert_eq!(
            event(ActRequest::Scroll {
                anchor: "hub:notes".into(),
                by: None,
                to: Some(ScrollTo::Bottom),
            }),
            json!({ "request": "r1", "kind": "scroll", "anchor": "hub:notes", "to": "bottom" })
        );
    }

    #[test]
    fn a_keys_act_names_the_window_that_takes_them_when_there_is_one() {
        let keys = StageKeys {
            key: "K".into(),
            code: "KeyK".into(),
            meta: true,
            shift: true,
            alt: false,
            ctrl: false,
        };
        let keys_json = json!({
            "key": "K",
            "code": "KeyK",
            "meta": true,
            "shift": true,
            "alt": false,
            "ctrl": false,
        });

        assert_eq!(
            event(ActRequest::Keys {
                keys: keys.clone(),
                page: Some(StagePage::QuickNotes),
            }),
            json!({ "request": "r1", "kind": "keys", "keys": keys_json, "page": "quick-notes" })
        );
        assert_eq!(
            event(ActRequest::Keys { keys, page: None }),
            json!({ "request": "r1", "kind": "keys", "keys": keys_json })
        );
    }

    #[test]
    fn an_act_event_reads_back_from_its_json() {
        let sent = ActEvent {
            request: "r2".into(),
            act: ActRequest::Type {
                anchor: "a".into(),
                text: String::new(),
                mode: TypeMode::Replace,
            },
        };

        let back: ActEvent = serde_json::from_value(serde_json::to_value(&sent).unwrap()).unwrap();

        assert_eq!(back, sent);
    }

    #[test]
    fn a_collect_event_is_just_the_request_id() {
        assert_eq!(
            serde_json::to_value(CollectEvent {
                request: "r1".into()
            })
            .unwrap(),
            json!({ "request": "r1" })
        );
    }

    #[test]
    fn a_page_anchor_reads_camel_case_and_defaults_what_is_missing() {
        let anchor: PageAnchor = serde_json::from_value(json!({
            "id": "tool:a",
            "label": "Tool",
            "rect": { "x": 1.5, "y": 2.0, "width": 30.0, "height": 40.0 },
            "inView": false,
        }))
        .unwrap();

        let expected = PageAnchor {
            label: "Tool".into(),
            rect: PageRect {
                x: 1.5,
                y: 2.0,
                width: 30.0,
                height: 40.0,
            },
            in_view: false,
            ..self::anchor("tool:a")
        };
        assert_eq!(anchor, expected);
    }

    #[test]
    fn a_page_anchor_reads_its_role_and_states() {
        let anchor: PageAnchor = serde_json::from_value(json!({
            "id": "x",
            "label": "",
            "rect": { "x": 0, "y": 0, "width": 1, "height": 1 },
            "inView": true,
            "visible": { "x": 0, "y": 0, "width": 1, "height": 1 },
            "role": "toggle",
            "list": "tools",
            "selected": true,
            "checked": false,
            "expanded": true,
            "disabled": true,
        }))
        .unwrap();

        assert_eq!(anchor.role, Some(Role::Toggle));
        assert_eq!(anchor.visible, Some(rect(1.0, 1.0)));
        assert_eq!(anchor.list.as_deref(), Some("tools"));
        assert_eq!(
            (
                anchor.selected,
                anchor.checked,
                anchor.expanded,
                anchor.disabled
            ),
            (Some(true), Some(false), Some(true), Some(true))
        );
    }

    #[test]
    fn every_role_and_panel_state_is_lowercase_on_the_wire() {
        for (role, name) in [
            (Role::Region, "region"),
            (Role::Button, "button"),
            (Role::Field, "field"),
            (Role::Item, "item"),
            (Role::Tab, "tab"),
            (Role::Toggle, "toggle"),
            (Role::Link, "link"),
            (Role::Menu, "menu"),
            (Role::Text, "text"),
        ] {
            assert_eq!(serde_json::to_value(role).unwrap(), json!(name));
        }
        assert_eq!(
            serde_json::to_value(PanelState::Open).unwrap(),
            json!("open")
        );
        assert_eq!(
            serde_json::to_value(PanelState::Closed).unwrap(),
            json!("closed")
        );
    }

    #[test]
    fn an_anchors_reply_reads_the_page_and_its_meta() {
        let reply: AnchorsReply = serde_json::from_value(json!({
            "request": "r1",
            "page": "quick-notes",
            "anchors": [],
            "meta": { "view": "today", "ui": { "inspector": "open" } },
        }))
        .unwrap();

        assert_eq!(reply.page, StagePage::QuickNotes);
        assert_eq!(
            reply.meta,
            Some(PageMeta {
                view: Some("today".into()),
                section: None,
                ui: Some(BTreeMap::from([(
                    "inspector".to_string(),
                    PanelState::Open
                )])),
            })
        );
    }

    #[test]
    fn a_reply_from_an_unknown_page_is_refused() {
        let parsed = serde_json::from_value::<AnchorsReply>(json!({
            "request": "r1",
            "page": "dock",
            "anchors": [],
        }));

        assert!(parsed.is_err());
    }

    #[test]
    fn cleaning_drops_anchors_without_an_id() {
        assert_eq!(
            cleaned_ids(vec![anchor("a"), anchor(""), anchor("   "), anchor("b")]),
            ["a", "b"]
        );
    }

    #[test]
    fn cleaning_trims_the_id() {
        assert_eq!(cleaned_ids(vec![anchor("  tool:a ")]), ["tool:a"]);
    }

    #[test]
    fn cleaning_drops_anchors_without_a_size() {
        let mut flat = anchor("flat");
        flat.rect = rect(10.0, 0.0);
        let mut thin = anchor("thin");
        thin.rect = rect(0.0, 10.0);
        let mut negative = anchor("negative");
        negative.rect = rect(-5.0, 10.0);

        assert_eq!(
            cleaned_ids(vec![flat, thin, negative, anchor("ok")]),
            ["ok"]
        );
    }

    #[test]
    fn cleaning_drops_anchors_with_texts_over_two_hundred_characters() {
        let mut long_label = anchor("long-label");
        long_label.label = "é".repeat(201);
        let mut long_list = anchor("long-list");
        long_list.list = Some("l".repeat(201));
        let mut at_the_limit = anchor(&"i".repeat(200));
        at_the_limit.label = "l".repeat(200);
        at_the_limit.list = Some("l".repeat(200));

        assert_eq!(
            cleaned_ids(vec![
                anchor(&"i".repeat(201)),
                long_label,
                long_list,
                at_the_limit,
            ]),
            ["i".repeat(200)]
        );
    }

    #[test]
    fn cleaning_keeps_an_empty_label() {
        let mut unlabeled = anchor("unlabeled");
        unlabeled.label = String::new();

        assert_eq!(cleaned_ids(vec![unlabeled]), ["unlabeled"]);
    }

    #[test]
    fn cleaning_keeps_sane_meta_and_drops_overlong_meta() {
        let sane = PageMeta {
            view: Some("v".repeat(200)),
            section: Some("vault".into()),
            ui: Some(BTreeMap::from([("k".repeat(100), PanelState::Open)])),
        };
        let long_view = PageMeta {
            view: Some("v".repeat(201)),
            ..PageMeta::default()
        };
        let long_section = PageMeta {
            section: Some("s".repeat(201)),
            ..PageMeta::default()
        };
        let long_key = PageMeta {
            ui: Some(BTreeMap::from([("k".repeat(101), PanelState::Closed)])),
            ..PageMeta::default()
        };

        assert_eq!(reply(vec![], Some(sane.clone())).cleaned().meta, Some(sane));
        for meta in [long_view, long_section, long_key] {
            assert_eq!(reply(vec![], Some(meta)).cleaned().meta, None);
        }
        assert_eq!(reply(vec![], None).cleaned().meta, None);
    }

    #[test]
    fn an_act_reply_reads_a_minimal_answer_and_omits_what_it_lacks() {
        let reply: ActReply = serde_json::from_value(json!({
            "request": "r1",
            "page": "panel",
            "handled": false,
            "ok": false,
        }))
        .unwrap();

        assert_eq!(
            reply,
            ActReply {
                request: "r1".into(),
                page: StagePage::Panel,
                ..ActReply::default()
            }
        );
        assert_eq!(
            serde_json::to_value(&reply).unwrap(),
            json!({ "request": "r1", "page": "panel", "handled": false, "ok": false })
        );
    }

    #[test]
    fn an_act_reply_reads_a_failure_with_its_status_and_rectangles() {
        let reply: ActReply = serde_json::from_value(json!({
            "request": "r1",
            "page": "hub",
            "handled": true,
            "ok": false,
            "error": "“hub:notes” isn't in anything that scrolls",
            "status": 409,
            "rect": { "x": 1.0, "y": 2.0, "width": 3.0, "height": 4.0 },
            "visible": null,
        }))
        .unwrap();

        assert_eq!(reply.status, Some(409));
        assert_eq!(
            reply.error.as_deref(),
            Some("“hub:notes” isn't in anything that scrolls")
        );
        assert_eq!(
            reply.rect,
            Some(PageRect {
                x: 1.0,
                y: 2.0,
                width: 3.0,
                height: 4.0,
            })
        );
        assert_eq!(reply.visible, None);
    }
}
