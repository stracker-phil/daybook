//! Quick access links (Settings → Links): which show now (`rules`), the `links` overlay and opening one.
//! Today's events come from the calendar, which is async, so they are kept here and the split itself
//! is computed whenever someone asks.

pub mod rules;

use std::sync::Mutex;

use chrono::{Local, NaiveDate};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use self::rules::{EventSpan, QuickAccess, quick_access};
use crate::bus::{ArgChoice, Command, CommandBus};
use crate::error::AppResult;
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::tools::{Link, link_argument, link_command, link_target};
use crate::{calendar, timetracker, vault, window};

const VIEW: &str = "links";
const PREFIX: &str = "link:";

/// Today's events, as the event rules read them.
#[derive(Default)]
pub struct Links {
    events: Mutex<Option<(NaiveDate, Vec<EventSpan>)>>,
}

impl Links {
    fn events_on(&self, day: NaiveDate) -> Vec<EventSpan> {
        match &*self.events.lock_unpoisoned() {
            Some((cached, events)) if *cached == day => events.clone(),
            _ => Vec::new(),
        }
    }
}

/// Reads today's events again (the calendar caches them).
pub async fn refresh(app: &AppHandle) {
    let today = app.state::<AppState>().notes.today();
    let events = calendar::events_on(app, today, today).await;
    let spans = events.iter().map(EventSpan::from).collect();
    *app.state::<AppState>().links.events.lock_unpoisoned() = Some((today, spans));
}

pub fn refresh_soon(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move { refresh(&app).await });
}

/// The links that show now and the rest, with the events read last.
pub fn current(app: &AppHandle) -> QuickAccess {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let context = state.contexts.current(&settings).map(|c| c.id.clone());
    let events = state.links.events_on(state.notes.today());
    quick_access(
        Local::now().naive_local(),
        &settings.links,
        context.as_deref(),
        &events,
        timetracker::current_project(app).as_deref(),
    )
}

/// A link of the overlay; `command` opens it.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "LinkItem")]
pub struct LinkRow {
    pub title: String,
    pub target: String,
    pub command: String,
}

/// The `links` overlay's lists.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "QuickAccess")]
pub struct LinksView {
    pub now: Vec<LinkRow>,
    pub more: Vec<LinkRow>,
}

fn rows(links: Vec<Link>) -> Vec<LinkRow> {
    links
        .into_iter()
        .map(|link| LinkRow {
            command: link_command(&link.target),
            title: link.title,
            target: link.target,
        })
        .collect()
}

#[tauri::command]
pub async fn links_view(app: AppHandle) -> LinksView {
    refresh(&app).await;
    let QuickAccess { now, more } = current(&app);
    LinksView {
        now: rows(now),
        more: rows(more),
    }
}

/// Opens `target` when it is a configured link; anything else is ignored.
fn open(app: &AppHandle, target: &str) -> AppResult<()> {
    let settings = app.state::<AppState>().settings.get();
    if !settings.links.iter().any(|link| link.target == target) {
        log::info!("Not opening {target}: no link has it");
        return Ok(());
    }
    window::panel::close(app, true);
    vault::commands::open_target(app, target)
}

fn link_choices(app: &AppHandle) -> Vec<ArgChoice> {
    let QuickAccess { now, more } = current(app);
    now.into_iter()
        .chain(more)
        .map(|link| ArgChoice {
            word: link.title.clone(),
            title: Some(link.title),
            argument: link_argument(&link.target),
        })
        .collect()
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(Command::new("links:open", "Links…").run(|app| async move {
        window::panel::open(&app, VIEW, Value::Null).await;
        Ok(())
    }));
    bus.add_family(
        Command::new(PREFIX, "Open a link from Settings → Links")
            .family("<link>")
            .choices(link_choices)
            .run(|app, argument| async move { open(&app, &link_target(&argument)) }),
    );
}
