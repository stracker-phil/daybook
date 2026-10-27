//! The activity overlay's Tauri commands: its suggestions and its ↵.

use chrono::Local;
use tauri::{AppHandle, Manager};

use super::current::KeptActivity;
use super::pick::{ActivityPick, Plan, plan};
use super::session::ActivityMode;
use super::suggest::{self, LiveCall, Suggestion, SuggestionInput};
use super::{offered, recent, recent_timers, since_timer_started};
use crate::error::AppResult;
use crate::state::AppState;
use crate::store::settings::Project;
use crate::timetracker::client::{self, TimeTracker};
use crate::timetracker::reply::TimerState;
use crate::{calendar, calls, jira};

/// The overlay's suggestions, in its order.
#[tauri::command]
pub async fn activity_suggestions(app: AppHandle) -> Vec<Suggestion> {
    let settings = app.state::<AppState>().settings.get();
    let offered = offered(&settings, &app);
    let offered: Vec<&Project> = offered.iter().collect();
    let now = Local::now();
    let today = now.date_naive();
    let now = now.naive_local();
    let events = calendar::events_on(&app, today, today).await;
    let since = since_timer_started(&app, now);
    let call = calls::active(&app);
    let recent = recent::recent_activities(&recent_timers(&app).await, &offered);
    let tickets = jira::today_tickets(&app);
    suggest::activity_suggestions(&SuggestionInput {
        live_call: call.as_ref().map(|call| LiveCall {
            app: &call.app,
            title: &call.title,
        }),
        events: &events,
        since,
        now,
        tickets: &tickets,
        recent: &recent,
        projects: &offered,
    })
}

/// Does the pick's plan in TimeTracker: starts its timer, stops a running one for an activity without
/// a timer, or sends nothing for an unchanged check-in; what it did.
pub fn submit_pick(
    tt: &dyn TimeTracker,
    mode: ActivityMode,
    pick: &ActivityPick,
    current: Option<&KeptActivity>,
    links: &[String],
    running: Option<&TimerState>,
) -> AppResult<Plan> {
    let plan = plan(mode, pick, current, links, running);
    match &plan {
        Plan::Start {
            project_id,
            note,
            tag,
        } => client::start(tt, project_id, note, tag.as_deref())?,
        Plan::NoTimer if running.is_some() => client::stop(tt)?,
        Plan::NoTimer | Plan::Unchanged => {}
    }
    Ok(plan)
}

