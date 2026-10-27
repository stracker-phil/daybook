//! The Stage hook's verbs (docs/stage.md): `describe`, `state` and `window`; `act` is in `acts.rs`.
//! The HTTP API routes `/v1/stage/<verb>` here; status codes and messages are part of the protocol.

use serde::Serialize;
use serde_json::{Map, Value};

use super::catalog::{StageCommand, StageHotkey, stage_hotkeys};
use super::hub::{AppInfo, Hub, StageHost};
use super::protocol::{FEATURES, PROTOCOL, StageResult};
use super::requests::{WindowAction, WindowRequest};
use super::windows::{AnchorInfo, StagePage, WindowInfo, WindowState, window_info};
use crate::automation::{AutomationError, Params};

#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct Describe {
    pub protocol: u32,
    pub features: [&'static str; FEATURES.len()],
    pub app: AppInfo,
    pub windows: Vec<WindowInfo>,
    pub anchors: Vec<AnchorInfo>,
    pub commands: Vec<StageCommand>,
    pub hotkeys: Vec<StageHotkey>,
    pub state: Map<String, Value>,
}

#[derive(Serialize)]
struct StateResult {
    state: Map<String, Value>,
}

#[derive(Serialize)]
struct WindowResult {
    window: WindowInfo,
}

pub(super) fn not_open(page: StagePage) -> AutomationError {
    AutomationError::new(
        409,
        format!("The {} window isn't open; show it first.", page.as_str()),
    )
}

pub(super) fn object(result: impl Serialize) -> StageResult<Map<String, Value>> {
    match serde_json::to_value(result) {
        Ok(Value::Object(out)) => Ok(out),
        Ok(_) => Err(AutomationError::new(500, "The answer isn't an object.")),
        Err(e) => Err(AutomationError::new(500, e.to_string())),
    }
}

impl<H: StageHost> Hub<H> {
    /// `/v1/stage/<verb>`; the result is sent as `{ ok: true, ...result }`.
    pub async fn run(&self, verb: &str, params: &Params) -> StageResult<Map<String, Value>> {
        match verb {
            "describe" => object(self.describe().await),
            "state" => object(StateResult {
                state: self.host.state().await,
            }),
            "window" => object(WindowResult {
                window: self.window(params).await?,
            }),
            "act" => self.act(params).await,
            _ => Err(AutomationError::new(
                404,
                "Use /v1/stage/describe, window, act or state.",
            )),
        }
    }

    pub(super) fn describe_window(&self, page: StagePage) -> WindowInfo {
        let frame = self.host.frame(page);
        let meta = self.meta(page);
        window_info(
            page,
            WindowState {
                frame: frame.as_ref(),
                shown: self.host.is_shown(page),
                meta: meta.as_ref(),
                panel_view: (page == StagePage::Panel)
                    .then(|| self.host.panel_view())
                    .flatten(),
            },
        )
    }

    pub async fn describe(&self) -> Describe {
        let anchors = self.collect().await;
        Describe {
            protocol: PROTOCOL,
            features: FEATURES,
            app: self.app.clone(),
            windows: StagePage::ALL
                .into_iter()
                .map(|page| self.describe_window(page))
                .collect(),
            anchors,
            commands: self.host.commands(),
            hotkeys: stage_hotkeys(&self.host.hotkeys()),
            state: self.host.state().await,
        }
    }

    async fn window(&self, params: &Params) -> StageResult<WindowInfo> {
        let request = WindowRequest::parse(params)?;
        let page = request.page;
        match request.action {
            WindowAction::Show => self.show(page, request.section.as_deref())?,
            WindowAction::Hide => self.host.hide(page),
            WindowAction::Focus => self.focus(page)?,
            WindowAction::Place => self.place(&request)?,
        }
        tokio::time::sleep(self.timeouts.settle).await;
        // Fresh view, section and panels from the pages.
        self.collect().await;
        Ok(self.describe_window(page))
    }

    fn show(&self, page: StagePage, section: Option<&str>) -> StageResult<()> {
        if page == StagePage::Panel {
            return Err(AutomationError::new(
                409,
                "The overlay opens with a command, e.g. act kind=command command=palette:open.",
            ));
        }
        self.host.show(page, section);
        Ok(())
    }

    pub(super) fn focus(&self, page: StagePage) -> StageResult<()> {
        if page == StagePage::Rail {
            return Err(AutomationError::new(409, "The rail never takes focus."));
        }
        if !self.host.is_shown(page) {
            return Err(not_open(page));
        }
        self.host.focus(page);
        Ok(())
    }

