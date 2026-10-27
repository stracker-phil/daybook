//! Project Launcher through its command line tool `plaunch`: the projects it lists (for Settings →
//! Projects) and showing one (the Projects tool, the `projects` overlay).

pub mod parse;
mod tool;

pub use tool::{Launcher, LauncherProjects};

use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Manager};
use ts_rs::TS;

use crate::bus::{ArgChoice, Command, CommandBus};
use crate::error::AppResult;
use crate::state::AppState;
use crate::tools::ProjectGroup;
use crate::vault::commands::blocking;
use crate::{commands, projects, timetracker, window};

const VIEW: &str = "projects";
/// `projects:open:<id>` shows a Launcher project.
pub const OPEN_PREFIX: &str = "projects:open:";

/// The current projects' Launcher projects, as the Projects tool shows them.
fn groups(app: &AppHandle) -> Vec<ProjectGroup> {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let offered = state.contexts.current_projects(&settings);
    projects::project_groups(&offered, timetracker::current_project(app).as_deref())
}

/// A Launcher project of the overlay; `command` shows it.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct LauncherRow {
    pub id: String,
    pub name: String,
    pub command: String,
}

/// A project and its Launcher projects, for the `projects` overlay.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "ProjectGroup")]
#[serde(rename_all = "camelCase")]
pub struct LauncherGroup {
    pub project: String,
    pub projects: Vec<LauncherRow>,
}

pub fn open_command(id: &str) -> String {
    format!("{OPEN_PREFIX}{id}")
}

fn group_rows(group: ProjectGroup) -> LauncherGroup {
    LauncherGroup {
        project: group.project,
        projects: group
            .projects
            .into_iter()
            .map(|project| LauncherRow {
                command: open_command(&project.id),
                id: project.id,
                name: project.name,
            })
            .collect(),
    }
}

/// Linked projects take the names Project Launcher lists now; saved only when one changed.
async fn follow_names(app: &AppHandle, listed: &LauncherProjects) -> AppResult<()> {
    if listed.error.is_some() || !listed.found {
        return Ok(());
    }
    let state = app.state::<AppState>();
    let mut settings = state.settings.get();
    if projects::follow_launcher_names(&mut settings.projects, &listed.projects) {
        commands::save_settings(app, &state, settings).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn launcher_projects(app: AppHandle, fresh: bool) -> AppResult<LauncherProjects> {
    let handle = app.clone();
    let listed = blocking(move || Ok(handle.state::<AppState>().launcher.list(fresh))).await?;
    follow_names(&app, &listed).await?;
    Ok(listed)
}

#[tauri::command]
pub fn projects_view(app: AppHandle) -> Vec<LauncherGroup> {
    groups(&app).into_iter().map(group_rows).collect()
}

fn project_choices(app: &AppHandle) -> Vec<ArgChoice> {
    let mut choices: Vec<ArgChoice> = Vec::new();
    for project in groups(app).into_iter().flat_map(|group| group.projects) {
        if !choices.iter().any(|choice| choice.argument == project.id) {
            choices.push(ArgChoice {
                word: project.name.clone(),
                title: Some(project.name),
                argument: project.id,
            });
        }
    }
    choices
}

async fn show(app: AppHandle, id: String) -> AppResult<()> {
    window::panel::close(&app, true);
    blocking(move || app.state::<AppState>().launcher.open(&id)).await
}

pub fn register(bus: &mut CommandBus<AppHandle>) {
    bus.add(
        Command::new("projects:open", "Projects…")
            .help("Projects… (their Project Launcher projects)")
            .run(|app| async move {
                window::panel::open(&app, VIEW, Value::Null).await;
                Ok(())
            }),
    );
    bus.add_family(
        Command::new(OPEN_PREFIX, "Show a project in Project Launcher")
            .family("<project>")
            .choices(project_choices)
            .run(show),
    );
}
