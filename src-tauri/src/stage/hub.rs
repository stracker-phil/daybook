//! The hub between the HTTP API and the pages: it asks the pages with an event under a fresh request
//! id and gathers their replies under a timeout. `collect` waits for every page on screen; `dispatch`
//! takes the first page that handled the act. The verbs are in `verbs.rs` and `acts.rs`.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;

use serde::Serialize;
use serde_json::{Map, Value};

use super::catalog::{CommandStatus, StageCommand};
use super::inbox::Inbox;
use super::page::{ActEvent, ActReply, ActRequest, AnchorsReply, CollectEvent, PageMeta};
use super::protocol::ScreenRect;
use super::windows::{AnchorInfo, Frame, StagePage, sort_anchors, to_screen_anchors};
use crate::automation::AutomationError;
use crate::lock::MutexExt;
use crate::store::settings::Hotkeys;

pub const COLLECT_EVENT: &str = "stage:collect";
pub const ACT_EVENT: &str = "stage:act";

/// What the hub needs from the app: its windows, a way to reach the pages, the commands and the state.
pub trait StageHost: Send + Sync + 'static {
    /// None while the window doesn't exist.
    fn frame(&self, page: StagePage) -> Option<Frame>;
    /// On screen as the app means it: the overlay open, the rail slid in, a window visible.
    fn is_shown(&self, page: StagePage) -> bool;
    /// The open overlay's view.
    fn panel_view(&self) -> Option<String>;
    fn show(&self, page: StagePage, section: Option<&str>);
    fn hide(&self, page: StagePage);
    fn focus(&self, page: StagePage);
    fn place(&self, page: StagePage, rect: ScreenRect);
    fn emit(&self, pages: &[StagePage], event: &str, payload: Value);
    fn commands(&self) -> Vec<StageCommand>;
    fn command_status(&self, id: &str) -> CommandStatus;
    fn run_command(&self, id: &str) -> impl Future<Output = Result<(), AutomationError>> + Send;
    /// The global shortcuts as configured, registered or not (ADR 0018).
    fn hotkeys(&self) -> Hotkeys;
    /// The narrator's state, flat.
    fn state(&self) -> impl Future<Output = Map<String, Value>> + Send;
}

#[derive(Debug, Clone, Copy)]
pub struct Timeouts {
    /// Pages on screen answer at once; one that doesn't (still loading) is left out.
    pub collect: Duration,
    /// A page may first scroll the anchor into view, smoothly.
    pub act: Duration,
    /// A window needs a moment to appear, move or take focus before it is measured or sent keys.
    pub settle: Duration,
    /// After a command the pages get a frame to render, so an immediate `describe` sees the result.
    pub after_command: Duration,
    pub wait_poll: Duration,
}

impl Default for Timeouts {
    fn default() -> Self {
        Self {
            collect: Duration::from_millis(400),
            act: Duration::from_millis(2500),
            settle: Duration::from_millis(150),
            after_command: Duration::from_millis(50),
            wait_poll: Duration::from_millis(100),
        }
    }
}

/// The app as `describe` names it.
#[derive(Serialize, Debug, Clone, PartialEq)]
pub struct AppInfo {
    pub name: String,
    pub id: String,
    pub version: String,
}

pub struct Hub<H> {
    pub(super) host: H,
    pub(super) timeouts: Timeouts,
    pub(super) app: AppInfo,
    anchors: Inbox<AnchorsReply>,
    acts: Inbox<ActReply>,
    /// What each page last said about itself, also while hidden.
    meta: Mutex<HashMap<StagePage, PageMeta>>,
    last_request: Mutex<Option<chrono::DateTime<chrono::Utc>>>,
}

fn payload(event: impl Serialize) -> Value {
    serde_json::to_value(event).unwrap_or(Value::Null)
}

impl<H: StageHost> Hub<H> {
    pub fn new(host: H, timeouts: Timeouts, app: AppInfo) -> Self {
        Self {
            host,
            timeouts,
            app,
            anchors: Inbox::default(),
            acts: Inbox::default(),
            meta: Mutex::default(),
            last_request: Mutex::default(),
        }
    }

    /// When a presenter last used the hook (Settings → Automation); memory only.
    pub fn last_request(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        *self.last_request.lock_unpoisoned()
    }

    /// Every presenter request counts, failed ones too.
    pub fn touch(&self) {
        *self.last_request.lock_unpoisoned() = Some(chrono::Utc::now());
    }

    /// A page's meta is kept even when its request is over; late anchors are dropped.
    pub fn receive_anchors(&self, reply: AnchorsReply) {
        let reply = reply.cleaned();
        if let Some(meta) = &reply.meta {
            self.meta.lock_unpoisoned().insert(reply.page, meta.clone());
        }
        let request = reply.request.clone();
        self.anchors.deliver(&request, reply);
    }

    pub fn receive_act(&self, reply: ActReply) {
        let request = reply.request.clone();
        self.acts.deliver(&request, reply);
    }