    fn place(&self, request: &WindowRequest) -> StageResult<()> {
        let page = request.page;
        if !page.movable() {
            return Err(AutomationError::new(
                409,
                format!("The {} window places itself.", page.as_str()),
            ));
        }
        if request.bounds.is_empty() {
            return Err(AutomationError::bad(
                "Give x, y, width or height to place the window.",
            ));
        }
        let frame = self
            .host
            .frame(page)
            .filter(|_| self.host.is_shown(page))
            .ok_or_else(|| not_open(page))?;
        self.host.place(page, request.bounds.over(frame.bounds));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::stage::page::{PageMeta, PanelState};
    use crate::stage::test_host::{Call, FakeHost, anchor, hub, map, params, rect};
    use crate::stage::windows::sections;
    use crate::store::settings::Hotkeys;

    async fn run(
        hub: &Hub<FakeHost>,
        verb: &str,
        items: &[(&str, &str)],
    ) -> Result<Map<String, Value>, AutomationError> {
        hub.run(verb, &params(items)).await
    }

    async fn window(hub: &Hub<FakeHost>, items: &[(&str, &str)]) -> Value {
        let out = run(hub, "window", items).await.expect("window answers");
        out["window"].clone()
    }

    async fn window_error(hub: &Hub<FakeHost>, items: &[(&str, &str)]) -> AutomationError {
        run(hub, "window", items).await.expect_err("window refuses")
    }

    fn json_of(window: &WindowInfo) -> Value {
        serde_json::to_value(window).expect("window serializes")
    }

    #[tokio::test]
    async fn describe_lists_the_five_windows_in_order() {
        let hub = hub();

        let describe = hub.describe().await;

        let ids: Vec<StagePage> = describe.windows.iter().map(|w| w.id).collect();
        assert_eq!(
            ids,
            [
                StagePage::Hub,
                StagePage::Settings,
                StagePage::Panel,
                StagePage::Rail,
                StagePage::QuickNotes
            ]
        );
    }

    #[tokio::test]
    async fn describe_announces_the_protocol_the_features_and_the_app() {
        let hub = hub();

        let described = serde_json::to_value(hub.describe().await).unwrap();

        assert_eq!(described["protocol"], 1);
        assert_eq!(
            described["features"],
            json!([
                "reveal",
                "visible",
                "sections",
                "ui",
                "type.append",
                "keys.window",
                "scroll",
                "wait"
            ])
        );
        assert_eq!(
            described["app"],
            json!({"name": "Daybook", "id": "test.daybook", "version": "1.2.3"})
        );
    }

    #[tokio::test]
    async fn describe_takes_commands_hotkeys_and_state_from_the_app() {
        let hub = hub();
        let command = StageCommand {
            id: "journal:open".into(),
            help: "Opens the journal".into(),
            shortcut: Some("cmd+j".into()),
            enabled: true,
            arg: None,
            choices: None,
        };
        hub.host.set_commands(vec![command.clone()]);
        hub.host.set_hotkeys(Hotkeys {
            journal: "ctrl+alt+cmd+j".into(),
            ..Hotkeys::none()
        });
        hub.host.set_state(map(json!({"status": "working"})));

        let describe = hub.describe().await;

        assert_eq!(describe.commands, vec![command]);
        assert_eq!(
            serde_json::to_value(&describe.hotkeys).unwrap(),
            json!([{"action": "journal", "shortcut": "ctrl+alt+cmd+j", "command": "journal:open"}])
        );
        assert_eq!(describe.state, map(json!({"status": "working"})));
    }

    #[tokio::test]
    async fn a_hidden_window_is_described_without_bounds_view_or_anchors() {
        let hub = hub();
        hub.host.hide_window(StagePage::Hub);
        hub.host
            .page_shows(StagePage::Hub, vec![anchor("tool:search", 0.0, 0.0)]);
        hub.host.page_reports(
            StagePage::Hub,
            PageMeta {
                view: Some("today".into()),
                ..PageMeta::default()
            },
        );

        let describe = hub.describe().await;

        assert_eq!(
            json_of(&describe.windows[0]),
            json!({
                "id": "hub",
                "title": "Daybook",
                "visible": false,
                "focused": false,
                "movable": true,
                "bounds": null,
                "content": null,
                "view": null
            })
        );
        assert!(describe.anchors.is_empty());
    }

    #[tokio::test]
    async fn a_shown_page_that_stays_silent_is_still_described() {
        let hub = hub();
        hub.host.open_at(StagePage::Hub, rect(10, 20, 300, 200));

        let describe = hub.describe().await;

        assert_eq!(
            json_of(&describe.windows[0]),
            json!({
                "id": "hub",
                "title": "Daybook",
                "visible": true,
                "focused": false,
                "movable": true,
                "bounds": {"x": 10, "y": 20, "width": 300, "height": 200},
                "content": {"x": 10, "y": 20, "width": 300, "height": 200},
                "view": null
            })
        );
        assert!(describe.anchors.is_empty());
    }

    #[tokio::test]
    async fn the_hub_view_comes_from_the_pages_meta() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host.page_reports(
            StagePage::Hub,
            PageMeta {
                view: Some("day:2026-10-16".into()),
                ..PageMeta::default()
            },
        );

        let describe = hub.describe().await;

        assert_eq!(describe.windows[0].view.as_deref(), Some("day:2026-10-16"));
    }

