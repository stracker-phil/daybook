//! Settings → Status's diagnostics: what the vault, Slack, TimeTracker, the permissions and the
//! automation entries report now (`groups` builds the texts).

pub mod groups;

use tauri::{AppHandle, Manager};

use groups::Diagnostics;
use groups::{
    AutomationFacts, SlackFacts, TimeTrackerFacts, VaultFacts, automation_group, permissions_group,
    slack_group, timetracker_group, vault_group,
};

use crate::state::AppState;
use crate::{automation, calendar, calls, http_api, slack, timetracker};

/// The URL scheme this build claims (`tauri.conf.json`, `tauri.debug.conf.json`).
const SCHEME: &str = if cfg!(debug_assertions) {
    "daybook-debug"
} else {
    "daybook"
};

fn vault_facts(app: &AppHandle) -> VaultFacts {
    let state = app.state::<AppState>();
    let settings = state.settings.get();
    let today = state.notes.state(&settings);
    VaultFacts {
        path: settings.vault.path.clone(),
        today_note: today.path,
        today_exists: today.exists,
        waiting: today.waiting,
    }
}

fn timetracker_facts(
    app: &AppHandle,
    info: timetracker::commands::TimeTrackerInfo,
) -> TimeTrackerFacts {
    let settings = app.state::<AppState>().settings.get();
    let linked = settings
        .projects
        .iter()
        .filter(|project| project.active && !project.timetracker_projects.is_empty())
        .count();
    TimeTrackerFacts {
        state: info.state,
        version: info.version,
        watch: timetracker::watch_link(app),
        linked,
        unlinked: timetracker::link::unlinked_project_names(&settings.projects),
    }
}

fn automation_facts(app: &AppHandle) -> AutomationFacts {
    let cli = automation::cli::cli_info();
    let api = http_api::api_status(app);
    AutomationFacts {
        cli_path: cli.path,
        cli_linked: cli.linked,
        api_running: api.running,
        api_error: api.error,
        api_port: api.port,
        scheme: SCHEME.into(),
    }
}

async fn slack_facts(app: &AppHandle) -> SlackFacts {
    let info = slack::commands::slack_info(app.clone()).await;
    SlackFacts {
        connected: info.connected,
        identity: info.identity,
        last_error: slack::last_error(app),
    }
}

#[tauri::command]
pub async fn diagnostics_get(app: AppHandle) -> Diagnostics {
    let slack = slack_facts(&app).await;
    let tt = timetracker::info(&app).await;
    let calendar = calendar::calendar_info(app.clone()).await.access;
    Diagnostics {
        groups: vec![
            vault_group(&vault_facts(&app)),
            slack_group(&slack),
            timetracker_group(&timetracker_facts(&app, tt)),
            permissions_group(calendar, calls::access::is_trusted()),
            automation_group(&automation_facts(&app)),
        ],
    }
}
