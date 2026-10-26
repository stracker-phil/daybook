use std::path::Path;
use std::sync::Arc;

use crate::contexts::Contexts;
use crate::launch::LaunchQueue;
use crate::routines::Fired;
use crate::slack::SlackFiles;
use crate::status::Statuses;
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
    pub fired: Fired,
    pub todos: NotesIndex,
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
            fired: Fired::open(config_dir),
            todos: NotesIndex::default(),
        }
    }
}
