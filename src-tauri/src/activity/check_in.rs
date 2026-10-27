//! When the check-in opens (docs/timetracker.md#check-ins): every `checkInMinutes` while Working in a
//! context with the Status feature, inside its work hours, not paused today and with TimeTracker
//! running; never during a call (the interval counts anew from its end) or while away; a busy overlay
//! puts it off by a minute. Pure; the scheduler ticks it.

use std::time::{Duration, Instant};

use crate::timetracker::TtState;

/// A check-in that falls due while another overlay is open waits this long.
pub const BUSY_RETRY: Duration = Duration::from_secs(60);

/// What decides whether check-ins run at all.
#[derive(Clone, Debug, PartialEq)]
pub struct Conditions {
    pub has_vault: bool,
    pub working: bool,
    pub status_feature: bool,
    /// The current context's interval; 0: none.
    pub check_in_minutes: u32,
    /// No work hours count as inside.
    pub in_work_hours: bool,
    pub paused_today: bool,
    pub tt: TtState,
}

impl Conditions {
    /// The interval, while check-ins run.
    pub fn interval(&self) -> Option<Duration> {
        let runs = self.has_vault
            && self.working
            && self.status_feature
            && self.check_in_minutes > 0
            && self.in_work_hours
            && !self.paused_today
            && self.tt == TtState::Available;
        runs.then(|| Duration::from_secs(u64::from(self.check_in_minutes) * 60))
    }
}

/// What holds a due check-in back right now.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Moment {
    pub in_call: bool,
    pub away: bool,
    pub overlay_open: bool,
}

/// When the next check-in is due; `None` until counting starts.
#[derive(Debug, Default)]
pub struct CheckIns {
    next_at: Option<Instant>,
}

impl CheckIns {
    pub fn next_at(&self) -> Option<Instant> {
        self.next_at
    }

    /// One scheduler tick; `true`: open the check-in now.
    pub fn tick(&mut self, conditions: &Conditions, moment: &Moment, now: Instant) -> bool {
        let Some(interval) = conditions.interval() else {
            self.next_at = None;
            return false;
        };
        let due = *self.next_at.get_or_insert(now + interval);
        if moment.in_call {
            self.next_at = None;
            return false;
        }
        if moment.away || now < due {
            return false;
        }
        if moment.overlay_open {
            self.next_at = Some(now + BUSY_RETRY);
            return false;
        }
        self.next_at = Some(now + interval);
        true
    }

    /// An activity was picked or confirmed, or the timer changed: a full interval from now.
    pub fn restart(&mut self, conditions: &Conditions, now: Instant) {
        self.next_at = conditions.interval().map(|interval| now + interval);
    }

