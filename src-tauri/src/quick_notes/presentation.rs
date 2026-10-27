//! Presenting the open quick note (docs/windows.md#quick-notes-editor): the state, kept in memory only,
//! the command a (re)loading page asks, and the bus commands. The window work is in
//! `window::presentation`.

use serde::Serialize;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use super::{Frame, QuickNotes};
use crate::bus::{Command, CommandBus};
use crate::lock::MutexExt;
use crate::state::AppState;
use crate::window;
use crate::window::display::{Presentation, Rect};

#[derive(Serialize, Clone, Copy, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct QuickNotePresentation {
    pub presenting: bool,
}

impl QuickNotes {
    pub fn presentation(&self) -> Option<Presentation> {
        *self.presentation.lock_unpoisoned()
    }

    pub fn presenting(&self) -> bool {
        self.presentation().is_some()
    }

    pub fn set_presentation(&self, presentation: Presentation) {
        *self.presentation.lock_unpoisoned() = Some(presentation);
    }

    /// Ends the presentation and returns it.
    pub fn take_presentation(&self) -> Option<Presentation> {
        self.presentation.lock_unpoisoned().take()
    }

    /// The frame `quick_notes.json` keeps: `current`, or the normal frame while presenting, so it never
    /// holds a full-screen one. `main_height` is the main screen's height, to convert the Cocoa frame.
    pub fn frame_to_remember(&self, current: Frame, main_height: f64) -> Frame {
        match self.presentation() {
            Some(presentation) => Frame::from_cocoa(presentation.normal, main_height),
            None => current,
        }
    }
}

impl Frame {
    /// A Cocoa frame (y up from the bottom of the main screen, `main_height` tall) as a top-left frame.
    fn from_cocoa(rect: Rect, main_height: f64) -> Self {
        Self {
            x: rect.x,
            y: main_height - rect.y - rect.height,
            width: rect.width,
            height: rect.height,
        }
    }
}

#[tauri::command]
pub fn quick_notes_presentation(app: AppHandle) -> QuickNotePresentation {
    QuickNotePresentation {
        presenting: app.state::<AppState>().quick_notes.presenting(),
    }
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("notes:present", "Present Quick Note")
            .help("Fill the editor's screen with the open quick note; again returns to the small editor")
            .enabled_when(window::quick_notes::visible)
            .run_now(window::quick_notes::toggle_presentation),
    );
    // Esc while presenting: never toggles back in.
    bus.add(
        Command::new("notes:endPresentation", "End Quick Note Presentation")
            .hidden()
            .run_now(window::presentation::end),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::temp_config_dir;
    use crate::window::display::Screen;

    const NORMAL: Rect = Rect::new(100.0, 200.0, 600.0, 400.0);

    fn presentation() -> Presentation {
        let frame = Rect::new(0.0, 0.0, 1440.0, 900.0);
        Presentation {
            normal: NORMAL,
            screen: Screen {
                frame,
                visible: frame,
            },
        }
    }

    #[test]
    fn a_new_editor_is_not_presenting() {
        let dir = temp_config_dir();
        let quick_notes = QuickNotes::open(dir.path());
        assert!(!quick_notes.presenting());
        assert_eq!(quick_notes.presentation(), None);
    }

    #[test]
    fn a_set_presentation_is_presenting_and_can_be_read_back() {
        let dir = temp_config_dir();
        let quick_notes = QuickNotes::open(dir.path());
        quick_notes.set_presentation(presentation());
        assert!(quick_notes.presenting());
        assert_eq!(quick_notes.presentation(), Some(presentation()));
    }

    #[test]
    fn taking_the_presentation_returns_it_and_ends_it() {
        let dir = temp_config_dir();
        let quick_notes = QuickNotes::open(dir.path());
        quick_notes.set_presentation(presentation());
        assert_eq!(quick_notes.take_presentation(), Some(presentation()));
        assert!(!quick_notes.presenting());
        assert_eq!(quick_notes.take_presentation(), None);
    }

    #[test]
    fn the_frame_to_remember_is_the_current_one_when_not_presenting() {
        let dir = temp_config_dir();
        let current = Frame {
            x: 1.0,
            y: 2.0,
            width: 3.0,
            height: 4.0,
        };
        assert_eq!(
            QuickNotes::open(dir.path()).frame_to_remember(current, 1000.0),
            current
        );
    }

    #[test]
    fn the_frame_to_remember_while_presenting_is_the_normal_one() {
        let dir = temp_config_dir();
        let quick_notes = QuickNotes::open(dir.path());
        quick_notes.set_presentation(presentation());
        let fullscreen = Frame {
            x: 0.0,
            y: 0.0,
            width: 1440.0,
            height: 900.0,
        };
        assert_eq!(
            quick_notes.frame_to_remember(fullscreen, 1000.0),
            Frame {
                x: 100.0,
                y: 400.0,
                width: 600.0,
                height: 400.0,
            }
        );
    }

    #[test]
    fn the_presentation_state_is_sent_to_the_page_as_presenting() {
        assert_eq!(
            serde_json::to_value(QuickNotePresentation { presenting: true }).unwrap(),
            serde_json::json!({ "presenting": true })
        );
    }

    #[test]
    fn a_cocoa_frame_is_flipped_to_the_top_left_and_keeps_x_and_size() {
        assert_eq!(
            Frame::from_cocoa(Rect::new(100.0, 200.0, 600.0, 400.0), 1000.0),
            Frame {
                x: 100.0,
                y: 400.0,
                width: 600.0,
                height: 400.0,
            }
        );
    }
}
