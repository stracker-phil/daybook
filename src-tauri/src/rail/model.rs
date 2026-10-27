//! The rail's buttons: the tools the rail shows, each with its tooltip, caption, shortcut and click. The
//! page only draws them. Pure.

use chrono::NaiveTime;
use serde::Serialize;
use ts_rs::TS;

use crate::bus::CommandStatus;
use crate::contexts::context_title;
use crate::hub::shortcut_for;
use crate::music::state::{MusicState, PlayerState};
use crate::status::status_label;
use crate::store::settings::{Hotkeys, Rail, Status, ToolId, ToolPlacement};
use crate::tools::{LinkItem, Surface, ToolState, TtState, tidy_separators, visible_tools};

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct RailTool {
    pub id: ToolId,
    /// The tooltip's first line.
    pub label: String,
    pub detail: Option<String>,
    /// For screen readers.
    pub aria_label: String,
    /// The short line under the icon; the page hides it when captions are off.
    pub caption: Option<String>,
    /// The global shortcut, in Settings notation (`ctrl+alt+cmd+f`).
    pub shortcut: Option<String>,
    /// The tool whose choices the tooltip lists.
    pub menu: Option<ToolId>,
    /// What a click runs through the bus.
    pub command: String,
    /// The context's short label, drawn instead of an icon.
    pub badge: Option<String>,
    /// A button filled with the accent colour (the call).
    pub filled: bool,
    /// The icon in the accent colour (music while it plays).
    pub accent: bool,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum RailButton {
    Tool(RailTool),
    Separator,
}

/// Payload of `rail_model` and the answer of `rail_model`.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct RailModel {
    pub buttons: Vec<RailButton>,
    /// Today's status for the status dot; `None` before the day's first draws a hollow ring.
    pub status: Option<Status>,
    /// The look, the delays and the tooltip settings.
    pub settings: Rail,
}

pub struct RailInput<'a> {
    pub tools: &'a [ToolPlacement],
    pub state: &'a ToolState,
    /// The global shortcuts that work (`shortcuts::shown`).
    pub hotkeys: &'a Hotkeys,
    pub commands: &'a dyn CommandStatus,
    pub music: &'a MusicState,
    /// The detected call's title, e.g. the meeting's name.
    pub call_title: Option<&'a str>,
    pub now: NaiveTime,
    pub settings: &'a Rail,
}

struct Button {
    label: String,
    detail: Option<String>,
    caption: Option<String>,
    command: &'static str,
    menu: bool,
}

fn button(label: &str, detail: Option<String>, command: &'static str, menu: bool) -> Button {
    Button {
        label: label.to_string(),
        detail,
        caption: None,
        command,
        menu,
    }
}

/// Whole minutes from `since` (`HH:MM`) to `now`: 0 if it lies ahead; `None` when `since` doesn't parse.
fn minutes_since(since: &str, now: NaiveTime) -> Option<i64> {
    let since = NaiveTime::parse_from_str(since, "%H:%M").ok()?;
    Some((now - since).num_minutes().max(0))
}

fn status_button(input: &RailInput) -> Button {
    let state = input.state;
    let break_minutes = state
        .status_since
        .as_deref()
        .filter(|_| state.status == Some(Status::Break))
        .and_then(|since| minutes_since(since, input.now));
    Button {
        caption: break_minutes.map(|minutes| format!("{minutes}m")),
        ..button(
            status_label(state.status),
            Some(break_minutes.map_or_else(
                || "Click to change your status".to_string(),
                |minutes| format!("For {minutes} min, click to change"),
            )),
            "status:toggle",
            true,
        )
    }
}

fn call_button(input: &RailInput) -> Button {
    let app = input.state.meeting_app.clone().unwrap_or_default();
    let what = match input.call_title.filter(|title| !title.is_empty()) {
        Some(title) if !app.is_empty() => format!("{app} · {title}"),
        Some(title) => title.to_string(),
        None => app.clone(),
    };
    Button {
        caption: (!app.is_empty()).then_some(app),
        ..button(
            &input.state.meeting_since.as_ref().map_or_else(
                || "In a meeting".to_string(),
                |since| format!("In a meeting since {since}"),
            ),
            Some(if what.is_empty() {
                "Click to check it’s still on".to_string()
            } else {
                format!("{what}, click to check it’s still on")
            }),
            "meetings:check",
            false,
        )
    }
}