    #[tokio::test]
    async fn the_overlays_view_comes_from_the_app() {
        let hub = hub();
        hub.host.open(StagePage::Panel);
        hub.host.set_panel_view("palette");

        let describe = hub.describe().await;

        assert_eq!(describe.windows[2].view.as_deref(), Some("palette"));
    }

    #[tokio::test]
    async fn settings_lists_its_sections_and_has_a_null_section_while_hidden() {
        let hub = hub();
        hub.host.hide_window(StagePage::Settings);
        hub.host.page_reports(
            StagePage::Settings,
            PageMeta {
                section: Some(sections()[0].id.into()),
                ..PageMeta::default()
            },
        );

        let describe = hub.describe().await;

        let settings = json_of(&describe.windows[1]);
        assert_eq!(
            settings["sections"],
            serde_json::to_value(sections()).unwrap()
        );
        assert_eq!(settings.get("section"), Some(&Value::Null));
    }

    #[tokio::test]
    async fn a_shown_settings_window_reports_the_section_its_page_says() {
        let hub = hub();
        let section = sections()[1].id;
        hub.host.open(StagePage::Settings);
        hub.host.page_reports(
            StagePage::Settings,
            PageMeta {
                section: Some(section.into()),
                ..PageMeta::default()
            },
        );

        let describe = hub.describe().await;

        assert_eq!(json_of(&describe.windows[1])["section"], section);
    }

    #[tokio::test]
    async fn the_ui_of_a_hidden_window_is_kept() {
        let hub = hub();
        hub.host.hide_window(StagePage::Hub);
        hub.host.page_reports(
            StagePage::Hub,
            PageMeta {
                ui: Some([("inspector".to_string(), PanelState::Open)].into()),
                ..PageMeta::default()
            },
        );

        let describe = hub.describe().await;

        assert_eq!(
            json_of(&describe.windows[0])["ui"],
            json!({"inspector": "open"})
        );
    }

    #[tokio::test]
    async fn state_answers_the_apps_state() {
        let hub = hub();
        hub.host
            .set_state(map(json!({"status": "break", "vault": true})));

        let out = run(&hub, "state", &[]).await.unwrap();

        assert_eq!(
            Value::Object(out),
            json!({"state": {"status": "break", "vault": true}})
        );
    }

    #[tokio::test]
    async fn an_unknown_verb_is_404() {
        let hub = hub();

        let err = run(&hub, "dance", &[]).await.unwrap_err();

        assert_eq!(
            err,
            AutomationError::new(404, "Use /v1/stage/describe, window, act or state.")
        );
    }

    #[tokio::test]
    async fn showing_a_window_answers_its_description() {
        let hub = hub();

        let shown = window(&hub, &[("window", "quick-notes")]).await;

        assert_eq!(hub.host.calls(), [Call::Show(StagePage::QuickNotes, None)]);
        assert_eq!(shown["id"], "quick-notes");
        assert_eq!(shown["visible"], true);
    }

    #[tokio::test]
    async fn showing_the_settings_window_passes_the_section() {
        let hub = hub();
        let section = sections()[1].id;

        window(&hub, &[("window", "settings"), ("section", section)]).await;

        assert_eq!(
            hub.host.calls(),
            [Call::Show(StagePage::Settings, Some(section.to_string()))]
        );
    }

