//! A waiting change's line in the action log.

use super::PendingEntry;
use crate::action_log::{self, ActionState, ActionTarget};

/// The action log's title for a waiting change written after a restart.
const WAITING_TITLE: &str = "Waiting entries";

impl PendingEntry {
    /// Moves its action-log line on; one from an earlier session gets a line once it is written.
    pub(super) fn report(&self, state: ActionState, detail: Option<String>) {
        if self.logged.is_logged() {
            self.logged.update(state, detail);
        } else if state != ActionState::Waiting {
            action_log::log_titled(
                Some(WAITING_TITLE),
                ActionTarget::Obsidian,
                &self.description,
                state,
                detail,
            );
        }
    }
}