/// The activity row: the running timer as caption and detail, else a prompt to log.
fn activity_button(state: &ToolState) -> Button {
    let detail = match (state.tt, &state.timer) {
        (TtState::Available, Some(timer)) => format!("Now: {}", timer.text()),
        (TtState::Available, None) => "Log what you just did".into(),
        (TtState::NotRunning, _) => "TimeTracker isn't running".into(),
        (TtState::Missing, _) => "TimeTracker isn't installed".into(),
    };
    Button {
        caption: state
            .timer
            .as_ref()
            .map(|timer| timer.project.clone())
            .filter(|project| !project.is_empty()),
        ..button("Log Activity…", Some(detail), "timetracker:show", false)
    }
}

fn journal_button(state: &ToolState) -> Button {
    let progress = &state.journal;
    let has_questions = progress.total > 0;
    Button {
        caption: has_questions.then(|| format!("{}/{}", progress.done, progress.total)),
        ..button(
            "Journal",
            Some(if has_questions {
                format!("{} of {} questions answered", progress.done, progress.total)
            } else {
                "Answer today’s journal questions".into()
            }),
            "journal:open",
            true,
        )
    }
}

/// The links that match now; a project counts each of its links.
fn links_button(state: &ToolState) -> Button {
    let (count, names) =
        state
            .links_now
            .iter()
            .fold((0, Vec::new()), |(count, mut names), item| {
                let (links, title) = match item {
                    LinkItem::Link(link) => (1, link.title.as_str()),
                    LinkItem::Group { title, links } => (links.len(), title.as_str()),
                };
                names.push(title);
                (count + links, names)
            });
    Button {
        caption: (count > 0).then(|| count.to_string()),
        ..button(
            "Links",
            Some(if count > 0 {
                format!("Now: {}", names.join(", "))
            } else {
                "No links for right now".into()
            }),
            "links:open",
            true,
        )
    }
}

fn projects_detail(state: &ToolState) -> Option<String> {
    let first = state.projects.first()?;
    let names: Vec<&str> = first.projects.iter().map(|p| p.name.as_str()).collect();
    Some(format!("{}: {}", first.project, names.join(", ")))
}

fn music_detail(music: &MusicState) -> String {
    let name = music
        .entry
        .as_ref()
        .and_then(|id| music.entries.iter().find(|entry| &entry.id == id))
        .map(|entry| entry.name.clone())
        .or_else(|| music.track.clone());
    let (state, alone) = match music.player {
        PlayerState::Playing => ("Playing", "Playing"),
        PlayerState::Paused => ("Paused", "Paused"),
        PlayerState::Stopped | PlayerState::Closed => return "Nothing is playing".into(),
    };
    name.map_or_else(|| alone.to_string(), |name| format!("{state}: {name}"))
}

fn tool_button(id: ToolId, input: &RailInput) -> Option<Button> {
    let state = input.state;
    let plain = |label: &str, detail: &str, command, menu| {
        button(label, Some(detail.to_string()), command, menu)
    };
    Some(match id {
        ToolId::Context => button(
            &state
                .current_context()
                .map(|context| context_title(&context.label, &context.name))
                .unwrap_or_else(|| "Context".into()),
            Some("Click to switch context".into()),
            "context:switch",
            true,
        ),
        ToolId::Status => status_button(input),
        ToolId::Activity => activity_button(state),
        ToolId::Call => call_button(input),
        ToolId::Journal => journal_button(state),
        ToolId::Hub => plain(
            "Daybook",
            "Your day at a glance: routines, journal, notes and open todos",
            "hub:open",
            false,
        ),
        ToolId::Search => plain(
            "Search Vault…",
            "Find notes and preview them",
            "search:open",
            true,
        ),
        ToolId::Jira => plain(
            "Open Jira Issue…",
            "By number, key or link",
            "jira:open",
            true,
        ),
        ToolId::QuickNotes => plain(
            "Quick Notes",
            "Write into a section of today’s notes, e.g. during a meeting",
            "notes:quick",
            true,
        ),
        ToolId::Links => links_button(state),
        ToolId::Projects => button("Projects…", projects_detail(state), "projects:open", true),
        ToolId::Music => button(
            "Music",
            Some(music_detail(input.music)),
            "music:toggle",
            true,
        ),
        ToolId::Separator1 | ToolId::Separator2 => return None,
    })
}

