//! A call that drops for longer than the grace (a reconnect, breakout rooms) and comes back in the same
//! app within `RESTART_WITHIN` is one call for TimeTracker: an ended call waits that long before it is
//! suggested, and a restart joins it. Pure.

use chrono::{NaiveDateTime, TimeDelta};

use crate::calls::ActiveCall;
use crate::calls::checks::GENERIC_TITLE;
use crate::store::settings::Status;

const RESTART_WITHIN: TimeDelta = TimeDelta::minutes(2);

#[derive(Clone, Debug, PartialEq)]
pub struct EndedCall {
    pub call: ActiveCall,
    pub end: NaiveDateTime,
    pub status: Option<Status>,
}

#[derive(Debug, Default)]
pub struct SplitCalls {
    waiting: Option<EndedCall>,
    /// The earlier part of the call running now.
    continues: Option<ActiveCall>,
}

/// The earlier part's start and, unless it was just "Meeting", its title.
fn joined(first: &ActiveCall, later: &ActiveCall) -> ActiveCall {
    let title = if first.title == GENERIC_TITLE {
        &later.title
    } else {
        &first.title
    };
    ActiveCall {
        since: first.since,
        title: title.clone(),
        ..later.clone()
    }
}

impl SplitCalls {
    /// A call ended (joined to its earlier part): it waits. Returns a call that waited before and is
    /// final now.
    pub fn ended(&mut self, mut ended: EndedCall) -> Option<EndedCall> {
        if let Some(first) = self.continues.take()
            && first.bundle_id == ended.call.bundle_id
        {
            ended.call = joined(&first, &ended.call);
        }
        self.waiting.replace(ended)
    }

    /// A call started: the same app's call that ended at most `RESTART_WITHIN` before goes on in it;
    /// any other waiting call is final and returned.
    pub fn started(&mut self, call: &ActiveCall) -> Option<EndedCall> {
        let waiting = self.waiting.take()?;
        if waiting.call.bundle_id == call.bundle_id && call.since - waiting.end <= RESTART_WITHIN {
            self.continues = Some(waiting.call);
            None
        } else {
            Some(waiting)
        }
    }

    pub fn due_at(&self) -> Option<NaiveDateTime> {
        self.waiting
            .as_ref()
            .map(|waiting| waiting.end + RESTART_WITHIN)
    }

