//! What the tools read: a snapshot that each feature fills with its part.

use crate::store::settings::Feature;
pub use crate::store::settings::Status;
pub use crate::timetracker::TtState;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TodayNote {
    pub exists: bool,
    /// Obsidian is creating it.
    pub creating: bool,
    /// Changes waiting for it.
    pub waiting: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ContextItem {
    pub id: String,
    pub label: String,
    pub name: String,
}

/// Answered questions of today's journal (overall or one group).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    pub done: u32,
    pub total: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GroupProgress {
    pub title: String,
    pub progress: Progress,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoutineProgress {
    pub id: String,
    pub name: String,
    pub progress: Progress,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Link {
    pub title: String,
    pub target: String,
}

/// A link, or a project's links under its name.
#[derive(Clone, Debug, PartialEq)]
pub enum LinkItem {
    Link(Link),
    Group { title: String, links: Vec<Link> },
}

#[derive(Clone, Debug, PartialEq)]
pub struct LauncherProject {
    pub id: String,
    pub name: String,
}

/// A project's Project Launcher projects.
#[derive(Clone, Debug, PartialEq)]
pub struct ProjectGroup {
    pub project: String,
    pub projects: Vec<LauncherProject>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MusicItem {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Music {
    pub entries: Vec<MusicItem>,
    pub playing: bool,
    /// Only known while Music runs.
    pub volume: Option<u8>,
    /// The entry that plays or is paused.
    pub entry: Option<String>,
}

/// A topic of today's note that quick notes can go under.
#[derive(Clone, Debug, PartialEq)]
pub struct Topic {
    pub title: String,
    pub level: u8,
}

/// TimeTracker's running timer as the surfaces show it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TimerView {
    /// The linked Daybook project's name, else TimeTracker's project.
    pub project: String,
    /// Empty without a note.
    pub note: String,
}

impl TimerView {
    /// `Oscorp · QRS-6805 review`
    pub fn text(&self) -> String {
        [self.project.as_str(), self.note.as_str()]
            .into_iter()
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join(" · ")
    }
}

/// A timer used lately, on a project that counts now.
#[derive(Clone, Debug, PartialEq)]
pub struct RecentActivity {
    /// The Daybook project.
    pub project: String,
    /// Empty without a note.
    pub note: String,
    /// TimeTracker's project, so a pick starts the same timer.
    pub tt_project_id: String,
    pub tag: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct ToolState {
    pub has_vault: bool,
    pub today: TodayNote,
    pub contexts: Vec<ContextItem>,
    /// The id of the current context.
    pub context: Option<String>,
    /// The current context's features.
    pub features: Vec<Feature>,
    /// `None` until a status was set today.
    pub status: Option<Status>,
    /// `HH:MM` the status was set.
    pub status_since: Option<String>,
    pub journal: Progress,
    pub journal_groups: Vec<GroupProgress>,
    pub routines: Vec<RoutineProgress>,
    /// The links that match now.
    pub links_now: Vec<LinkItem>,
    /// Every other link, so any link stays reachable.
    pub links_more: Vec<LinkItem>,
    pub projects: Vec<ProjectGroup>,
    /// `HH:MM` the detected call started.
    pub meeting_since: Option<String>,
    /// The app trigger of the detected call, e.g. Zoom.
    pub meeting_app: Option<String>,
    pub meeting_detection: bool,
    pub music: Music,
    pub search_scopes: Vec<String>,
    pub note_topics: Vec<Topic>,
    /// Today's Jira tickets, newest first.
    pub tickets: Vec<String>,
    pub tt: TtState,
    /// TimeTracker's running timer.
    pub timer: Option<TimerView>,
    /// Timers used lately, newest first.
    pub recent_activities: Vec<RecentActivity>,
    /// The current context's check-in interval; 0: none.
    pub check_in_minutes: u32,
    pub check_ins_paused: bool,
}

/// A vault with today's note, two contexts (the first current, with Status and Jira), the status
/// `Working`, and nothing else to pick.
#[cfg(test)]
pub(crate) fn bare_state() -> ToolState {
    ToolState {
        has_vault: true,
        today: TodayNote {
            exists: true,
            ..TodayNote::default()
        },
        contexts: vec![
            ContextItem {
                id: "work".into(),
                label: "[W]".into(),
                name: "Work".into(),
            },
            ContextItem {
                id: "private".into(),
                label: String::new(),
                name: "Private".into(),
            },
        ],
        context: Some("work".into()),
        features: vec![Feature::Status, Feature::Jira],
        status: Some(Status::Working),
        meeting_detection: true,
        ..ToolState::default()
    }
}

/// `bare_state` with something to pick in every tool.
#[cfg(test)]
pub(crate) fn fixture() -> ToolState {
    let link = |title: &str, target: &str| Link {
        title: title.into(),
        target: target.into(),
    };
    ToolState {
        journal: Progress { done: 1, total: 3 },
        journal_groups: vec![
            GroupProgress {
                title: "Dawn".into(),
                progress: Progress { done: 1, total: 3 },
            },
            GroupProgress {
                title: "Health".into(),
                progress: Progress { done: 0, total: 0 },
            },
        ],
        routines: vec![RoutineProgress {
            id: "wrap".into(),
            name: "Wrap-up".into(),
            progress: Progress { done: 1, total: 3 },
        }],
        links_now: vec![LinkItem::Link(link("Board", "https://board"))],
        links_more: vec![LinkItem::Group {
            title: "Oscorp".into(),
            links: vec![link("Wiki", "https://wiki")],
        }],
        projects: vec![ProjectGroup {
            project: "Oscorp".into(),
            projects: vec![LauncherProject {
                id: "p1".into(),
                name: "QRS".into(),
            }],
        }],
        music: Music {
            entries: vec![MusicItem {
                id: "cell".into(),
                name: "Cell".into(),
            }],
            playing: true,
            volume: Some(48),
            entry: Some("cell".into()),
        },
        search_scopes: vec!["work".into(), "all".into()],
        note_topics: vec![Topic {
            title: "Daily".into(),
            level: 3,
        }],
        tickets: vec!["QRS-1".into()],
        ..bare_state()
    }
}

impl ToolState {
    pub fn has_feature(&self, feature: Feature) -> bool {
        self.features.contains(&feature)
    }

    pub fn current_context(&self) -> Option<&ContextItem> {
        let id = self.context.as_deref()?;
        self.contexts.iter().find(|context| context.id == id)
    }
}
