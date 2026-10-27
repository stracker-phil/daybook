//! Undo for a switch by work hours: its notification's button switches back while the scheduled
//! context is still current.

use tauri::{AppHandle, Manager};

use super::switch;
use crate::action_log;
use crate::bus::{Command, CommandBus};
use crate::error::AppResult;
use crate::lock::MutexExt;
use crate::state::AppState;

/// The scheduled switch's notification button.
pub(super) const UNDO: &str = "context:undo";

/// The last switch by work hours, which Undo reverts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct LastSwitch {
    pub to: String,
    pub from: String,
}

/// The context to switch back to: `last.from`, only while `last.to` is still current.
pub(super) fn undo_target<'a>(last: &'a LastSwitch, current: Option<&str>) -> Option<&'a str> {
    (current == Some(last.to.as_str())).then_some(last.from.as_str())
}

async fn undo_schedule(app: AppHandle) -> AppResult<()> {
    let state = app.state::<AppState>();
    let Some(last) = state.contexts.undo.lock_unpoisoned().take() else {
        return Ok(());
    };
    let settings = state.settings.get();
    let current = state.contexts.current(&settings).map(|c| c.id.as_str());
    let Some(from) = undo_target(&last, current) else {
        return Ok(());
    };
    let name = |id: &str| {
        settings
            .contexts
            .iter()
            .find(|context| context.id == id)
            .map_or_else(|| id.to_string(), |context| context.name.clone())
    };
    let title = format!("Context change undone: {} → {}", name(&last.to), name(from));
    action_log::traced_sync(&title, |trigger| {
        trigger.keep();
        switch(&app, from).map(|_| ())
    })
}

pub(super) fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new(UNDO, "Undo the Context Switch")
            .help("Switches back from the last switch by work hours")
            .hidden()
            .run(undo_schedule),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last_switch() -> LastSwitch {
        LastSwitch {
            to: "work".into(),
            from: "private".into(),
        }
    }

    #[test]
    fn undo_switches_back_while_the_scheduled_context_is_current() {
        assert_eq!(undo_target(&last_switch(), Some("work")), Some("private"));
    }

    #[test]
    fn undo_does_nothing_when_another_context_is_current() {
        for current in ["private", "other"] {
            assert_eq!(
                undo_target(&last_switch(), Some(current)),
                None,
                "{current}"
            );
        }
    }

    #[test]
    fn undo_does_nothing_without_a_current_context() {
        assert_eq!(undo_target(&last_switch(), None), None);
    }
}
