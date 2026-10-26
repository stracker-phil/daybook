//! Vault search (docs/search.md): an index of every note and image, kept current by a watcher on the whole
//! vault; queries narrowed by scopes; a preview that follows links.

pub mod commands;
pub mod files;
mod index;
pub mod note;
pub mod posix;
pub mod preview;
pub mod query;
pub mod rank;
pub mod scopes;
pub mod uri;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use serde_json::json;
use tauri::{AppHandle, Manager};

use self::commands::{SearchResponse, SearchResult};
use self::files::is_hidden;
use self::index::SearchIndex;
use self::rank::Highlighter;
use crate::bus::{ArgChoice, Command, CommandBus};
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::vault::watch::FolderWatch;
use crate::window;

/// The overlay view key the page routes on.
pub const VIEW: &str = "search";

/// The vault being indexed and its watcher.
struct Followed {
    root: PathBuf,
    _watch: Option<FolderWatch>,
}

#[derive(Default)]
pub struct Search {
    index: Arc<SearchIndex>,
    watch: Mutex<Option<Followed>>,
}

impl Search {
    /// Indexes and watches `root` (canonical); `None` forgets the index. The same root again does nothing.
    pub fn follow(&self, root: Option<PathBuf>) {
        let mut followed = self.watch.lock_unpoisoned();
        if followed.as_ref().map(|f| f.root.as_path()) == root.as_deref() {
            return;
        }
        *followed = None;
        let Some(root) = root else {
            self.index.clear();
            return;
        };
        let watch = self.watch_vault(&root);
        let index = Arc::clone(&self.index);
        let building = root.clone();
        thread::spawn(move || index.build(&building));
        *followed = Some(Followed {
            root,
            _watch: watch,
        });
    }

    fn watch_vault(&self, root: &Path) -> Option<FolderWatch> {
        let index = Arc::clone(&self.index);
        let inside = root.to_path_buf();
        let accept = move |path: &Path| {
            path.strip_prefix(&inside)
                .is_ok_and(|relative| !is_hidden(relative))
        };
        FolderWatch::start(root, true, accept, move |paths| index.update(&paths))
            .map_err(|e| log::warn!("Can't watch {} for search: {e}", root.display()))
            .ok()
    }
}

/// Follows the vault in settings: at startup and after every settings save.
pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    let root = state
        .settings
        .get()
        .vault
        .path
        .and_then(|path| PathBuf::from(path).canonicalize().ok())
        .filter(|root| root.is_dir());
    state.search.follow(root);
}

/// The chips' scopes, then the typed ones, once each.
fn wanted_scopes(chips: Vec<String>, typed: Vec<String>) -> Vec<String> {
    let mut wanted = Vec::new();
    for scope in chips.into_iter().chain(typed) {
        if !wanted.contains(&scope) {
            wanted.push(scope);
        }
    }
    wanted
}

/// The notes matching `text` within the `chips`' scopes, highlighted.
fn run_query(app: &AppHandle, text: &str, chips: Vec<String>) -> SearchResponse {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let query = query::parse(text);
    let wanted = wanted_scopes(chips, query.scopes);
    let folders = scopes::resolve(&settings.search.scopes).prefixes(&wanted);
    let exclude = scopes::exclude_prefixes(&settings.search.exclude);
    let index = &state.search.index;
    let now = chrono::Local::now().timestamp_millis();
    let highlight = Highlighter::new(&query.terms);
    let results = index
        .search(&query.terms, &folders, &exclude, now)
        .into_iter()
        .map(|hit| SearchResult {
            name_parts: highlight.parts(&hit.name),
            line_parts: highlight.parts(&hit.line),
            path: hit.path,
            name: hit.name,
            folder: hit.folder,
        })
        .collect();
    SearchResponse {
        results,
        terms: query.terms,
        scopes: wanted,
        indexing: index.is_building(),
        notes: u32::try_from(index.count()).unwrap_or(u32::MAX),
    }
}

/// The scope names for menus and choices: settings order, then `all`.
pub fn scope_names(app: &AppHandle) -> Vec<String> {
    let settings = app.state::<AppState>().settings.get();
    scopes::resolve(&settings.search.scopes).names()
}

fn default_scope(app: &AppHandle) -> Option<String> {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    scopes::default_scope(&settings, state.contexts.current(&settings))
}

async fn open(app: &AppHandle, scope: Option<String>) {
    window::panel::open(app, VIEW, json!({ "scope": scope })).await;
}

fn scope_choices(app: &AppHandle) -> Vec<ArgChoice> {
    scope_names(app)
        .into_iter()
        .map(|name| ArgChoice {
            title: Some(format!("Search Vault: {name}")),
            ..ArgChoice::new(name)
        })
        .collect()
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("search:open", "Search Vault…")
            .help("Find notes and preview them")
            .needs_vault()
            .run(|app| async move {
                let scope = default_scope(&app);
                open(&app, scope).await;
                Ok(())
            }),
    );
    bus.add_family(
        Command::new("search:scope:", "Search Vault… in one scope")
            .needs_vault()
            .family("<scope>")
            .choices(scope_choices)
            .run(|app, scope| async move {
                open(&app, Some(scope)).await;
                Ok(())
            }),
    );
}