/// ↵: does the pick in TimeTracker and keeps it as the current activity (then Working, during a break
/// or after the day); then the overlay closes. An error leaves the overlay open.
#[tauri::command]
pub async fn activity_submit(
    app: AppHandle,
    mode: ActivityMode,
    pick: ActivityPick,
) -> AppResult<()> {
    super::submit(&app, mode, pick).await
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use crate::error::ErrorKind;
    use crate::timetracker::client::{RawReply, RunFailure, Watch};

    struct Fake {
        answer: Result<RawReply, RunFailure>,
        calls: Mutex<Vec<Vec<String>>>,
    }

    impl Fake {
        fn answering(code: i32, stdout: &str, stderr: &str) -> Self {
            Self {
                answer: Ok(RawReply {
                    code: Some(code),
                    stdout: stdout.into(),
                    stderr: stderr.into(),
                }),
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.lock().unwrap().clone()
        }
    }

    impl TimeTracker for Fake {
        fn run(&self, args: &[&str]) -> Result<RawReply, RunFailure> {
            self.calls
                .lock()
                .unwrap()
                .push(args.iter().map(|arg| arg.to_string()).collect());
            self.answer.clone()
        }

        fn watch(&self) -> Result<Box<dyn Watch>, RunFailure> {
            Err(RunFailure::Missing)
        }
    }

    fn running() -> TimerState {
        TimerState {
            recording_id: "r1".into(),
            project_id: "tt1".into(),
            project: "SDK v2".into(),
            client: Some("Oscorp".into()),
            tag_id: None,
            tag: None,
            note: Some("QRS-6805 review".into()),
            started_at: 0,
        }
    }

    fn pick(tt_project: Option<&str>, note: &str) -> ActivityPick {
        ActivityPick {
            project: "Oscorp".into(),
            tt_project_id: tt_project.map(str::to_string),
            note: note.into(),
            tag: None,
        }
    }

    fn kept(note: &str) -> KeptActivity {
        KeptActivity {
            project: "Oscorp".into(),
            note: note.into(),
        }
    }

    fn links(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|id| id.to_string()).collect()
    }

    #[test]
    fn submitting_starts_the_timer_on_the_only_link_with_the_note_verbatim() {
        let tt = Fake::answering(0, "", "");
        let pick = pick(None, "  - fix: the thing ");
        let plan = submit_pick(
            &tt,
            ActivityMode::Switch,
            &pick,
            None,
            &links(&["tt2"]),
            None,
        )
        .unwrap();
        assert_eq!(
            plan,
            Plan::Start {
                project_id: "tt2".into(),
                note: "  - fix: the thing ".into(),
                tag: None,
            }
        );
        assert_eq!(
            tt.calls(),
            vec![vec![
                "timer",
                "start",
                "tt2",
                "--note=  - fix: the thing ",
                "--json"
            ]]
        );
    }

    #[test]
    fn submitting_with_several_links_starts_the_picked_one() {
        let tt = Fake::answering(0, "", "");
        let pick = pick(Some("tt3"), "review");
        submit_pick(
            &tt,
            ActivityMode::Switch,
            &pick,
            None,
            &links(&["tt2", "tt3"]),
            None,
        )
        .unwrap();
        assert_eq!(
            tt.calls(),
            vec![vec!["timer", "start", "tt3", "--note=review", "--json"]]
        );
    }

    #[test]
    fn an_unchanged_check_in_sends_nothing() {
        let tt = Fake::answering(0, "", "");
        let timer = running();
        let plan = submit_pick(
            &tt,
            ActivityMode::Checkin,
            &pick(None, "QRS-6805 review"),
            Some(&kept("QRS-6805 review")),
            &links(&["tt1"]),
            Some(&timer),
        )
        .unwrap();
        assert_eq!(plan, Plan::Unchanged);
        assert_eq!(tt.calls(), Vec::<Vec<String>>::new());
    }

    #[test]
    fn a_confirmed_check_in_never_stops_a_timer_on_an_unlinked_project() {
        let tt = Fake::answering(0, "", "");
        let timer = running();
        let plan = submit_pick(
            &tt,
            ActivityMode::Checkin,
            &pick(None, "QRS-6805 review"),
            Some(&kept("QRS-6805 review")),
            &links(&[]),
            Some(&timer),
        )
        .unwrap();
        assert_eq!(plan, Plan::Unchanged);
        assert_eq!(tt.calls(), Vec::<Vec<String>>::new());
    }

    #[test]
    fn a_confirmed_check_in_with_several_links_starts_the_picked_project() {
        let tt = Fake::answering(0, "", "");
        let timer = running();
        submit_pick(
            &tt,
            ActivityMode::Checkin,
            &pick(Some("tt3"), "QRS-6805 review"),
            Some(&kept("QRS-6805 review")),
            &links(&["tt1", "tt3"]),
            Some(&timer),
        )
        .unwrap();
        assert_eq!(
            tt.calls(),
            vec![vec![
                "timer",
                "start",
                "tt3",
                "--note=QRS-6805 review",
                "--json"
            ]]
        );
    }

    #[test]
    fn a_changed_check_in_starts_the_new_timer() {
        let tt = Fake::answering(0, "", "");
        let timer = running();
        submit_pick(
            &tt,
            ActivityMode::Checkin,
            &pick(None, "QRS-7000 deploy"),
            Some(&kept("QRS-6805 review")),
            &links(&["tt1"]),
            Some(&timer),
        )
        .unwrap();
        assert_eq!(
            tt.calls(),
            vec![vec![
                "timer",
                "start",
                "tt1",
                "--note=QRS-7000 deploy",
                "--json"
            ]]
        );
    }

    #[test]
    fn an_activity_without_a_timer_stops_a_running_timer() {
        let timer = running();
        for (links, picked, label) in [
            (links(&[]), None, "no links"),
            (links(&["tt1", "tt2"]), None, "several links, none picked"),
        ] {
            let tt = Fake::answering(0, "", "");
            let plan = submit_pick(
                &tt,
                ActivityMode::Switch,
                &pick(picked, "gardening"),
                None,
                &links,
                Some(&timer),
            )
            .unwrap();
            assert_eq!(plan, Plan::NoTimer, "{label}");
            assert_eq!(tt.calls(), vec![vec!["timer", "stop", "--json"]], "{label}");
        }
    }

    #[test]
    fn an_activity_without_a_timer_sends_nothing_when_none_runs() {
        for (mode, links) in [
            (ActivityMode::Switch, links(&[])),
            (ActivityMode::Switch, links(&["tt1", "tt2"])),
            (ActivityMode::Checkin, links(&[])),
            (ActivityMode::Checkin, links(&["tt1", "tt2"])),
        ] {
            let tt = Fake::answering(0, "", "");
            submit_pick(&tt, mode, &pick(None, "gardening"), None, &links, None).unwrap();
            assert_eq!(tt.calls(), Vec::<Vec<String>>::new(), "{mode:?} {links:?}");
        }
    }

    #[test]
    fn an_error_from_timetracker_comes_back_as_its_kind() {
        for (code, stdout, stderr, kind, message) in [
            (
                1,
                r#"{"kind":"invalid_input","message":"`note`: must be a single line"}"#,
                "",
                ErrorKind::TimetrackerRefused,
                "`note`: must be a single line",
            ),
            (
                2,
                "",
                "TimeTracker is not running\n",
                ErrorKind::TimetrackerNotRunning,
                "TimeTracker isn't running.",
            ),
        ] {
            let tt = Fake::answering(code, stdout, stderr);
            let pick = pick(None, "a\nb");
            let err = submit_pick(
                &tt,
                ActivityMode::Switch,
                &pick,
                None,
                &links(&["tt1"]),
                None,
            )
            .unwrap_err();
            assert_eq!(err.kind(), kind, "{code}");
            assert_eq!(err.to_string(), message, "{code}");
        }
    }
}
