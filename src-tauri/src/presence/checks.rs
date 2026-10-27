//! Whether someone can see the screen, as plain state: away means asleep or locked. Pure.
//!
//! Nothing may open by itself while away: a window shown at the lock screen stays on screen after
//! unlocking but never gets keyboard focus, so it can't be typed into or dismissed.

use std::sync::LazyLock;
use std::time::Duration;

use regex::Regex;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Away {
    pub asleep: bool,
    pub locked: bool,
}

impl Away {
    pub fn is_away(self) -> bool {
        self.asleep || self.locked
    }
}

/// Waking is never coming back by itself: the Mac wakes at the lock screen, and macOS doesn't reliably
/// announce that lock. So the lock state is read at every wake and decides.
pub fn after_wake(screen_locked: bool) -> Away {
    Away {
        asleep: false,
        locked: screen_locked,
    }
}

pub fn is_coming_back(before: Away, after: Away) -> bool {
    before.is_away() && !after.is_away()
}

/// Timers stop while the Mac sleeps, so a check this long after the sleep means it is awake again,
/// even when no wake was announced.
const ASLEEP_AT_LEAST: Duration = Duration::from_secs(60);

pub fn has_been_asleep_long_enough(asleep_for: Duration) -> bool {
    asleep_for >= ASLEEP_AT_LEAST
}

/// An unreadable lock state counts as locked for this long; after that as unlocked, so a broken
/// `ioreg` can't keep everything that waits for the owner off for good.
const UNREADABLE_LOCKED_AT_MOST: Duration = Duration::from_secs(5 * 60);

pub fn locked_when_unreadable(unreadable_for: Duration) -> bool {
    unreadable_for < UNREADABLE_LOCKED_AT_MOST
}

static SCREEN_LOCKED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#""CGSSessionScreenIsLocked"\s*=\s*Yes"#).expect("valid regex"));

/// `ioreg -n Root -d1 -r` lists `"CGSSessionScreenIsLocked"=Yes` only while the screen is locked.
pub fn is_screen_locked_output(output: &str) -> bool {
    SCREEN_LOCKED.is_match(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASLEEP: Away = Away {
        asleep: true,
        locked: false,
    };

    #[test]
    fn the_lock_state_is_read_from_ioreg_output() {
        let session =
            r#""IOConsoleUsers" = ({"kCGSSessionOnConsoleKey"=Yes,"kCGSessionLoginDoneKey"=Yes"#;
        assert!(is_screen_locked_output(&format!(
            r#"{session},"CGSSessionScreenIsLocked"=Yes}})"#
        )));
        assert!(!is_screen_locked_output(&format!("{session}}})")));
    }

    #[test]
    fn being_away_means_asleep_or_locked() {
        for (asleep, locked, expected) in [
            (false, false, false),
            (true, false, true),
            (false, true, true),
            (true, true, true),
        ] {
            assert_eq!(
                Away { asleep, locked }.is_away(),
                expected,
                "asleep={asleep} locked={locked}"
            );
        }
    }

    #[test]
    fn a_check_a_minute_after_sleep_counts_as_awake() {
        for (secs, expected) in [(30, false), (59, false), (60, true), (61, true)] {
            assert_eq!(
                has_been_asleep_long_enough(Duration::from_secs(secs)),
                expected,
                "{secs} s"
            );
        }
    }

    #[test]
    fn an_unreadable_lock_state_counts_as_locked_for_five_minutes() {
        for (secs, expected) in [(0, true), (299, true), (300, false), (301, false)] {
            assert_eq!(
                locked_when_unreadable(Duration::from_secs(secs)),
                expected,
                "{secs} s"
            );
        }
    }

    #[test]
    fn waking_at_the_lock_screen_is_not_coming_back() {
        let at_lock_screen = after_wake(true);
        assert!(at_lock_screen.is_away());
        assert!(!is_coming_back(ASLEEP, at_lock_screen));
    }

    #[test]
    fn unlocking_after_the_wake_is_coming_back() {
        let at_lock_screen = after_wake(true);
        let unlocked = Away {
            locked: false,
            ..at_lock_screen
        };
        assert!(is_coming_back(at_lock_screen, unlocked));
    }

    #[test]
    fn waking_with_the_screen_unlocked_is_coming_back_right_away() {
        assert!(is_coming_back(ASLEEP, after_wake(false)));
    }

    #[test]
    fn a_lock_that_ends_while_still_asleep_is_not_coming_back() {
        let both = Away {
            asleep: true,
            locked: true,
        };
        let unlocked_but_asleep = Away {
            locked: false,
            ..both
        };
        assert!(!is_coming_back(both, unlocked_but_asleep));
    }
}