    pub(super) fn meta(&self, page: StagePage) -> Option<PageMeta> {
        self.meta.lock_unpoisoned().get(&page).cloned()
    }

    pub(super) fn shown_pages(&self) -> Vec<StagePage> {
        StagePage::ALL
            .into_iter()
            .filter(|page| self.host.is_shown(*page))
            .collect()
    }

    /// Asks every page for its anchors and what it shows; waits for the pages on screen and keeps
    /// only their anchors, in screen points and window, then reading order.
    pub(super) async fn collect(&self) -> Vec<AnchorInfo> {
        let contents: HashMap<StagePage, ScreenRect> = self
            .shown_pages()
            .into_iter()
            .filter_map(|page| Some((page, self.host.frame(page)?.content)))
            .collect();
        let mut waiting: HashSet<StagePage> = contents.keys().copied().collect();
        let mut ticket = self.anchors.open();
        let event = CollectEvent {
            request: ticket.id.clone(),
        };
        self.host
            .emit(&StagePage::ALL, COLLECT_EVENT, payload(event));
        let deadline = tokio::time::Instant::now() + self.timeouts.collect;
        let mut anchors = Vec::new();
        while !waiting.is_empty() {
            let Ok(Some(reply)) = tokio::time::timeout_at(deadline, ticket.replies.recv()).await
            else {
                break;
            };
            if waiting.remove(&reply.page)
                && let Some(content) = contents.get(&reply.page)
            {
                anchors.extend(to_screen_anchors(reply.page, reply.anchors, *content));
            }
        }
        sort_anchors(&mut anchors);
        anchors
    }

