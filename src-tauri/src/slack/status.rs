//! Keeps Slack in line with the picked status, a running call and calendar heads-ups (`decide` sets the
//! order). Each change returns the step to take and records a call status on its way. A call's status
//! is replaced afterwards only if Slack took it; one still on its way when the call ends is replaced
//! once it lands.

use super::decide::{Decision, HeadsUp, Wanted, changes_nothing, decide, focus_of, with_project};
use crate::store::settings::{SlackStatus, Status, StatusMap};

/// A status the owner picked.
#[derive(Clone, Debug, PartialEq)]
pub struct Picked {
    pub status: Status,
    /// The confirmed channel message, for `{note}`.
    pub note: String,
    /// Only the Working status uses it.
    pub project: Option<String>,
}

/// What the owner set up and where they are now.
pub struct Setup<'a> {
    /// The current context has the Slack feature.
    pub slack_on: bool,
    pub statuses: &'a StatusMap<SlackStatus>,
    pub workflows: &'a StatusMap<bool>,
    /// Today's status, while nothing was picked since the start; none counts as Off.
    pub today: Option<Status>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Step {
    Idle,
    Send(Decision),
    /// Nothing is sent; for the log.
    Skip {
        mapping: SlackStatus,
        note: String,
        reason: &'static str,
    },
}

pub const WORKFLOW_OFF: &str = "This status's workflow is off (Settings → Workflows)";
pub const KEPT_FOR_LATER: &str = "In a call: the call status stays, this one comes back afterward";
pub const SLACK_OFF: &str =
    "Not sent: Slack is off in this context (sent if you switch to one with Slack during the call)";

#[derive(Default)]
pub struct Live {
    picked: Option<Picked>,
    /// The call status while a call runs.
    call: Option<SlackStatus>,
    /// Slack took the call status.
    call_shown: bool,
    /// The call status sent last, until Slack answered.
    call_in_flight: Option<SlackStatus>,
    heads_up: Option<HeadsUp>,
}

impl Live {
    fn picked(&self, setup: &Setup) -> Picked {
        self.picked.clone().unwrap_or(Picked {
            status: setup.today.unwrap_or(Status::Off),
            note: String::new(),
            project: None,
        })
    }

    /// Only Working names the project.
    fn wanted(&self, setup: &Setup) -> Wanted {
        let picked = self.picked(setup);
        let project = if picked.status == Status::Working {
            picked.project.as_deref()
        } else {
            None
        };
        Wanted {
            mapping: with_project(setup.statuses.get(picked.status), project),
            note: picked.note,
            workflow_on: *setup.workflows.get(picked.status),
        }
    }

    fn call_on_screen(&self) -> bool {
        self.call.is_some() && self.call_shown
    }

    /// What Slack should show now.
    fn apply(&mut self, setup: &Setup) -> Step {
        let decision = decide(
            self.call.as_ref(),
            self.heads_up.as_ref(),
            self.wanted(setup),
        );
        self.call_shown = false;
        if let Decision::Call(mapping) = &decision
            && !changes_nothing(mapping)
        {
            self.call_in_flight = Some(mapping.clone());
        }
        match decision {
            Decision::WorkflowOff { mapping, note } => Step::Skip {
                mapping,
                note,
                reason: WORKFLOW_OFF,
            },
            decision => Step::Send(decision),
        }
    }

    fn kept_for_later(&self, setup: &Setup) -> Step {
        let Wanted { mapping, note, .. } = self.wanted(setup);
        Step::Skip {
            mapping,
            note,
            reason: KEPT_FOR_LATER,
        }
    }

    /// A status the owner picked.
    pub fn status_picked(&mut self, picked: Picked, setup: &Setup) -> Step {
        self.picked = Some(picked);
        if !setup.slack_on {
            Step::Idle
        } else if self.call_on_screen() {
            self.kept_for_later(setup)
        } else {
            self.apply(setup)
        }
    }

