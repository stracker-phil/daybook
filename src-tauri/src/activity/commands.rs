//! The activity overlay's Tauri commands: its suggestions and its ↵.

use chrono::Local;
use tauri::{AppHandle, Manager};

use super::pick::{ActivityPick, Plan, plan};
use super::session::ActivityMode;
use super::suggest::{self, LiveCall, Suggestion, SuggestionInput};
use super::{finish_pick, offered, recent, recent_timers, since_timer_started};
use crate::error::AppResult;
use crate::state::AppState;
use crate::store::settings::Project;
use crate::timetracker;
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

/// Starts TimeTracker's timer on the pick, or sends nothing for an unchanged check-in; what it did.
pub fn submit_pick(
    tt: &dyn TimeTracker,
    mode: ActivityMode,
    pick: &ActivityPick,
    running: Option<&TimerState>,
) -> AppResult<Plan> {
    let plan = plan(mode, pick, running)?;
    if let Plan::Start {
        project_id,
        note,
        tag,
    } = &plan
    {
        client::start(tt, project_id, note, tag.as_deref())?;
    }
    Ok(plan)
}

/// ↵: starts TimeTracker's timer on the pick (then Working, during a break or after the day), or keeps
/// it for an unchanged check-in; then the overlay closes. An error leaves the overlay open.
#[tauri::command]
pub async fn activity_submit(
    app: AppHandle,
    mode: ActivityMode,
    pick: ActivityPick,
) -> AppResult<()> {
    let running = timetracker::tracking(&app).timer;
    let plan = timetracker::blocking(&app, move |tool| {
        submit_pick(tool, mode, &pick, running.as_ref())
    })
    .await?;
    finish_pick(&app, &plan);
    Ok(())
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

    fn pick(project: &str, note: &str) -> ActivityPick {
        ActivityPick {
            tt_project_id: project.into(),
            note: note.into(),
            tag: None,
        }
    }

    #[test]
    fn submitting_starts_the_timer_on_the_picked_project_with_the_note_verbatim() {
        let tt = Fake::answering(0, "", "");
        let pick = pick("tt2", "  - fix: the thing ");
        submit_pick(&tt, ActivityMode::Switch, &pick, None).unwrap();
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
    fn an_unchanged_check_in_sends_nothing() {
        let tt = Fake::answering(0, "", "");
        let timer = running();
        submit_pick(
            &tt,
            ActivityMode::Checkin,
            &pick("tt1", "QRS-6805 review"),
            Some(&timer),
        )
        .unwrap();
        assert_eq!(tt.calls(), Vec::<Vec<String>>::new());
    }

    #[test]
    fn a_changed_check_in_starts_the_new_timer() {
        let tt = Fake::answering(0, "", "");
        let timer = running();
        submit_pick(
            &tt,
            ActivityMode::Checkin,
            &pick("tt1", "QRS-7000 deploy"),
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
    fn submitting_without_a_project_sends_nothing_and_is_invalid_input() {
        let tt = Fake::answering(0, "", "");
        let err = submit_pick(&tt, ActivityMode::Switch, &pick("", "review"), None).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidInput);
        assert_eq!(tt.calls(), Vec::<Vec<String>>::new());
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
            let pick = pick("tt1", "a\nb");
            let err = submit_pick(&tt, ActivityMode::Switch, &pick, None).unwrap_err();
            assert_eq!(err.kind(), kind, "{code}");
            assert_eq!(err.to_string(), message, "{code}");
        }
    }
}
