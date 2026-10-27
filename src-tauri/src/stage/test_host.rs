//! A fake app for the hub's tests: windows that can be opened and moved, pages that answer
//! `stage:collect` and `stage:act` the moment they are asked, a command table and the shortcuts.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use serde_json::{Map, Value};

use super::catalog::{CommandStatus, StageCommand};
use super::hub::{ACT_EVENT, AppInfo, COLLECT_EVENT, Hub, StageHost, Timeouts};
use super::page::{ActEvent, ActReply, AnchorsReply, CollectEvent, PageAnchor, PageMeta};
use super::protocol::{PageRect, ScreenRect};
use super::windows::{Frame, StagePage};
use crate::automation::{AutomationError, Params};
use crate::lock::MutexExt;
use crate::store::settings::Hotkeys;

/// What the hub asked the host to do with a window.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    Show(StagePage, Option<String>),
    Hide(StagePage),
    Focus(StagePage),
    Place(StagePage, ScreenRect),
}

/// An event the hub sent to the pages.
#[derive(Debug, Clone)]
pub struct Sent {
    pub pages: Vec<StagePage>,
    pub event: String,
    pub payload: Value,
}

struct State {
    frames: HashMap<StagePage, Frame>,
    shown: HashSet<StagePage>,
    panel_view: Option<String>,
    anchors: HashMap<StagePage, Vec<PageAnchor>>,
    meta: HashMap<StagePage, PageMeta>,
    answers: HashMap<StagePage, ActReply>,
    strays: Vec<StagePage>,
    statuses: HashMap<String, CommandStatus>,
    commands: Vec<StageCommand>,
    hotkeys: Hotkeys,
    state: Map<String, Value>,
    calls: Vec<Call>,
    ran: Vec<String>,
    sent: Vec<Sent>,
}

pub struct FakeHost {
    state: Mutex<State>,
    hub: OnceLock<Weak<Hub<FakeHost>>>,
}

pub fn rect(x: i64, y: i64, width: i64, height: i64) -> ScreenRect {
    ScreenRect {
        x,
        y,
        width,
        height,
    }
}

pub fn page_rect(x: f64, y: f64, width: f64, height: f64) -> PageRect {
    PageRect {
        x,
        y,
        width,
        height,
    }
}