    /// A call started or moved to another app (its trigger's status) or ended (`None`). An ended call
    /// Slack shows is replaced in every context; a move to an app with the same status sends nothing.
    pub fn call_changed(&mut self, call: Option<SlackStatus>, setup: &Setup) -> Step {
        let Some(mapping) = call else {
            let shown = self.call_on_screen();
            self.call = None;
            return if shown && self.call_in_flight.is_none() {
                self.apply(setup)
            } else {
                Step::Idle
            };
        };
        let already = self.call.as_ref() == Some(&mapping)
            && (self.call_shown || self.call_in_flight.as_ref() == Some(&mapping));
        self.call = Some(mapping.clone());
        if already {
            return Step::Idle;
        }
        if setup.slack_on {
            self.apply(setup)
        } else {
            Step::Skip {
                mapping,
                note: String::new(),
                reason: SLACK_OFF,
            }
        }
    }

    /// Whether `next` differs from the heads-up Slack heard of last.
    pub(super) fn heads_up_differs(&self, next: Option<&HeadsUp>) -> bool {
        self.heads_up.as_ref() != next
    }

    /// A heads-up started, changed or ended; Slack hears only of a change, and not over a shown call.
    pub fn heads_up_changed(&mut self, next: Option<HeadsUp>, setup: &Setup) -> Step {
        let changed = self.heads_up_differs(next.as_ref());
        self.heads_up = next;
        if !changed || !setup.slack_on || self.call_on_screen() {
            Step::Idle
        } else {
            self.apply(setup)
        }
    }

    /// The timer's project changed; Slack hears of it only while Working and when its text changes
    /// (Internal reads as none).
    pub fn project_changed(&mut self, project: Option<String>, setup: &Setup) -> Step {
        let current = self.picked(setup);
        let same_focus = focus_of(current.project.as_deref()) == focus_of(project.as_deref());
        let working = current.status == Status::Working;
        self.picked = Some(Picked { project, ..current });
        if same_focus || !setup.slack_on || !working {
            Step::Idle
        } else if self.call_on_screen() {
            self.kept_for_later(setup)
        } else {
            self.apply(setup)
        }
    }

    /// A call already running shows as soon as the new context has Slack.
    pub fn context_changed(&mut self, setup: &Setup) -> Step {
        if self.call.is_some() && !self.call_shown && setup.slack_on {
            self.apply(setup)
        } else {
            Step::Idle
        }
    }

    /// Slack took the call status `sent`. It counts as shown while it is still the call's status;
    /// otherwise the call ended or moved on its way, and what is wanted now replaces it. An older send
    /// landing changes nothing: the newer one settles it.
    pub fn call_sent(&mut self, sent: &SlackStatus, setup: &Setup) -> Step {
        if self.call_in_flight.as_ref() != Some(sent) {
            return Step::Idle;
        }
        self.call_in_flight = None;
        if self.call.as_ref() == Some(sent) {
            self.call_shown = true;
            Step::Idle
        } else {
            self.apply(setup)
        }
    }

    /// The call status `sent` never reached Slack (failed, replaced or not connected).
    pub fn call_not_sent(&mut self, sent: &SlackStatus) {
        self.call_in_flight.take_if(|in_flight| *in_flight == *sent);
    }

    /// After a restart without a call: Slack still shows a call status from before, so what is wanted
    /// now replaces it, in every context.
    pub fn restore_after_restart(&mut self, setup: &Setup) -> Step {
        if self.call.is_some() || self.call_in_flight.is_some() {
            Step::Idle
        } else {
            self.apply(setup)
        }
    }

