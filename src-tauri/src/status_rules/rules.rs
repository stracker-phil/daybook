//! Calendar heads-ups for the Slack status: "an absence starts within N hours". Pure.

use std::sync::LazyLock;

use chrono::{Local, NaiveDateTime, TimeDelta, TimeZone, Timelike};
use regex::Regex;

use crate::calendar::CalendarEvent;
use crate::slack::HeadsUp;
use crate::store::settings::{SlackStatus, StatusRules};

static PLACEHOLDER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\{(\w+)\}").unwrap());
static SPACES: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s{2,}").unwrap());

/// "Out of office" (by title, e.g. from the HR calendar), or a busy block of at least `min_busy_hours` (a
/// blocked day).
pub fn is_absence(event: &CalendarEvent, rules: &StatusRules) -> bool {
    let title = event.event.title.trim().to_lowercase();
    if rules
        .absence_titles
        .iter()
        .any(|wanted| wanted.trim().to_lowercase() == title)
    {
        return true;
    }
    let length = event.event.end - event.event.start;
    event.busy && length.as_seconds_f64() >= rules.min_busy_hours * 3600.0
}

/// `2pm`, `2:30pm`
pub fn format_time(time: NaiveDateTime) -> String {
    let hour = match time.hour() % 12 {
        0 => 12,
        hour => hour,
    };
    let minutes = match time.minute() {
        0 => String::new(),
        minute => format!(":{minute:02}"),
    };
    let suffix = if time.hour() < 12 { "am" } else { "pm" };
    format!("{hour}{minutes}{suffix}")
}

/// All-day events end at the next midnight; their last day is the one before.
fn last_moment(end: NaiveDateTime) -> NaiveDateTime {
    end - TimeDelta::milliseconds(1)
}

/// The absence's span in words (`from 2pm to 7pm CEST`), with the date and zone only where they say something.
pub fn describe_when(event: &CalendarEvent, now: NaiveDateTime, tz: &str) -> String {
    let (start, end) = (event.event.start, event.event.end);
    let zone = if tz.is_empty() {
        String::new()
    } else {
        format!(" {tz}")
    };
    if event.all_day {
        let (first, last) = (start.date(), last_moment(end).date());
        return if first == last {
            format!("on {first}")
        } else {
            format!("from {first} to {last}")
        };
    }
    if start.date() == last_moment(end).date() {
        let day = if start.date() == now.date() {
            String::new()
        } else {
            format!("on {} ", start.date())
        };
        return format!(
            "{day}from {} to {}{zone}",
            format_time(start),
            format_time(end)
        );
    }
    format!(
        "from {} {} to {} {}{zone}",
        start.date(),
        format_time(start),
        end.date(),
        format_time(end)
    )
}

/// Fills the placeholders; an unknown one stays as written.
fn fill(template: &str, event: &CalendarEvent, now: NaiveDateTime, tz: &str) -> String {
    let (start, end) = (event.event.start, event.event.end);
    let last_day = if event.all_day { last_moment(end) } else { end };
    let value = |key: &str| -> Option<String> {
        Some(match key {
            "when" => describe_when(event, now, tz),
            "start" => format_time(start),
            "end" => format_time(end),
            "startDate" => start.date().to_string(),
            "endDate" => last_day.date().to_string(),
            "tz" => tz.to_string(),
            _ => return None,
        })
    };
    let filled = PLACEHOLDER.replace_all(template, |caps: &regex::Captures| {
        value(&caps[1]).unwrap_or_else(|| caps[0].to_string())
    });
    SPACES.replace_all(&filled, " ").trim().to_string()
}

fn unix_seconds(time: NaiveDateTime) -> i64 {
    Local
        .from_local_datetime(&time)
        .earliest()
        .map_or(0, |local| local.timestamp())
}

/// Days to read ahead for the longest enabled rule; `None` without one.
pub fn lookahead_days(rules: &StatusRules) -> Option<u64> {
    rules
        .rules
        .iter()
        .filter(|rule| rule.enabled)
        .map(|rule| u64::from(rule.hours).div_ceil(24))
        .max()
}

