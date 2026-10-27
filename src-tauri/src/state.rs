use std::path::Path;
use std::sync::Arc;

use crate::action_log::ActionLog;
use crate::activity::pause::CheckInPause;
use crate::contexts::Contexts;
use crate::jira::tickets::JiraTickets;
use crate::launch::LaunchQueue;
use crate::launcher::Launcher;
use crate::links::Links;
use crate::music::rules::Lowered;
use crate::quick_notes::QuickNotes;
use crate::routines::Fired;
use crate::search::Search;
use crate::slack::SlackFiles;
use crate::status::Statuses;
use crate::store::json_store::JsonStore;
use crate::store::keybindings::KeybindingsStore;
use crate::store::settings::SettingsStore;
use crate::store::ui_state::UiStateStore;
use crate::todos::NotesIndex;
use crate::vault::DailyNotes;

pub struct AppState {
    pub settings: SettingsStore,
    pub keybindings: KeybindingsStore,
    pub ui_state: UiStateStore,
    pub launches: LaunchQueue,
    pub notes: Arc<DailyNotes>,
    pub contexts: Contexts,
    pub statuses: Statuses,
    pub slack: SlackFiles,
    /// The music volume a call lowered, until it is back (`music-held.json`), for a quit mid-call.
    pub music_held: JsonStore<Option<Lowered>>,
    pub fired: Fired,
    pub todos: NotesIndex,
    pub quick_notes: QuickNotes,
    pub search: Search,
    pub links: Links,
    pub jira: JiraTickets,
    pub launcher: Launcher,
    pub check_in_pause: CheckInPause,
    pub action_log: Arc<ActionLog>,
}

impl AppState {
    pub fn open(config_dir: &Path) -> Self {
        Self {
            settings: SettingsStore::open(config_dir),
            keybindings: KeybindingsStore::open(config_dir),
            ui_state: UiStateStore::open(config_dir),
            launches: LaunchQueue::default(),
            notes: Arc::new(DailyNotes::open(config_dir)),
            contexts: Contexts::open(config_dir),
            statuses: Statuses::open(config_dir),
            slack: SlackFiles::open(config_dir),
            music_held: JsonStore::open(config_dir.join(crate::music::rules::HELD_FILE)),
            fired: Fired::open(config_dir),
            todos: NotesIndex::default(),
            quick_notes: QuickNotes::open(config_dir),
            search: Search::default(),
            links: Links::default(),
            jira: JiraTickets::open(config_dir),
            launcher: Launcher::default(),
            check_in_pause: CheckInPause::open(config_dir),
            action_log: ActionLog::open(config_dir),
        }
    }
}