    #[tokio::test]
    async fn the_answer_carries_what_the_page_reports_after_the_action() {
        let hub = hub();
        let section = sections()[1].id;
        hub.host.page_reports(
            StagePage::Settings,
            PageMeta {
                section: Some(section.into()),
                ..PageMeta::default()
            },
        );

        let shown = window(&hub, &[("window", "settings")]).await;

        assert_eq!(shown["section"], section);
    }

    #[tokio::test]
    async fn hiding_a_window_answers_it_as_not_visible() {
        let hub = hub();
        hub.host.open(StagePage::Hub);

        let hidden = window(&hub, &[("window", "hub"), ("action", "hide")]).await;

        assert_eq!(hub.host.calls(), [Call::Hide(StagePage::Hub)]);
        assert_eq!(hidden["visible"], false);
        assert_eq!(hidden["bounds"], Value::Null);
    }

    #[tokio::test]
    async fn focusing_an_open_window_focuses_it() {
        let hub = hub();
        hub.host.open(StagePage::Settings);

        let focused = window(&hub, &[("window", "settings"), ("action", "focus")]).await;

        assert_eq!(hub.host.calls(), [Call::Focus(StagePage::Settings)]);
        assert_eq!(focused["focused"], true);
    }

    #[tokio::test]
    async fn bounds_without_an_action_place_the_window_and_keep_the_rest() {
        let hub = hub();
        hub.host.open_at(StagePage::Hub, rect(100, 50, 800, 600));

        let placed = window(&hub, &[("window", "hub"), ("x", "10"), ("width", "500")]).await;

        assert_eq!(
            hub.host.calls(),
            [Call::Place(StagePage::Hub, rect(10, 50, 500, 600))]
        );
        assert_eq!(
            placed["bounds"],
            json!({"x": 10, "y": 50, "width": 500, "height": 600})
        );
    }

    #[tokio::test]
    async fn the_overlay_cannot_be_shown_by_the_window_verb() {
        let hub = hub();

        let err = window_error(&hub, &[("window", "panel")]).await;

        assert_eq!(
            err,
            AutomationError::new(
                409,
                "The overlay opens with a command, e.g. act kind=command command=palette:open."
            )
        );
        assert!(hub.host.calls().is_empty());
    }

    #[tokio::test]
    async fn the_rail_never_takes_focus() {
        let hub = hub();
        hub.host.open(StagePage::Rail);

        let err = window_error(&hub, &[("window", "rail"), ("action", "focus")]).await;

        assert_eq!(
            err,
            AutomationError::new(409, "The rail never takes focus.")
        );
    }

    #[tokio::test]
    async fn a_window_that_places_itself_refuses_to_be_placed() {
        let hub = hub();
        for page in ["panel", "rail"] {
            let err = window_error(&hub, &[("window", page), ("x", "1")]).await;

            assert_eq!(
                err,
                AutomationError::new(409, format!("The {page} window places itself.")),
                "{page}"
            );
        }
    }

    #[tokio::test]
    async fn placing_without_bounds_is_400() {
        let hub = hub();
        hub.host.open(StagePage::Hub);

        let err = window_error(&hub, &[("window", "hub"), ("action", "place")]).await;

        assert_eq!(
            err,
            AutomationError::new(400, "Give x, y, width or height to place the window.")
        );
    }

    #[tokio::test]
    async fn focusing_or_placing_a_closed_window_is_409() {
        let hub = hub();
        hub.host.hide_window(StagePage::Hub);
        let cases: [(&str, &[(&str, &str)]); 4] = [
            ("hub", &[("action", "focus")]),
            ("hub", &[("action", "place"), ("x", "5")]),
            ("quick-notes", &[("action", "focus")]),
            ("quick-notes", &[("x", "5")]),
        ];
        for (page, extra) in cases {
            let mut items = vec![("window", page)];
            items.extend_from_slice(extra);

            let err = window_error(&hub, &items).await;

            assert_eq!(
                err,
                AutomationError::new(409, format!("The {page} window isn't open; show it first.")),
                "{page} {extra:?}"
            );
        }
        assert!(hub.host.calls().is_empty());
    }

    #[tokio::test]
    async fn window_params_are_checked_before_the_window_is_touched() {
        let hub = hub();
        hub.host.open(StagePage::Hub);

        let err = window_error(&hub, &[("window", "hub"), ("action", "dance")]).await;

        assert_eq!(
            err,
            AutomationError::new(400, "“action” must be show, hide, focus or place.")
        );
        assert!(hub.host.calls().is_empty());
    }
}