/// A 100 x 20 anchor at (x, y) in its page.
pub fn anchor(id: &str, x: f64, y: f64) -> PageAnchor {
    PageAnchor {
        id: id.into(),
        label: id.into(),
        rect: page_rect(x, y, 100.0, 20.0),
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

/// A page that took the act and did it.
pub fn handled() -> ActReply {
    ActReply {
        handled: true,
        ok: true,
        ..ActReply::default()
    }
}

/// A page that doesn't show the anchor.
pub fn declined() -> ActReply {
    ActReply::default()
}

pub fn params(items: &[(&str, &str)]) -> Params {
    items
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect()
}

pub fn map(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("not an object: {other}"),
    }
}

/// Short timeouts, so a page that never answers costs milliseconds.
pub fn quick() -> Timeouts {
    Timeouts {
        collect: Duration::from_millis(40),
        act: Duration::from_millis(80),
        settle: Duration::from_millis(1),
        after_command: Duration::from_millis(1),
        wait_poll: Duration::from_millis(5),
    }
}

pub fn hub() -> Arc<Hub<FakeHost>> {
    hub_with(quick())
}

pub fn hub_with(timeouts: Timeouts) -> Arc<Hub<FakeHost>> {
    let app = AppInfo {
        name: "Daybook".into(),
        id: "test.daybook".into(),
        version: "1.2.3".into(),
    };
    let hub = Arc::new(Hub::new(FakeHost::new(), timeouts, app));
    let _ = hub.host.hub.set(Arc::downgrade(&hub));
    hub
}

fn frame_at(content: ScreenRect) -> Frame {
    Frame {
        title: String::new(),
        focused: false,
        bounds: content,
        content,
    }
}

impl FakeHost {
    fn new() -> Self {
        Self {
            state: Mutex::new(State {
                frames: HashMap::new(),
                shown: HashSet::new(),
                panel_view: None,
                anchors: HashMap::new(),
                meta: HashMap::new(),
                answers: HashMap::new(),
                strays: Vec::new(),
                statuses: HashMap::new(),
                commands: Vec::new(),
                hotkeys: Hotkeys::none(),
                state: Map::new(),
                calls: Vec::new(),
                ran: Vec::new(),
                sent: Vec::new(),
            }),
            hub: OnceLock::new(),
        }
    }

    /// Content origin (100, 80), 800 x 570.
    pub fn open(&self, page: StagePage) {
        self.open_at(page, rect(100, 80, 800, 570));
    }

    /// Shown, with this window area (the window has no frame of its own).
    pub fn open_at(&self, page: StagePage, content: ScreenRect) {
        let mut state = self.state.lock_unpoisoned();
        state.frames.insert(page, frame_at(content));
        state.shown.insert(page);
    }

    /// The window exists but isn't on screen.
    pub fn hide_window(&self, page: StagePage) {
        let mut state = self.state.lock_unpoisoned();
        state
            .frames
            .entry(page)
            .or_insert_with(|| frame_at(rect(100, 80, 800, 570)));
        state.shown.remove(&page);
    }

    pub fn set_focused(&self, page: StagePage) {
        if let Some(frame) = self.state.lock_unpoisoned().frames.get_mut(&page) {
            frame.focused = true;
        }
    }

    pub fn set_panel_view(&self, view: &str) {
        self.state.lock_unpoisoned().panel_view = Some(view.into());
    }

    /// The page answers `stage:collect` with these anchors.
    pub fn page_shows(&self, page: StagePage, anchors: Vec<PageAnchor>) {
        self.state.lock_unpoisoned().anchors.insert(page, anchors);
    }

    /// The page answers `stage:collect` with this meta.
    pub fn page_reports(&self, page: StagePage, meta: PageMeta) {
        self.state.lock_unpoisoned().meta.insert(page, meta);
    }

    /// The page answers `stage:act` this way.
    pub fn page_answers(&self, page: StagePage, reply: ActReply) {
        self.state
            .lock_unpoisoned()
            .answers
            .insert(page, ActReply { page, ..reply });
    }

    /// The page answers every `stage:act` this way, also when it wasn't asked.
    pub fn page_answers_unasked(&self, page: StagePage, reply: ActReply) {
        self.page_answers(page, reply);
        self.state.lock_unpoisoned().strays.push(page);
    }

    pub fn set_status(&self, command: &str, status: CommandStatus) {
        self.state
            .lock_unpoisoned()
            .statuses
            .insert(command.into(), status);
    }

    pub fn set_commands(&self, commands: Vec<StageCommand>) {
        self.state.lock_unpoisoned().commands = commands;
    }

    pub fn set_hotkeys(&self, hotkeys: Hotkeys) {
        self.state.lock_unpoisoned().hotkeys = hotkeys;
    }

    pub fn set_state(&self, state: Map<String, Value>) {
        self.state.lock_unpoisoned().state = state;
    }

    pub fn calls(&self) -> Vec<Call> {
        self.state.lock_unpoisoned().calls.clone()
    }

    /// The commands the hub ran, in order.
    pub fn ran(&self) -> Vec<String> {
        self.state.lock_unpoisoned().ran.clone()
    }

    pub fn sent(&self) -> Vec<Sent> {
        self.state.lock_unpoisoned().sent.clone()
    }

    /// The events sent as `stage:act`.
    pub fn acts_sent(&self) -> Vec<Sent> {
        self.sent()
            .into_iter()
            .filter(|sent| sent.event == ACT_EVENT)
            .collect()
    }

    /// The last `stage:act` payload without its request id.
    pub fn last_act(&self) -> Value {
        let mut payload = self.acts_sent().pop().expect("no act was sent").payload;
        if let Value::Object(object) = &mut payload {
            object.remove("request");
        }
        payload
    }
}

enum Reply {
    Anchors(AnchorsReply),
    Act(ActReply),
}

impl StageHost for FakeHost {
    fn frame(&self, page: StagePage) -> Option<Frame> {
        self.state.lock_unpoisoned().frames.get(&page).cloned()
    }

    fn is_shown(&self, page: StagePage) -> bool {
        self.state.lock_unpoisoned().shown.contains(&page)
    }

    fn panel_view(&self) -> Option<String> {
        self.state.lock_unpoisoned().panel_view.clone()
    }

    fn show(&self, page: StagePage, section: Option<&str>) {
        self.state
            .lock_unpoisoned()
            .calls
            .push(Call::Show(page, section.map(String::from)));
        if !self.is_shown(page) {
            self.open(page);
        }
    }

    fn hide(&self, page: StagePage) {
        let mut state = self.state.lock_unpoisoned();
        state.calls.push(Call::Hide(page));
        state.shown.remove(&page);
    }

    fn focus(&self, page: StagePage) {
        self.state.lock_unpoisoned().calls.push(Call::Focus(page));
        self.set_focused(page);
    }

    fn place(&self, page: StagePage, rect: ScreenRect) {
        let mut state = self.state.lock_unpoisoned();
        state.calls.push(Call::Place(page, rect));
        if let Some(frame) = state.frames.get_mut(&page) {
            frame.bounds = rect;
        }
    }

    fn emit(&self, pages: &[StagePage], event: &str, payload: Value) {
        let replies: Vec<Reply> = {
            let mut state = self.state.lock_unpoisoned();
            state.sent.push(Sent {
                pages: pages.to_vec(),
                event: event.into(),
                payload: payload.clone(),
            });
            match event {
                COLLECT_EVENT => {
                    let request = serde_json::from_value::<CollectEvent>(payload)
                        .expect("a collect event")
                        .request;
                    pages
                        .iter()
                        .filter(|page| {
                            state.anchors.contains_key(*page) || state.meta.contains_key(*page)
                        })
                        .map(|&page| {
                            Reply::Anchors(AnchorsReply {
                                request: request.clone(),
                                page,
                                anchors: state.anchors.get(&page).cloned().unwrap_or_default(),
                                meta: state.meta.get(&page).cloned(),
                            })
                        })
                        .collect()
                }
                ACT_EVENT => {
                    let request = serde_json::from_value::<ActEvent>(payload)
                        .expect("an act event")
                        .request;
                    state
                        .strays
                        .iter()
                        .chain(pages.iter())
                        .filter_map(|page| state.answers.get(page))
                        .map(|answer| {
                            Reply::Act(ActReply {
                                request: request.clone(),
                                ..answer.clone()
                            })
                        })
                        .collect()
                }
                _ => Vec::new(),
            }
        };
        let Some(hub) = self.hub.get().and_then(Weak::upgrade) else {
            return;
        };
        for reply in replies {
            match reply {
                Reply::Anchors(reply) => hub.receive_anchors(reply),
                Reply::Act(reply) => hub.receive_act(reply),
            }
        }
    }

    fn commands(&self) -> Vec<StageCommand> {
        self.state.lock_unpoisoned().commands.clone()
    }

    fn command_status(&self, id: &str) -> CommandStatus {
        self.state
            .lock_unpoisoned()
            .statuses
            .get(id)
            .copied()
            .unwrap_or(CommandStatus::Unknown)
    }

    fn run_command(&self, id: &str) -> impl Future<Output = Result<(), AutomationError>> + Send {
        self.state.lock_unpoisoned().ran.push(id.into());
        std::future::ready(Ok(()))
    }

    fn hotkeys(&self) -> Hotkeys {
        self.state.lock_unpoisoned().hotkeys.clone()
    }

    fn state(&self) -> impl Future<Output = Map<String, Value>> + Send {
        std::future::ready(self.state.lock_unpoisoned().state.clone())
    }
}