    /// The waiting call, once `RESTART_WITHIN` passed after its end without a restart.
    pub fn due(&mut self, now: NaiveDateTime) -> Option<EndedCall> {
        if self.due_at().is_some_and(|due_at| now >= due_at) {
            self.waiting.take()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn at(hour: u32, minute: u32, second: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, 27)
            .unwrap()
            .and_hms_opt(hour, minute, second)
            .unwrap()
    }

    fn call(bundle_id: &str, title: &str, since: NaiveDateTime) -> ActiveCall {
        ActiveCall {
            app: "App".into(),
            bundle_id: bundle_id.into(),
            title: title.into(),
            since,
        }
    }

    fn ended(call: ActiveCall, end: NaiveDateTime, status: Option<Status>) -> EndedCall {
        EndedCall { call, end, status }
    }

    #[test]
    fn nothing_is_due_without_an_ended_call() {
        let mut split = SplitCalls::default();

        assert_eq!(split.due_at(), None);
        assert_eq!(split.due(at(12, 0, 0)), None);
        assert_eq!(split.started(&call("zoom", "Sync", at(10, 0, 0))), None);
    }

    #[test]
    fn an_ended_call_waits_two_minutes_before_it_is_final() {
        let mut split = SplitCalls::default();
        let first = ended(
            call("zoom", "Sync", at(10, 0, 0)),
            at(10, 30, 0),
            Some(Status::Working),
        );
        assert_eq!(split.ended(first.clone()), None);

        assert_eq!(split.due_at(), Some(at(10, 32, 0)));
        for (now, is_due) in [
            (at(10, 30, 0), false),
            (at(10, 31, 59), false),
            (at(10, 32, 0), true),
        ] {
            let mut copy = SplitCalls::default();
            copy.ended(first.clone());
            assert_eq!(copy.due(now).is_some(), is_due, "{now}");
        }
        assert_eq!(split.due(at(10, 40, 0)), Some(first));
        assert_eq!(split.due_at(), None);
        assert_eq!(split.due(at(10, 41, 0)), None);
    }

    #[test]
    fn the_same_app_restarting_within_two_minutes_joins_the_parts() {
        let mut split = SplitCalls::default();
        split.ended(ended(
            call("zoom", "Oscorp Weekly", at(10, 0, 0)),
            at(10, 30, 0),
            Some(Status::Working),
        ));

        assert_eq!(split.started(&call("zoom", "Other", at(10, 32, 0))), None);
        assert_eq!(split.due_at(), None);

        let waiting = split.ended(ended(
            call("zoom", "Meeting", at(10, 32, 0)),
            at(11, 0, 0),
            Some(Status::Break),
        ));

        assert_eq!(waiting, None);
        assert_eq!(
            split.due(at(11, 2, 0)),
            Some(ended(
                call("zoom", "Oscorp Weekly", at(10, 0, 0)),
                at(11, 0, 0),
                Some(Status::Break),
            ))
        );
    }

    #[test]
    fn a_generic_first_title_gives_way_to_the_later_one() {
        let mut split = SplitCalls::default();
        split.ended(ended(
            call("teams", GENERIC_TITLE, at(10, 0, 0)),
            at(10, 30, 0),
            None,
        ));
        split.started(&call("teams", "Budget", at(10, 31, 0)));

        split.ended(ended(
            call("teams", "Budget", at(10, 31, 0)),
            at(11, 0, 0),
            Some(Status::Working),
        ));

        assert_eq!(
            split.due(at(11, 2, 0)),
            Some(ended(
                call("teams", "Budget", at(10, 0, 0)),
                at(11, 0, 0),
                Some(Status::Working),
            ))
        );
    }

    #[test]
    fn a_restart_exactly_two_minutes_later_still_joins() {
        let mut split = SplitCalls::default();
        split.ended(ended(call("zoom", "A", at(10, 0, 0)), at(10, 30, 0), None));

        assert_eq!(split.started(&call("zoom", "A", at(10, 32, 0))), None);
    }

    #[test]
    fn a_restart_after_two_minutes_returns_the_waiting_call() {
        let mut split = SplitCalls::default();
        let first = ended(
            call("zoom", "A", at(10, 0, 0)),
            at(10, 30, 0),
            Some(Status::Working),
        );
        split.ended(first.clone());

        assert_eq!(
            split.started(&call("zoom", "A", at(10, 32, 1))),
            Some(first)
        );
        assert_eq!(split.due_at(), None);
    }

    #[test]
    fn another_app_starting_returns_the_waiting_call() {
        let mut split = SplitCalls::default();
        let first = ended(call("zoom", "A", at(10, 0, 0)), at(10, 30, 0), None);
        split.ended(first.clone());

        assert_eq!(
            split.started(&call("teams", "B", at(10, 31, 0))),
            Some(first)
        );
    }

    #[test]
    fn a_call_of_another_app_does_not_join_an_earlier_part() {
        let mut split = SplitCalls::default();
        split.ended(ended(call("zoom", "A", at(10, 0, 0)), at(10, 30, 0), None));
        split.started(&call("zoom", "A", at(10, 31, 0)));

        let teams = ended(
            call("teams", "B", at(10, 31, 0)),
            at(11, 0, 0),
            Some(Status::Working),
        );

        assert_eq!(split.ended(teams.clone()), None);
        assert_eq!(split.due(at(11, 2, 0)), Some(teams));
    }

    #[test]
    fn an_ended_call_while_another_waits_returns_the_older_one() {
        let mut split = SplitCalls::default();
        let older = ended(call("zoom", "A", at(10, 0, 0)), at(10, 30, 0), None);
        let newer = ended(call("teams", "B", at(10, 40, 0)), at(11, 0, 0), None);
        split.ended(older.clone());

        assert_eq!(split.ended(newer.clone()), Some(older));
        assert_eq!(split.due(at(11, 2, 0)), Some(newer));
    }
}
