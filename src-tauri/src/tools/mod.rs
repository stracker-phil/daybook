//! The tools (context, status, journal, search, links, music, …) and what each lets you pick directly.
//! The menu-bar menu shows the choices as submenus, the palette lists them, and the rail shows them in a
//! tool's tooltip. Pure except `snapshot`: `ToolState` holds everything the tools read.

mod choices;
mod snapshot;
mod state;
mod visibility;

pub use choices::{
    Choice, Step, choice_commands, link_argument, link_command, link_target, tool_choices,
};
pub use snapshot::snapshot;
pub use state::*;
pub use visibility::{Surface, tidy_separators, visible_tools};
