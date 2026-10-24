use std::path::Path;

use crate::launch::LaunchQueue;
use crate::store::keybindings::KeybindingsStore;
use crate::store::settings::SettingsStore;
use crate::store::ui_state::UiStateStore;

pub struct AppState {
    pub settings: SettingsStore,
    pub keybindings: KeybindingsStore,
    pub ui_state: UiStateStore,
    pub launches: LaunchQueue,
}

impl AppState {
    pub fn open(config_dir: &Path) -> Self {
        Self {
            settings: SettingsStore::open(config_dir),
            keybindings: KeybindingsStore::open(config_dir),
            ui_state: UiStateStore::open(config_dir),
            launches: LaunchQueue::default(),
        }
    }
}