    /// Sends the act to `pages`; the first that handled it answers. None: no page could (none shows
    /// the anchor, or none answered in time).
    pub(super) async fn dispatch(&self, act: ActRequest, pages: &[StagePage]) -> Option<ActReply> {
        if pages.is_empty() {
            return None;
        }
        let mut waiting: HashSet<StagePage> = pages.iter().copied().collect();
        let mut ticket = self.acts.open();
        let event = ActEvent {
            request: ticket.id.clone(),
            act,
        };
        self.host.emit(pages, ACT_EVENT, payload(event));
        let deadline = tokio::time::Instant::now() + self.timeouts.act;
        while !waiting.is_empty() {
            let Ok(Some(reply)) = tokio::time::timeout_at(deadline, ticket.replies.recv()).await
            else {
                return None;
            };
            if !waiting.contains(&reply.page) {
                continue;
            }
            if reply.handled {
                return Some(reply);
            }
            waiting.remove(&reply.page);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::stage::test_host::{
        anchor, declined, handled, hub, hub_with, page_rect, quick, rect,
    };

    fn reveal(anchor: &str) -> ActRequest {
        ActRequest::Reveal {
            anchor: anchor.into(),
        }
    }

    #[tokio::test]
    async fn anchors_come_by_window_then_top_to_bottom_then_left_to_right() {
        let hub = hub();
        hub.host.open(StagePage::Settings);
        hub.host.open(StagePage::Hub);
        hub.host.page_shows(
            StagePage::Hub,
            vec![
                anchor("low", 0.0, 30.0),
                anchor("right", 40.0, 10.0),
                anchor("left", 0.0, 10.0),
            ],
        );
        hub.host
            .page_shows(StagePage::Settings, vec![anchor("settings", 0.0, 0.0)]);

        let ids: Vec<String> = hub.collect().await.into_iter().map(|a| a.id).collect();

        assert_eq!(ids, ["left", "right", "low", "settings"]);
    }

    #[tokio::test]
    async fn anchors_are_moved_to_the_windows_content_origin_and_rounded() {
        let hub = hub();
        hub.host.open_at(StagePage::Hub, rect(120, 88, 1240, 692));
        let tool = crate::stage::page::PageAnchor {
            rect: page_rect(16.4, 100.6, 208.2, 32.0),
            visible: Some(page_rect(16.4, 100.6, 208.2, 20.0)),
            ..anchor("tool:activity", 0.0, 0.0)
        };
        hub.host.page_shows(StagePage::Hub, vec![tool]);

        let anchors = hub.collect().await;

        assert_eq!(anchors.len(), 1);
        assert_eq!(anchors[0].window, StagePage::Hub);
        assert_eq!(anchors[0].rect, rect(136, 189, 208, 32));
        assert_eq!(anchors[0].visible, Some(rect(136, 189, 208, 20)));
    }

    #[tokio::test]
    async fn every_page_is_asked_for_its_anchors_also_the_hidden_ones() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host.page_shows(StagePage::Hub, vec![]);

        hub.collect().await;

        let sent = hub.host.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].event, "stage:collect");
        assert_eq!(sent[0].pages, StagePage::ALL);
    }

    #[tokio::test]
    async fn a_hidden_pages_anchors_are_dropped_but_its_meta_is_kept() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        hub.host
            .page_shows(StagePage::Hub, vec![anchor("shown", 0.0, 0.0)]);
        hub.host.hide_window(StagePage::Settings);
        hub.host
            .page_shows(StagePage::Settings, vec![anchor("hidden", 0.0, 0.0)]);
        let meta = PageMeta {
            section: Some("vault".into()),
            ..PageMeta::default()
        };
        hub.host.page_reports(StagePage::Settings, meta.clone());

        let ids: Vec<String> = hub.collect().await.into_iter().map(|a| a.id).collect();

        assert_eq!(ids, ["shown"]);
        assert_eq!(hub.meta(StagePage::Settings), Some(meta));
    }

    #[tokio::test]
    async fn collect_waits_for_a_silent_page_on_screen_up_to_the_timeout() {
        let timeouts = Timeouts {
            collect: Duration::from_millis(150),
            ..quick()
        };
        let hub = hub_with(timeouts);
        hub.host.open(StagePage::Hub);
        hub.host.open(StagePage::Settings);
        hub.host
            .page_shows(StagePage::Hub, vec![anchor("answered", 0.0, 0.0)]);

        let started = Instant::now();
        let ids: Vec<String> = hub.collect().await.into_iter().map(|a| a.id).collect();

        assert!(started.elapsed() >= Duration::from_millis(150));
        assert_eq!(ids, ["answered"]);
    }

    #[tokio::test]
    async fn collect_does_not_wait_when_no_page_is_shown() {
        let timeouts = Timeouts {
            collect: Duration::from_millis(600),
            ..quick()
        };
        let hub = hub_with(timeouts);
        hub.host.hide_window(StagePage::Hub);

        let started = Instant::now();
        let anchors = hub.collect().await;

        assert!(anchors.is_empty());
        assert!(started.elapsed() < Duration::from_millis(300));
    }

    #[tokio::test]
    async fn collect_returns_once_every_page_on_screen_has_answered() {
        let timeouts = Timeouts {
            collect: Duration::from_millis(600),
            ..quick()
        };
        let hub = hub_with(timeouts);
        hub.host.open(StagePage::Hub);
        hub.host.open(StagePage::Panel);
        hub.host.page_shows(StagePage::Hub, vec![]);
        hub.host.page_shows(StagePage::Panel, vec![]);

        let started = Instant::now();
        hub.collect().await;

        assert!(started.elapsed() < Duration::from_millis(300));
    }

    #[tokio::test]
    async fn a_late_reply_is_dropped_but_its_meta_is_stored() {
        let hub = hub();
        hub.host.open(StagePage::Hub);
        assert!(hub.collect().await.is_empty());
        let request = hub.host.sent()[0].payload["request"]
            .as_str()
            .expect("a request id")
            .to_string();
        let meta = PageMeta {
            view: Some("today".into()),
            ..PageMeta::default()
        };

        hub.receive_anchors(AnchorsReply {
            request,
            page: StagePage::Hub,
            anchors: vec![anchor("late", 0.0, 0.0)],
            meta: Some(meta.clone()),
        });

        assert_eq!(hub.meta(StagePage::Hub), Some(meta));
        assert!(hub.collect().await.is_empty());
    }

    #[test]
    fn a_request_is_remembered_as_the_last_one() {
        let hub = hub();
        assert_eq!(hub.last_request(), None);

        hub.touch();

        assert!(hub.last_request().is_some());
    }

    #[tokio::test]
    async fn dispatch_to_no_page_answers_none_without_asking_anyone() {
        let hub = hub();

        assert_eq!(hub.dispatch(reveal("a"), &[]).await, None);
        assert!(hub.host.sent().is_empty());
    }

    #[tokio::test]
    async fn dispatch_answers_none_when_the_page_does_not_reply_in_time() {
        let hub = hub();
        hub.host.open(StagePage::Hub);

        assert_eq!(hub.dispatch(reveal("a"), &[StagePage::Hub]).await, None);
    }

    #[tokio::test]
    async fn dispatch_gives_up_at_once_when_every_page_declined() {
        let timeouts = Timeouts {
            act: Duration::from_millis(600),
            ..quick()
        };
        let hub = hub_with(timeouts);
        hub.host.page_answers(StagePage::Hub, declined());
        hub.host.page_answers(StagePage::Settings, declined());

        let started = Instant::now();
        let reply = hub
            .dispatch(reveal("a"), &[StagePage::Hub, StagePage::Settings])
            .await;

        assert_eq!(reply, None);
        assert!(started.elapsed() < Duration::from_millis(300));
    }

    #[tokio::test]
    async fn dispatch_ignores_a_reply_from_a_page_it_did_not_ask() {
        let hub = hub();
        hub.host.page_answers_unasked(StagePage::Rail, handled());
        hub.host.page_answers(StagePage::Hub, handled());

        let reply = hub.dispatch(reveal("a"), &[StagePage::Hub]).await;

        assert_eq!(reply.map(|r| r.page), Some(StagePage::Hub));
    }
}