    /// Daybook quits: a call status Slack shows, or that is on its way, is replaced by what is wanted
    /// without the call. `None` when nothing is to be sent.
    pub fn quitting(&mut self, setup: &Setup) -> Option<Decision> {
        if !self.call_on_screen() && self.call_in_flight.is_none() {
            return None;
        }
        self.call = None;
        self.call_in_flight = None;
        match self.apply(setup) {
            Step::Send(decision) => Some(decision),
            Step::Idle | Step::Skip { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::settings::{Presence, Settings};

    struct World {
        statuses: StatusMap<SlackStatus>,
        workflows: StatusMap<bool>,
    }

    impl World {
        fn new() -> Self {
            let slack = Settings::default().slack;
            Self {
                statuses: slack.statuses,
                workflows: slack.manual_workflows,
            }
        }

        fn setup(&self, slack_on: bool) -> Setup<'_> {
            Setup {
                slack_on,
                statuses: &self.statuses,
                workflows: &self.workflows,
                today: None,
            }
        }
    }

    fn picked(status: Status, note: &str) -> Picked {
        Picked {
            status,
            note: note.into(),
            project: None,
        }
    }

    fn call() -> SlackStatus {
        SlackStatus::new(":headphones:", "In a call", Presence::Away)
    }

    fn heads_up() -> HeadsUp {
        HeadsUp {
            label: "Absence within 8 h".into(),
            mapping: SlackStatus::new(":information_source:", "Away later", Presence::Away),
            expiration: 1_790_000_000,
        }
    }

    fn status_step(mapping: &SlackStatus, note: &str) -> Step {
        Step::Send(Decision::Status {
            mapping: mapping.clone(),
            note: note.into(),
        })
    }

    #[test]
    fn picking_a_status_sends_its_mapping_with_the_message() {
        let world = World::new();
        let step =
            Live::default().status_picked(picked(Status::Break, "lunch"), &world.setup(true));
        assert_eq!(step, status_step(&world.statuses.break_, "lunch"));
    }

    #[test]
    fn picking_a_status_with_its_workflow_off_skips() {
        let mut world = World::new();
        world.workflows.break_ = false;
        let step =
            Live::default().status_picked(picked(Status::Break, "lunch"), &world.setup(true));
        assert_eq!(
            step,
            Step::Skip {
                mapping: world.statuses.break_.clone(),
                note: "lunch".into(),
                reason: WORKFLOW_OFF,
            }
        );
    }

    #[test]
    fn picking_a_status_outside_a_slack_context_does_nothing() {
        let world = World::new();
        let step = Live::default().status_picked(picked(Status::Break, ""), &world.setup(false));
        assert_eq!(step, Step::Idle);
    }

    #[test]
    fn a_call_switches_the_status_to_the_call_status_and_back() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = Live::default();
        live.status_picked(picked(Status::Working, ""), &setup);

        assert_eq!(
            live.call_changed(Some(call()), &setup),
            Step::Send(Decision::Call(call()))
        );
        live.call_sent(&call(), &setup);

        assert_eq!(
            live.status_picked(picked(Status::Break, "coffee"), &setup),
            Step::Skip {
                mapping: world.statuses.break_.clone(),
                note: "coffee".into(),
                reason: KEPT_FOR_LATER,
            }
        );
        assert_eq!(
            live.call_changed(None, &setup),
            status_step(&world.statuses.break_, "coffee")
        );
    }

    #[test]
    fn the_end_of_a_call_restores_the_last_picked_status() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = Live::default();
        live.status_picked(picked(Status::Working, ""), &setup);
        live.call_changed(Some(call()), &setup);
        live.call_sent(&call(), &setup);
        assert_eq!(
            live.call_changed(None, &setup),
            status_step(&world.statuses.working, "")
        );
    }