/// The heads-up for the next absence that hasn't started: the enabled rule with the fewest hours that
/// reaches its start. `None` when no rule does. `zone_of` names the time zone at a time (`CEST`), unless
/// the rules set a label.
pub fn pick_heads_up(
    now: NaiveDateTime,
    events: &[CalendarEvent],
    rules: &StatusRules,
    zone_of: impl Fn(NaiveDateTime) -> String,
) -> Option<HeadsUp> {
    let next = events
        .iter()
        .filter(|event| event.takes_place() && event.event.start > now && is_absence(event, rules))
        .min_by_key(|event| event.event.start)?;
    let until = (next.event.start - now).as_seconds_f64();
    let rule = rules
        .rules
        .iter()
        .filter(|rule| rule.enabled && until <= f64::from(rule.hours) * 3600.0)
        .min_by_key(|rule| rule.hours)?;
    let tz = match rules.tz_label.trim() {
        "" => zone_of(next.event.start),
        label => label.to_string(),
    };
    Some(HeadsUp {
        label: format!("Absence within {} h", rule.hours),
        mapping: SlackStatus {
            text: fill(&rule.slack.text, next, now, &tz),
            ..rule.slack.clone()
        },
        expiration: unix_seconds(next.event.start),
    })
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;
    use crate::calendar::Event;
    use crate::store::settings::{Presence, StatusRule};

    fn at(day: u32, hour: u32, minute: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 10, day)
            .unwrap()
            .and_hms_opt(hour, minute, 0)
            .unwrap()
    }

    fn rules() -> StatusRules {
        StatusRules {
            tz_label: "CEST".into(),
            ..StatusRules::default()
        }
    }

    fn event(
        title: &str,
        start: NaiveDateTime,
        end: NaiveDateTime,
        all_day: bool,
        busy: bool,
    ) -> CalendarEvent {
        CalendarEvent {
            event: Event {
                title: title.into(),
                calendar: "Work".into(),
                start,
                end,
                url: None,
                location: None,
                notes: None,
            },
            all_day,
            canceled: false,
            declined: false,
            busy,
        }
    }

    fn all_day(from: u32, to: u32) -> CalendarEvent {
        event("Out of office", at(from, 0, 0), at(to, 0, 0), true, false)
    }

    fn block(day: u32, from: u32, to: u32) -> CalendarEvent {
        event("Blocked", at(day, from, 0), at(day, to, 0), false, true)
    }

    fn pick(now: NaiveDateTime, events: &[CalendarEvent], rules: &StatusRules) -> Option<HeadsUp> {
        pick_heads_up(now, events, rules, |_| "XYZ".into())
    }

    fn unix(time: NaiveDateTime) -> i64 {
        Local
            .from_local_datetime(&time)
            .earliest()
            .unwrap()
            .timestamp()
    }

    #[test]
    fn out_of_office_counts_by_title_ignoring_case_and_whitespace() {
        for title in ["Out of office", "  OUT OF OFFICE ", "out of Office"] {
            let absence = event(title, at(5, 0, 0), at(6, 0, 0), true, false);
            assert!(is_absence(&absence, &rules()), "{title}");
        }
    }

    #[test]
    fn long_busy_blocks_count_but_short_or_free_ones_do_not() {
        let cases = [
            (
                event("Doctor", at(1, 9, 0), at(1, 18, 0), false, true),
                true,
            ),
            (
                event("Doctor", at(1, 9, 0), at(1, 17, 0), false, true),
                true,
            ),
            (
                event("Workshop", at(1, 9, 0), at(1, 12, 0), false, true),
                false,
            ),
            (
                event("Holiday", at(1, 0, 0), at(2, 0, 0), true, false),
                false,
            ),
        ];
        for (case, expected) in cases {
            assert_eq!(
                is_absence(&case, &rules()),
                expected,
                "{}",
                case.event.title
            );
        }
    }

    #[test]
    fn the_rule_with_the_fewest_hours_that_reaches_the_absence_wins() {
        let soon = pick(at(1, 10, 0), &[block(1, 14, 23)], &rules()).unwrap();
        assert_eq!(soon.label, "Absence within 8 h");
        assert_eq!(soon.mapping.text, "Away later from 2pm to 11pm CEST");
        assert_eq!(soon.expiration, unix(at(1, 14, 0)));

        let later = pick(at(1, 10, 0), &[all_day(5, 10)], &rules()).unwrap();
        assert_eq!(later.label, "Absence within 168 h");
        assert_eq!(
            later.mapping.text,
            "Upcoming absence from 2026-10-05 to 2026-10-09"
        );
        assert_eq!(later.expiration, unix(at(5, 0, 0)));
    }

    #[test]
    fn absences_that_started_or_lie_out_of_reach_give_no_heads_up() {
        assert_eq!(pick(at(5, 10, 0), &[all_day(5, 10)], &rules()), None);
        assert_eq!(pick(at(1, 10, 0), &[all_day(12, 13)], &rules()), None);
        assert_eq!(pick(at(1, 10, 0), &[], &rules()), None);
    }

    #[test]
    fn a_disabled_rule_falls_back_to_the_next_enabled_one() {
        let mut rules = rules();
        rules.rules[0].enabled = false;
        let heads_up = pick(at(1, 10, 0), &[block(1, 14, 23)], &rules).unwrap();
        assert_eq!(heads_up.label, "Absence within 168 h");

        for rule in &mut rules.rules {
            rule.enabled = false;
        }
        assert_eq!(pick(at(1, 10, 0), &[block(1, 14, 23)], &rules), None);
    }

    #[test]
    fn canceled_and_declined_absences_are_ignored() {
        let canceled = CalendarEvent {
            canceled: true,
            ..all_day(5, 6)
        };
        let declined = CalendarEvent {
            declined: true,
            ..all_day(5, 6)
        };
        assert_eq!(pick(at(1, 10, 0), &[canceled], &rules()), None);
        assert_eq!(pick(at(1, 10, 0), &[declined], &rules()), None);
    }

    #[test]
    fn the_nearest_absence_decides_the_heads_up() {
        let events = [all_day(5, 6), block(1, 14, 23)];
        let heads_up = pick(at(1, 10, 0), &events, &rules()).unwrap();
        assert_eq!(heads_up.label, "Absence within 8 h");
    }

    #[test]
    fn an_empty_tz_label_uses_the_zone_of_the_absence_start() {
        let rules = StatusRules {
            tz_label: " ".into(),
            ..StatusRules::default()
        };
        let heads_up = pick_heads_up(at(1, 10, 0), &[block(1, 14, 23)], &rules, |start| {
            assert_eq!(start, at(1, 14, 0));
            "XYZ".into()
        })
        .unwrap();
        assert_eq!(heads_up.mapping.text, "Away later from 2pm to 11pm XYZ");
    }

    #[test]
    fn placeholders_fill_and_unknown_ones_stay_as_written() {
        let rules = StatusRules {
            rules: vec![StatusRule {
                hours: 8,
                enabled: true,
                slack: SlackStatus::new(
                    ":car:",
                    "{start}-{end} {startDate} {endDate} {tz} {nope}",
                    Presence::Away,
                ),
            }],
            ..rules()
        };
        let heads_up = pick(at(1, 10, 0), &[block(1, 14, 23)], &rules).unwrap();
        assert_eq!(
            heads_up.mapping,
            SlackStatus::new(
                ":car:",
                "2pm-11pm 2026-10-01 2026-10-01 CEST {nope}",
                Presence::Away
            )
        );
    }

    #[test]
    fn the_end_date_of_an_all_day_absence_is_its_last_day() {
        let rules = StatusRules {
            rules: vec![StatusRule {
                hours: 168,
                enabled: true,
                slack: SlackStatus::new(":x:", "{startDate} to {endDate}", Presence::Keep),
            }],
            ..rules()
        };
        let heads_up = pick(at(1, 10, 0), &[all_day(5, 10)], &rules).unwrap();
        assert_eq!(heads_up.mapping.text, "2026-10-05 to 2026-10-09");
    }

    #[test]
    fn when_is_said_in_words() {
        let now = at(1, 10, 0);
        let multi_day = event("Trip", at(2, 22, 0), at(4, 9, 30), false, true);
        let cases = [
            (block(1, 14, 19), "from 2pm to 7pm CEST"),
            (block(2, 9, 18), "on 2026-10-02 from 9am to 6pm CEST"),
            (all_day(5, 6), "on 2026-10-05"),
            (all_day(5, 10), "from 2026-10-05 to 2026-10-09"),
            (multi_day, "from 2026-10-02 10pm to 2026-10-04 9:30am CEST"),
        ];
        for (case, expected) in cases {
            assert_eq!(describe_when(&case, now, "CEST"), expected);
        }
    }

    #[test]
    fn when_has_no_zone_suffix_without_a_label() {
        assert_eq!(
            describe_when(&block(1, 14, 19), at(1, 10, 0), ""),
            "from 2pm to 7pm"
        );
    }

    #[test]
    fn times_are_formatted_on_a_twelve_hour_clock() {
        for (time, expected) in [
            (at(1, 14, 0), "2pm"),
            (at(1, 9, 30), "9:30am"),
            (at(1, 0, 0), "12am"),
            (at(1, 12, 0), "12pm"),
            (at(1, 12, 5), "12:05pm"),
            (at(1, 23, 59), "11:59pm"),
        ] {
            assert_eq!(format_time(time), expected);
        }
    }
}
