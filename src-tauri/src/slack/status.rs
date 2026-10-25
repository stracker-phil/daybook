//! Keeps Slack in line with the picked status, a running call and calendar heads-ups (`decide` sets the
//! order). Pure: each change returns the step to take. A call's status is replaced afterwards only if
//! Slack took it.

use super::decide::{Decision, HeadsUp, Wanted, decide, with_project};
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

    /// A call started (its trigger's status) or ended (`None`). An ended call that Slack shows is
    /// replaced in every context.
    pub fn call_changed(&mut self, call: Option<SlackStatus>, setup: &Setup) -> Step {
        let Some(mapping) = call else {
            let shown = self.call_on_screen();
            self.call = None;
            return if shown { self.apply(setup) } else { Step::Idle };
        };
        self.call = Some(mapping.clone());
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

    /// A heads-up started, changed or ended; Slack hears only of a change, and not over a shown call.
    pub fn heads_up_changed(&mut self, next: Option<HeadsUp>, setup: &Setup) -> Step {
        let changed = next != self.heads_up;
        self.heads_up = next;
        if !changed || !setup.slack_on || self.call_on_screen() {
            Step::Idle
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

    /// Slack took `sent`; it counts as shown while it is still the call's status.
    pub fn call_sent(&mut self, sent: &SlackStatus) {
        self.call_shown = self.call.as_ref() == Some(sent);
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
        live.call_sent(&call());

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
        live.call_sent(&call());
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
        live.call_sent(&call());
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
        live.call_sent(&call());
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
    fn a_heads_up_does_not_interrupt_a_shown_call() {
        let world = World::new();
        let setup = world.setup(true);
        let mut live = Live::default();
        live.call_changed(Some(call()), &setup);
        live.call_sent(&call());
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
        live.call_sent(&call());
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
        live.call_sent(&call());
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
        live.call_sent(&call());
        assert_eq!(live.call_changed(None, &setup), Step::Idle);
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