    /// Counting starts again with the next tick (a pause toggled, another context).
    pub fn reset(&mut self) {
        self.next_at = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minutes(n: u64) -> Duration {
        Duration::from_secs(n * 60)
    }

    fn running() -> Conditions {
        Conditions {
            has_vault: true,
            working: true,
            status_feature: true,
            check_in_minutes: 15,
            in_work_hours: true,
            paused_today: false,
            tt: TtState::Available,
        }
    }

    fn quiet() -> Moment {
        Moment::default()
    }

    fn counting(start: Instant) -> CheckIns {
        let mut check_ins = CheckIns::default();
        assert!(!check_ins.tick(&running(), &quiet(), start));
        check_ins
    }

    #[test]
    fn counting_starts_with_the_first_tick_and_opens_nothing() {
        let start = Instant::now();
        let mut check_ins = CheckIns::default();
        assert!(!check_ins.tick(&running(), &quiet(), start));
        assert_eq!(check_ins.next_at(), Some(start + minutes(15)));
    }

    #[test]
    fn no_check_in_runs_when_a_condition_fails_and_counting_is_forgotten() {
        let cases = [
            (
                "no vault",
                Conditions {
                    has_vault: false,
                    ..running()
                },
            ),
            (
                "not working",
                Conditions {
                    working: false,
                    ..running()
                },
            ),
            (
                "no status feature",
                Conditions {
                    status_feature: false,
                    ..running()
                },
            ),
            (
                "no interval",
                Conditions {
                    check_in_minutes: 0,
                    ..running()
                },
            ),
            (
                "outside work hours",
                Conditions {
                    in_work_hours: false,
                    ..running()
                },
            ),
            (
                "paused today",
                Conditions {
                    paused_today: true,
                    ..running()
                },
            ),
            (
                "TimeTracker not running",
                Conditions {
                    tt: TtState::NotRunning,
                    ..running()
                },
            ),
            (
                "TimeTracker missing",
                Conditions {
                    tt: TtState::Missing,
                    ..running()
                },
            ),
        ];
        for (label, conditions) in cases {
            let start = Instant::now();
            let mut check_ins = counting(start);
            assert!(
                !check_ins.tick(&conditions, &quiet(), start + minutes(20)),
                "{label}"
            );
            assert_eq!(check_ins.next_at(), None, "{label}");
        }
    }

    #[test]
    fn a_check_in_waits_until_the_interval_is_over() {
        let start = Instant::now();
        let mut check_ins = counting(start);
        assert!(!check_ins.tick(&running(), &quiet(), start + minutes(14)));
        assert_eq!(check_ins.next_at(), Some(start + minutes(15)));
    }

    #[test]
    fn a_due_check_in_opens_and_the_next_is_a_full_interval_away() {
        let start = Instant::now();
        let mut check_ins = counting(start);
        assert!(check_ins.tick(&running(), &quiet(), start + minutes(16)));
        assert_eq!(check_ins.next_at(), Some(start + minutes(31)));
    }

    #[test]
    fn a_call_stops_counting_and_the_interval_restarts_from_the_first_tick_after_it() {
        let start = Instant::now();
        let mut check_ins = counting(start);
        let in_call = Moment {
            in_call: true,
            ..quiet()
        };
        assert!(!check_ins.tick(&running(), &in_call, start + minutes(20)));
        assert_eq!(check_ins.next_at(), None);
        assert!(!check_ins.tick(&running(), &quiet(), start + minutes(25)));
        assert_eq!(check_ins.next_at(), Some(start + minutes(40)));
        assert!(!check_ins.tick(&running(), &quiet(), start + minutes(39)));
        assert!(check_ins.tick(&running(), &quiet(), start + minutes(40)));
    }

    #[test]
    fn a_check_in_overdue_while_away_opens_on_the_first_tick_after_coming_back() {
        let start = Instant::now();
        let mut check_ins = counting(start);
        let away = Moment {
            away: true,
            ..quiet()
        };
        assert!(!check_ins.tick(&running(), &away, start + minutes(30)));
        assert_eq!(check_ins.next_at(), Some(start + minutes(15)));
        assert!(check_ins.tick(&running(), &quiet(), start + minutes(31)));
    }

    #[test]
    fn a_due_check_in_retries_a_minute_later_while_another_overlay_is_open() {
        let start = Instant::now();
        let mut check_ins = counting(start);
        let busy = Moment {
            overlay_open: true,
            ..quiet()
        };
        let due = start + minutes(15);
        assert!(!check_ins.tick(&running(), &busy, due));
        assert_eq!(check_ins.next_at(), Some(due + BUSY_RETRY));
        assert!(!check_ins.tick(&running(), &busy, due + Duration::from_secs(30)));
        assert_eq!(check_ins.next_at(), Some(due + BUSY_RETRY));
        assert!(check_ins.tick(&running(), &quiet(), due + BUSY_RETRY));
    }

    #[test]
    fn a_busy_overlay_does_not_matter_before_the_check_in_is_due() {
        let start = Instant::now();
        let mut check_ins = counting(start);
        let busy = Moment {
            overlay_open: true,
            ..quiet()
        };
        assert!(!check_ins.tick(&running(), &busy, start + minutes(5)));
        assert_eq!(check_ins.next_at(), Some(start + minutes(15)));
    }

    #[test]
    fn restarting_counts_a_full_interval_from_now() {
        let start = Instant::now();
        let mut check_ins = counting(start);
        check_ins.restart(&running(), start + minutes(10));
        assert_eq!(check_ins.next_at(), Some(start + minutes(25)));
        assert!(!check_ins.tick(&running(), &quiet(), start + minutes(24)));
        assert!(check_ins.tick(&running(), &quiet(), start + minutes(25)));
    }

    #[test]
    fn restarting_while_check_ins_do_not_run_counts_nothing() {
        let start = Instant::now();
        let mut check_ins = counting(start);
        let paused = Conditions {
            paused_today: true,
            ..running()
        };
        check_ins.restart(&paused, start + minutes(10));
        assert_eq!(check_ins.next_at(), None);
    }

    #[test]
    fn resetting_makes_the_next_tick_start_counting_anew() {
        let start = Instant::now();
        let mut check_ins = counting(start);
        check_ins.reset();
        assert_eq!(check_ins.next_at(), None);
        assert!(!check_ins.tick(&running(), &quiet(), start + minutes(20)));
        assert_eq!(check_ins.next_at(), Some(start + minutes(35)));
    }
}