/// The context's own label, else the first three letters of its name.
fn badge(state: &ToolState) -> Option<String> {
    let context = state.current_context()?;
    let label = context.label.trim();
    Some(if label.is_empty() {
        context.name.chars().take(3).collect()
    } else {
        label.to_string()
    })
}

fn aria_label(id: ToolId, label: &str) -> String {
    match id {
        ToolId::Context => format!("Context: {label}. Switch context"),
        ToolId::Status => format!("{label}. Change status"),
        _ => label.to_string(),
    }
}

/// The rail's buttons in Settings → Tools order; a tool whose command no feature registered yet is
/// left out.
pub fn rail_model(input: &RailInput) -> RailModel {
    let ids = visible_tools(input.tools, input.state, Surface::Rail);
    let kept: Vec<(ToolId, Option<Button>)> = ids
        .into_iter()
        .map(|id| (id, tool_button(id, input)))
        .filter(|(_, button)| {
            button
                .as_ref()
                .is_none_or(|button| input.commands.knows(button.command))
        })
        .collect();
    let order = tidy_separators(kept.iter().map(|(id, _)| *id).collect());
    let mut kept = kept.into_iter();
    let buttons = order
        .into_iter()
        .filter_map(|id| kept.find(|(kept_id, _)| *kept_id == id))
        .map(|(id, button)| match button {
            None => RailButton::Separator,
            Some(button) => RailButton::Tool(RailTool {
                id,
                aria_label: aria_label(id, &button.label),
                label: button.label,
                detail: button.detail,
                caption: button.caption,
                shortcut: shortcut_for(id, input.hotkeys).map(str::to_string),
                menu: button.menu.then_some(id),
                command: button.command.to_string(),
                badge: (id == ToolId::Context)
                    .then(|| badge(input.state))
                    .flatten(),
                filled: id == ToolId::Call,
                accent: id == ToolId::Music && input.music.player == PlayerState::Playing,
            }),
        })
        .collect();
    RailModel {
        buttons,
        status: input.state.status,
        settings: input.settings.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::menu::model::CommandsStub;
    use crate::music::state::MusicListItem;
    use crate::tools::{
        ContextItem, LauncherProject, Link, Progress, ProjectGroup, TimerView, bare_state, fixture,
    };

    fn music(player: PlayerState, entry: Option<&str>, track: Option<&str>) -> MusicState {
        MusicState {
            entries: vec![MusicListItem {
                id: "cell".into(),
                name: "Cell".into(),
            }],
            player,
            track: track.map(String::from),
            volume: None,
            entry: entry.map(String::from),
        }
    }

    struct Setup {
        tools: Vec<ToolPlacement>,
        state: ToolState,
        hotkeys: Hotkeys,
        commands: CommandsStub,
        music: MusicState,
        call_title: Option<&'static str>,
        now: NaiveTime,
        settings: Rail,
    }

    impl Setup {
        fn new(ids: &[ToolId], state: ToolState) -> Self {
            Self {
                tools: ids
                    .iter()
                    .map(|id| ToolPlacement {
                        id: *id,
                        rail: true,
                        window: true,
                    })
                    .collect(),
                state,
                hotkeys: Hotkeys::none(),
                commands: CommandsStub::default(),
                music: music(PlayerState::Playing, Some("cell"), None),
                call_title: None,
                now: NaiveTime::from_hms_opt(10, 25, 0).expect("valid time"),
                settings: Rail::default(),
            }
        }

        fn model(&self) -> RailModel {
            rail_model(&RailInput {
                tools: &self.tools,
                state: &self.state,
                hotkeys: &self.hotkeys,
                commands: &self.commands,
                music: &self.music,
                call_title: self.call_title,
                now: self.now,
                settings: &self.settings,
            })
        }

        fn tool(&self, id: ToolId) -> RailTool {
            self.model()
                .buttons
                .into_iter()
                .find_map(|button| match button {
                    RailButton::Tool(tool) if tool.id == id => Some(tool),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no {id:?} button"))
        }

        fn order(&self) -> Vec<Option<ToolId>> {
            self.model()
                .buttons
                .into_iter()
                .map(|button| match button {
                    RailButton::Tool(tool) => Some(tool.id),
                    RailButton::Separator => None,
                })
                .collect()
        }
    }

    fn link(title: &str) -> Link {
        Link {
            title: title.into(),
            target: format!("https://{title}"),
        }
    }

    #[test]
    fn plain_tools_have_their_label_detail_command_and_menu() {
        for (id, label, detail, command, menu) in [
            (
                ToolId::Hub,
                "Daybook",
                "Your day at a glance: routines, journal, notes and open todos",
                "hub:open",
                false,
            ),
            (
                ToolId::Search,
                "Search Vault…",
                "Find notes and preview them",
                "search:open",
                true,
            ),
            (
                ToolId::Jira,
                "Open Jira Issue…",
                "By number, key or link",
                "jira:open",
                true,
            ),
            (
                ToolId::QuickNotes,
                "Quick Notes",
                "Write into a section of today’s notes, e.g. during a meeting",
                "notes:quick",
                true,
            ),
        ] {
            let tool = Setup::new(&[id], fixture()).tool(id);
            assert_eq!(
                tool,
                RailTool {
                    id,
                    label: label.into(),
                    detail: Some(detail.into()),
                    aria_label: label.into(),
                    caption: None,
                    shortcut: None,
                    menu: menu.then_some(id),
                    command: command.into(),
                    badge: None,
                    filled: false,
                    accent: false,
                },
                "{id:?}"
            );
        }
    }

    #[test]
    fn the_status_reads_its_label_and_asks_to_be_changed() {
        for (status, since, label, detail, caption) in [
            (
                Some(Status::Working),
                Some("10:00"),
                "Working and present",
                "Click to change your status",
                None,
            ),
            (
                Some(Status::Off),
                None,
                "Done for today",
                "Click to change your status",
                None,
            ),
            (
                None,
                None,
                "No status yet",
                "Click to change your status",
                None,
            ),
            (
                Some(Status::Break),
                Some("10:00"),
                "Taking a break",
                "For 25 min, click to change",
                Some("25m"),
            ),
            (
                Some(Status::Break),
                Some("10:25"),
                "Taking a break",
                "For 0 min, click to change",
                Some("0m"),
            ),
            (
                Some(Status::Break),
                Some("11:00"),
                "Taking a break",
                "For 0 min, click to change",
                Some("0m"),
            ),
            (
                Some(Status::Break),
                None,
                "Taking a break",
                "Click to change your status",
                None,
            ),
            (
                Some(Status::Break),
                Some("soon"),
                "Taking a break",
                "Click to change your status",
                None,
            ),
        ] {
            let state = ToolState {
                status,
                status_since: since.map(String::from),
                ..fixture()
            };
            let tool = Setup::new(&[ToolId::Status], state).tool(ToolId::Status);
            assert_eq!(tool.label, label, "{status:?} {since:?}");
            assert_eq!(tool.detail.as_deref(), Some(detail), "{status:?} {since:?}");
            assert_eq!(tool.caption.as_deref(), caption, "{status:?} {since:?}");
            assert_eq!(tool.aria_label, format!("{label}. Change status"));
            assert_eq!(tool.command, "status:toggle");
            assert_eq!(tool.menu, Some(ToolId::Status));
        }
    }

    #[test]
    fn the_model_carries_todays_status_and_the_rail_settings() {
        let mut setup = Setup::new(&[ToolId::Hub], fixture());
        setup.state.status = Some(Status::Break);
        setup.settings.captions = false;
        let model = setup.model();
        assert_eq!(model.status, Some(Status::Break));
        assert_eq!(model.settings, setup.settings);
    }

    #[test]
    fn the_context_shows_its_title_and_label_as_the_badge() {
        let tool = Setup::new(&[ToolId::Context], fixture()).tool(ToolId::Context);
        assert_eq!(tool.label, "[W] Work");
        assert_eq!(tool.detail.as_deref(), Some("Click to switch context"));
        assert_eq!(tool.aria_label, "Context: [W] Work. Switch context");
        assert_eq!(tool.command, "context:switch");
        assert_eq!(tool.menu, Some(ToolId::Context));
        assert_eq!(tool.badge.as_deref(), Some("[W]"));
    }

    #[test]
    fn a_context_without_a_label_shows_the_first_three_letters_of_its_name() {
        for (name, badge) in [("Private", "Pri"), ("Évening", "Éve"), ("Go", "Go")] {
            let mut state = fixture();
            state.contexts[1] = ContextItem {
                id: "private".into(),
                label: " ".into(),
                name: name.into(),
            };
            state.context = Some("private".into());
            let tool = Setup::new(&[ToolId::Context], state).tool(ToolId::Context);
            assert_eq!(tool.label, name);
            assert_eq!(tool.badge.as_deref(), Some(badge), "{name}");
        }
    }

    #[test]
    fn without_a_current_context_the_button_has_a_generic_label_and_no_badge() {
        let state = ToolState {
            context: None,
            ..fixture()
        };
        let tool = Setup::new(&[ToolId::Context], state).tool(ToolId::Context);
        assert_eq!(tool.label, "Context");
        assert_eq!(tool.badge, None);
    }

    #[test]
    fn the_activity_button_tells_about_the_timer_and_captions_its_project() {
        let timer = |project: &str, note: &str| {
            Some(TimerView {
                project: project.into(),
                note: note.into(),
            })
        };
        for (tt, view, detail, caption) in [
            (
                TtState::Available,
                timer("Oscorp", "note"),
                "Now: Oscorp · note",
                Some("Oscorp"),
            ),
            (
                TtState::Available,
                timer("Oscorp", ""),
                "Now: Oscorp",
                Some("Oscorp"),
            ),
            (TtState::Available, None, "Log what you just did", None),
            (TtState::NotRunning, None, "TimeTracker isn't running", None),
            (TtState::Missing, None, "TimeTracker isn't installed", None),
        ] {
            let state = ToolState {
                tt,
                timer: view.clone(),
                ..fixture()
            };
            let tool = Setup::new(&[ToolId::Activity], state).tool(ToolId::Activity);
            assert_eq!(tool.label, "Log Activity…");
            assert_eq!(tool.detail.as_deref(), Some(detail), "{tt:?} {view:?}");
            assert_eq!(tool.caption.as_deref(), caption, "{tt:?} {view:?}");
            assert_eq!(tool.command, "timetracker:show");
            assert_eq!(tool.menu, None);
        }
    }

    #[test]
    fn a_call_is_an_accent_filled_button_that_checks_the_meeting() {
        let mut setup = Setup::new(
            &[ToolId::Call],
            ToolState {
                meeting_since: Some("09:30".into()),
                meeting_app: Some("Zoom".into()),
                ..fixture()
            },
        );
        setup.call_title = Some("Standup");
        assert_eq!(
            setup.tool(ToolId::Call),
            RailTool {
                id: ToolId::Call,
                label: "In a meeting since 09:30".into(),
                detail: Some("Zoom · Standup, click to check it’s still on".into()),
                aria_label: "In a meeting since 09:30".into(),
                caption: Some("Zoom".into()),
                shortcut: None,
                menu: None,
                command: "meetings:check".into(),
                badge: None,
                filled: true,
                accent: false,
            }
        );
    }

    #[test]
    fn a_calls_detail_names_what_is_known_of_the_meeting() {
        for (app, title, detail, caption) in [
            (
                Some("Zoom"),
                None,
                "Zoom, click to check it’s still on",
                Some("Zoom"),
            ),
            (
                Some("Zoom"),
                Some(""),
                "Zoom, click to check it’s still on",
                Some("Zoom"),
            ),
            (
                None,
                Some("Standup"),
                "Standup, click to check it’s still on",
                None,
            ),
            (None, None, "Click to check it’s still on", None),
        ] {
            let mut setup = Setup::new(
                &[ToolId::Call],
                ToolState {
                    meeting_since: Some("09:30".into()),
                    meeting_app: app.map(String::from),
                    ..fixture()
                },
            );
            setup.call_title = title;
            let tool = setup.tool(ToolId::Call);
            assert_eq!(tool.detail.as_deref(), Some(detail), "{app:?} {title:?}");
            assert_eq!(tool.caption.as_deref(), caption, "{app:?} {title:?}");
        }
    }

    #[test]
    fn the_call_only_appears_during_a_call() {
        let setup = Setup::new(&[ToolId::Call, ToolId::Hub], fixture());
        assert_eq!(setup.order(), vec![Some(ToolId::Hub)]);
    }

    #[test]
    fn the_journal_counts_the_answered_questions() {
        let tool = Setup::new(&[ToolId::Journal], fixture()).tool(ToolId::Journal);
        assert_eq!(tool.label, "Journal");
        assert_eq!(tool.detail.as_deref(), Some("1 of 3 questions answered"));
        assert_eq!(tool.caption.as_deref(), Some("1/3"));
        assert_eq!(tool.command, "journal:open");
        assert_eq!(tool.menu, Some(ToolId::Journal));
    }

    #[test]
    fn a_journal_without_questions_invites_to_answer_and_has_no_caption() {
        let state = ToolState {
            journal: Progress { done: 0, total: 0 },
            ..fixture()
        };
        let tool = Setup::new(&[ToolId::Journal], state).tool(ToolId::Journal);
        assert_eq!(
            tool.detail.as_deref(),
            Some("Answer today’s journal questions")
        );
        assert_eq!(tool.caption, None);
    }

    #[test]
    fn links_name_what_matches_now_and_count_a_projects_links_each() {
        let state = ToolState {
            links_now: vec![
                LinkItem::Link(link("Board")),
                LinkItem::Group {
                    title: "Oscorp".into(),
                    links: vec![link("Wiki"), link("Docs")],
                },
            ],
            ..fixture()
        };
        let tool = Setup::new(&[ToolId::Links], state).tool(ToolId::Links);
        assert_eq!(tool.label, "Links");
        assert_eq!(tool.detail.as_deref(), Some("Now: Board, Oscorp"));
        assert_eq!(tool.caption.as_deref(), Some("3"));
        assert_eq!(tool.command, "links:open");
        assert_eq!(tool.menu, Some(ToolId::Links));
    }

    #[test]
    fn links_without_a_match_for_now_say_so_and_have_no_caption() {
        let state = ToolState {
            links_now: vec![],
            ..fixture()
        };
        let tool = Setup::new(&[ToolId::Links], state).tool(ToolId::Links);
        assert_eq!(tool.detail.as_deref(), Some("No links for right now"));
        assert_eq!(tool.caption, None);
    }

    #[test]
    fn projects_list_the_first_clients_projects() {
        let project = |client: &str, names: &[&str]| ProjectGroup {
            project: client.into(),
            projects: names
                .iter()
                .map(|name| LauncherProject {
                    id: name.to_lowercase(),
                    name: (*name).into(),
                })
                .collect(),
        };
        let state = ToolState {
            projects: vec![
                project("Oscorp", &["QRS", "Portal"]),
                project("Zeno", &["Other"]),
            ],
            ..fixture()
        };
        let tool = Setup::new(&[ToolId::Projects], state).tool(ToolId::Projects);
        assert_eq!(tool.label, "Projects…");
        assert_eq!(tool.detail.as_deref(), Some("Oscorp: QRS, Portal"));
        assert_eq!(tool.command, "projects:open");
        assert_eq!(tool.menu, Some(ToolId::Projects));
    }

    #[test]
    fn music_says_what_plays_and_colours_its_icon_only_while_playing() {
        for (player, entry, track, detail, accent) in [
            (
                PlayerState::Playing,
                Some("cell"),
                None,
                "Playing: Cell",
                true,
            ),
            (
                PlayerState::Paused,
                Some("cell"),
                None,
                "Paused: Cell",
                false,
            ),
            (
                PlayerState::Playing,
                None,
                Some("Song – Artist"),
                "Playing: Song – Artist",
                true,
            ),
            (
                PlayerState::Playing,
                Some("gone"),
                Some("Song – Artist"),
                "Playing: Song – Artist",
                true,
            ),
            (PlayerState::Playing, None, None, "Playing", true),
            (PlayerState::Paused, None, None, "Paused", false),
            (
                PlayerState::Stopped,
                Some("cell"),
                None,
                "Nothing is playing",
                false,
            ),
            (
                PlayerState::Closed,
                None,
                Some("Song"),
                "Nothing is playing",
                false,
            ),
        ] {
            let mut setup = Setup::new(&[ToolId::Music], fixture());
            setup.music = music(player, entry, track);
            let tool = setup.tool(ToolId::Music);
            assert_eq!(tool.label, "Music");
            assert_eq!(tool.detail.as_deref(), Some(detail), "{player:?} {entry:?}");
            assert_eq!(tool.accent, accent, "{player:?} {entry:?}");
            assert_eq!(tool.command, "music:toggle");
            assert_eq!(tool.menu, Some(ToolId::Music));
        }
    }

    #[test]
    fn shortcuts_come_from_the_hotkeys_and_are_left_off_when_unset() {
        let mut setup = Setup::new(&[ToolId::Search, ToolId::Hub, ToolId::Context], fixture());
        setup.hotkeys.search = "ctrl+alt+cmd+f".into();
        setup.hotkeys.switch_context = "ctrl+alt+cmd+c".into();
        assert_eq!(
            setup.tool(ToolId::Search).shortcut.as_deref(),
            Some("ctrl+alt+cmd+f")
        );
        assert_eq!(
            setup.tool(ToolId::Context).shortcut.as_deref(),
            Some("ctrl+alt+cmd+c")
        );
        assert_eq!(setup.tool(ToolId::Hub).shortcut, None);
    }

    #[test]
    fn tools_follow_the_settings_order_and_leave_out_those_off_the_rail() {
        let mut setup = Setup::new(&[ToolId::Search, ToolId::Hub, ToolId::Jira], fixture());
        assert_eq!(
            setup.order(),
            vec![Some(ToolId::Search), Some(ToolId::Hub), Some(ToolId::Jira)]
        );
        setup.tools[1].rail = false;
        assert_eq!(
            setup.order(),
            vec![Some(ToolId::Search), Some(ToolId::Jira)]
        );
    }

    #[test]
    fn tools_that_dont_apply_are_left_out() {
        let setup = Setup::new(
            &[ToolId::Status, ToolId::Jira, ToolId::Links, ToolId::Hub],
            ToolState {
                features: vec![],
                links_now: vec![],
                links_more: vec![],
                ..bare_state()
            },
        );
        assert_eq!(setup.order(), vec![Some(ToolId::Hub)]);
    }

    #[test]
    fn separators_are_tidied() {
        let setup = Setup::new(
            &[
                ToolId::Separator1,
                ToolId::Hub,
                ToolId::Separator1,
                ToolId::Separator2,
                ToolId::Search,
                ToolId::Separator2,
            ],
            fixture(),
        );
        assert_eq!(
            setup.order(),
            vec![Some(ToolId::Hub), None, Some(ToolId::Search)]
        );
    }

    #[test]
    fn a_tool_whose_command_no_feature_registered_is_left_out_with_its_separators_tidied() {
        for (ids, order) in [
            (
                vec![
                    ToolId::Hub,
                    ToolId::Separator1,
                    ToolId::Jira,
                    ToolId::Separator2,
                    ToolId::Search,
                ],
                vec![Some(ToolId::Hub), None, Some(ToolId::Search)],
            ),
            (
                vec![ToolId::Hub, ToolId::Separator1, ToolId::Jira],
                vec![Some(ToolId::Hub)],
            ),
            (
                vec![ToolId::Jira, ToolId::Separator1, ToolId::Hub],
                vec![Some(ToolId::Hub)],
            ),
        ] {
            let mut setup = Setup::new(&ids, fixture());
            setup.commands = CommandsStub {
                unknown: vec!["jira:open"],
                ..CommandsStub::default()
            };
            assert_eq!(setup.order(), order, "{ids:?}");
        }
    }

    #[test]
    fn the_buttons_serialize_with_their_kind_and_camel_case_names() {
        let setup = Setup::new(
            &[ToolId::Hub, ToolId::Separator1, ToolId::Search],
            fixture(),
        );
        let json = serde_json::to_value(setup.model()).expect("serializes");
        assert_eq!(
            json["buttons"],
            serde_json::json!([
                {
                    "kind": "tool",
                    "id": "hub",
                    "label": "Daybook",
                    "detail": "Your day at a glance: routines, journal, notes and open todos",
                    "ariaLabel": "Daybook",
                    "caption": null,
                    "shortcut": null,
                    "menu": null,
                    "command": "hub:open",
                    "badge": null,
                    "filled": false,
                    "accent": false,
                },
                { "kind": "separator" },
                {
                    "kind": "tool",
                    "id": "search",
                    "label": "Search Vault…",
                    "detail": "Find notes and preview them",
                    "ariaLabel": "Search Vault…",
                    "caption": null,
                    "shortcut": null,
                    "menu": "search",
                    "command": "search:open",
                    "badge": null,
                    "filled": false,
                    "accent": false,
                },
            ])
        );
        assert_eq!(json["status"], serde_json::json!("working"));
        assert_eq!(json["settings"]["tooltipDelay"], serde_json::json!(300));
    }
}