    #[test]
    fn a_call_status_slack_never_took_is_not_replaced_when_the_call_ends() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = Live::default();
        live.call_changed(Some(call()), &setup);
        assert_eq!(live.call_changed(None, &setup), Step::Idle);
    }

    #[test]
    fn picking_during_a_call_slack_never_took_still_sends_the_call() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = Live::default();
        live.call_changed(Some(call()), &setup);
        assert_eq!(
            live.status_picked(picked(Status::Break, ""), &setup),
            Step::Send(Decision::Call(call()))
        );
    }

    #[test]
    fn a_shown_call_is_replaced_when_it_ends_outside_a_slack_context() {
        let world = World::new();
        let mut live = Live::default();
        live.status_picked(picked(Status::Working, ""), &world.setup(true));
        live.call_changed(Some(call()), &world.setup(true));
        live.call_sent(&call(), &world.setup(true));
        assert_eq!(
            live.call_changed(None, &world.setup(false)),
            status_step(&world.statuses.working, "")
        );
    }

    #[test]
    fn a_call_outside_a_slack_context_is_sent_once_a_slack_context_is_entered() {
        let world = World::new();
        let mut live = Live::default();
        assert_eq!(
            live.call_changed(Some(call()), &world.setup(false)),
            Step::Skip {
                mapping: call(),
                note: String::new(),
                reason: SLACK_OFF,
            }
        );
        assert_eq!(
            live.context_changed(&world.setup(true)),
            Step::Send(Decision::Call(call()))
        );
        live.call_sent(&call(), &world.setup(true));
        assert_eq!(live.context_changed(&world.setup(true)), Step::Idle);
    }

    #[test]
    fn changing_context_without_a_call_does_nothing() {
        let world = World::new();
        assert_eq!(
            Live::default().context_changed(&world.setup(true)),
            Step::Idle
        );
    }

    #[test]
    fn a_heads_up_is_sent_once_and_replaced_when_it_ends() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = Live::default();
        live.status_picked(picked(Status::Working, ""), &setup);

        assert_eq!(
            live.heads_up_changed(Some(heads_up()), &setup),
            Step::Send(Decision::HeadsUp {
                mapping: heads_up().mapping,
                expiration: 1_790_000_000,
                detail: "Status rule: Absence within 8 h".into(),
            })
        );
        assert_eq!(live.heads_up_changed(Some(heads_up()), &setup), Step::Idle);
        assert_eq!(
            live.heads_up_changed(None, &setup),
            status_step(&world.statuses.working, "")
        );
    }

    #[test]
    fn a_heads_up_differs_until_slack_heard_of_it() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = Live::default();
        assert!(!live.heads_up_differs(None));
        assert!(live.heads_up_differs(Some(&heads_up())));

        live.heads_up_changed(Some(heads_up()), &setup);
        assert!(!live.heads_up_differs(Some(&heads_up())));
        assert!(live.heads_up_differs(None));
        let later = HeadsUp {
            expiration: 1_790_000_600,
            ..heads_up()
        };
        assert!(live.heads_up_differs(Some(&later)));
    }

    #[test]
    fn a_heads_up_does_not_interrupt_a_shown_call() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = Live::default();
        live.call_changed(Some(call()), &setup);
        live.call_sent(&call(), &setup);
        assert_eq!(live.heads_up_changed(Some(heads_up()), &setup), Step::Idle);
    }

    #[test]
    fn a_heads_up_outside_a_slack_context_is_not_sent() {
        let world = World::new();
        assert_eq!(
            Live::default().heads_up_changed(Some(heads_up()), &world.setup(false)),
            Step::Idle
        );
    }

    #[test]
    fn without_a_pick_the_status_of_today_is_shown() {
        let world = World::new();
        let setup = Setup {
            today: Some(Status::Working),
            ..world.setup(true)
        };
        let mut live = Live::default();
        live.call_changed(Some(call()), &setup);
        live.call_sent(&call(), &setup);
        assert_eq!(
            live.call_changed(None, &setup),
            status_step(&world.statuses.working, "")
        );
    }

    #[test]
    fn without_a_pick_or_a_status_today_the_status_is_off() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = Live::default();
        live.call_changed(Some(call()), &setup);
        live.call_sent(&call(), &setup);
        assert_eq!(
            live.call_changed(None, &setup),
            status_step(&world.statuses.off, "")
        );
    }

    #[test]
    fn a_call_status_sent_after_the_call_changed_is_not_counted_as_shown() {
        let world = World::new();
        let setup = world.setup(true);
        let other = SlackStatus::new(":phone:", "On the phone", Presence::Away);
        let mut live = Live::default();
        live.call_changed(Some(call()), &setup);
        live.call_changed(None, &setup);
        live.call_changed(Some(other), &setup);
        live.call_sent(&call(), &setup);
        assert_eq!(live.call_changed(None, &setup), Step::Idle);
    }

    fn working_on(project: Option<&str>) -> Picked {
        Picked {
            status: Status::Working,
            note: String::new(),
            project: project.map(String::from),
        }
    }

    fn working_named(world: &World, text: &str) -> Step {
        status_step(
            &SlackStatus {
                text: text.into(),
                ..world.statuses.working.clone()
            },
            "",
        )
    }

    #[test]
    fn a_project_after_internal_is_sent_while_working() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = Live::default();
        live.status_picked(working_on(Some("Internal")), &setup);

        assert_eq!(
            live.project_changed(Some("Oscorp".into()), &setup),
            working_named(&world, "9:00 - 18:00 | Oscorp")
        );
    }

    #[test]
    fn the_same_project_again_sends_nothing() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = Live::default();
        live.status_picked(working_on(Some("Oscorp")), &setup);

        assert_eq!(
            live.project_changed(Some("Oscorp".into()), &setup),
            Step::Idle
        );
    }

    #[test]
    fn a_project_change_sends_nothing_outside_working_or_without_slack() {
        let world = World::new();
        let mut on_break = Live::default();
        on_break.status_picked(picked(Status::Break, ""), &world.setup(true));
        assert_eq!(
            on_break.project_changed(Some("Oscorp".into()), &world.setup(true)),
            Step::Idle
        );

        let mut no_slack = Live::default();
        no_slack.status_picked(working_on(None), &world.setup(false));
        assert_eq!(
            no_slack.project_changed(Some("Oscorp".into()), &world.setup(false)),
            Step::Idle
        );
    }

    #[test]
    fn a_project_change_during_a_shown_call_waits_until_the_call_ends() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = Live::default();
        live.status_picked(working_on(None), &setup);
        live.call_changed(Some(call()), &setup);
        live.call_sent(&call(), &setup);

        assert_eq!(
            live.project_changed(Some("Oscorp".into()), &setup),
            Step::Skip {
                mapping: SlackStatus {
                    text: "9:00 - 18:00 | Oscorp".into(),
                    ..world.statuses.working.clone()
                },
                note: String::new(),
                reason: KEPT_FOR_LATER,
            }
        );
        assert_eq!(
            live.call_changed(None, &setup),
            working_named(&world, "9:00 - 18:00 | Oscorp")
        );
        assert_eq!(
            live.status_picked(working_on(None), &setup),
            working_named(&world, "9:00 - 18:00")
        );
    }

    fn other_call() -> SlackStatus {
        SlackStatus::new(":phone:", "On the phone", Presence::Away)
    }

    fn keep_everything() -> SlackStatus {
        SlackStatus {
            keep_status: true,
            ..SlackStatus::new("", "", Presence::Keep)
        }
    }

    fn working_live(setup: &Setup) -> Live {
        let mut live = Live::default();
        live.status_picked(picked(Status::Working, ""), setup);
        live
    }

    #[test]
    fn a_call_moving_to_an_app_with_another_status_sends_only_that_status() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = working_live(&setup);
        live.call_changed(Some(call()), &setup);
        live.call_sent(&call(), &setup);

        assert_eq!(
            live.call_changed(Some(other_call()), &setup),
            Step::Send(Decision::Call(other_call()))
        );
    }

    #[test]
    fn a_call_moving_to_an_app_with_the_same_status_sends_nothing() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = working_live(&setup);
        live.call_changed(Some(call()), &setup);
        live.call_sent(&call(), &setup);

        assert_eq!(live.call_changed(Some(call()), &setup), Step::Idle);
    }

    #[test]
    fn a_call_ending_while_its_status_is_on_its_way_is_replaced_once_it_lands() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = working_live(&setup);
        live.call_changed(Some(call()), &setup);

        assert_eq!(live.call_changed(None, &setup), Step::Idle);
        assert_eq!(
            live.call_sent(&call(), &setup),
            status_step(&world.statuses.working, "")
        );
    }

    #[test]
    fn an_older_call_status_landing_changes_nothing() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = working_live(&setup);
        live.call_changed(Some(call()), &setup);
        live.call_changed(Some(other_call()), &setup);

        assert_eq!(live.call_sent(&call(), &setup), Step::Idle);
        assert_eq!(live.call_sent(&other_call(), &setup), Step::Idle);
        assert_eq!(
            live.call_changed(None, &setup),
            status_step(&world.statuses.working, "")
        );
    }

    #[test]
    fn a_call_status_that_never_reached_slack_is_not_replaced_when_the_call_ends() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = working_live(&setup);
        live.call_changed(Some(call()), &setup);
        live.call_not_sent(&call());

        assert_eq!(live.call_sent(&call(), &setup), Step::Idle);
        assert_eq!(live.call_changed(None, &setup), Step::Idle);
    }

    #[test]
    fn a_call_status_that_never_reached_slack_is_sent_again_with_the_next_change() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = working_live(&setup);
        live.call_changed(Some(call()), &setup);
        live.call_not_sent(&call());

        assert_eq!(
            live.call_changed(Some(call()), &setup),
            Step::Send(Decision::Call(call()))
        );
    }

    #[test]
    fn a_call_status_that_changes_nothing_is_never_tracked_as_sent() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = working_live(&setup);
        assert_eq!(
            live.call_changed(Some(keep_everything()), &setup),
            Step::Send(Decision::Call(keep_everything()))
        );

        assert_eq!(live.call_sent(&keep_everything(), &setup), Step::Idle);
        assert_eq!(live.quitting(&setup), None);
        assert_eq!(live.call_changed(None, &setup), Step::Idle);
    }

    #[test]
    fn a_restart_without_a_call_replaces_what_slack_shows() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = working_live(&setup);
        assert_eq!(
            live.restore_after_restart(&setup),
            status_step(&world.statuses.working, "")
        );
    }

    #[test]
    fn a_restart_during_a_call_or_with_a_send_in_flight_changes_nothing() {
        let world = World::new();
        let setup = world.setup(true);

        let mut in_call = working_live(&setup);
        in_call.call_changed(Some(call()), &setup);
        in_call.call_sent(&call(), &setup);
        assert_eq!(in_call.restore_after_restart(&setup), Step::Idle);

        let mut in_flight = working_live(&setup);
        in_flight.call_changed(Some(call()), &setup);
        in_flight.call_changed(None, &setup);
        assert_eq!(in_flight.restore_after_restart(&setup), Step::Idle);
    }

    #[test]
    fn quitting_during_a_shown_call_restores_the_wanted_status() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = working_live(&setup);
        live.call_changed(Some(call()), &setup);
        live.call_sent(&call(), &setup);

        assert_eq!(
            live.quitting(&setup),
            Some(Decision::Status {
                mapping: world.statuses.working.clone(),
                note: String::new(),
            })
        );
    }

    #[test]
    fn quitting_while_the_call_status_is_on_its_way_restores_the_wanted_status() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = working_live(&setup);
        live.call_changed(Some(call()), &setup);

        assert_eq!(
            live.quitting(&setup),
            Some(Decision::Status {
                mapping: world.statuses.working.clone(),
                note: String::new(),
            })
        );
    }

    #[test]
    fn quitting_without_a_call_status_on_slack_sends_nothing() {
        let world = World::new();
        let mut idle = working_live(&world.setup(true));
        assert_eq!(idle.quitting(&world.setup(true)), None);

        let mut outside_slack = Live::default();
        outside_slack.call_changed(Some(call()), &world.setup(false));
        assert_eq!(outside_slack.quitting(&world.setup(true)), None);
    }

    #[test]
    fn quitting_sends_nothing_when_the_workflow_of_the_wanted_status_is_off() {
        let mut world = World::new();
        world.workflows.break_ = false;
        let setup = world.setup(true);
        let mut live = Live::default();
        live.status_picked(picked(Status::Break, ""), &setup);
        live.call_changed(Some(call()), &setup);
        live.call_sent(&call(), &setup);

        assert_eq!(live.quitting(&setup), None);
    }

    #[test]
    fn only_working_names_the_project() {
        let world = World::new();
        let setup = world.setup(true);
        let with_project = |status| Picked {
            status,
            note: String::new(),
            project: Some("Oscorp".into()),
        };

        let Step::Send(Decision::Status { mapping, .. }) =
            Live::default().status_picked(with_project(Status::Working), &setup)
        else {
            panic!("expected a status to send");
        };
        assert_eq!(mapping.text, "9:00 - 18:00 | Oscorp");

        assert_eq!(
            Live::default().status_picked(with_project(Status::Break), &setup),
            status_step(&world.statuses.break_, "")
        );
    }
}
