//! What a status sends: the Slack status per status, channel messages, the end-of-day check and the
//! absence heads-up rules.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::read::*;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Working,
    Break,
    Off,
}

/// One value per status.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct StatusMap<T> {
    pub working: T,
    #[serde(rename = "break")]
    pub break_: T,
    pub off: T,
}

impl<T> StatusMap<T> {
    fn read(source: Option<&Value>, mut each: impl FnMut(Option<&Value>, Status) -> T) -> Self {
        let source = obj(source);
        Self {
            working: each(field(source, "working"), Status::Working),
            break_: each(field(source, "break"), Status::Break),
            off: each(field(source, "off"), Status::Off),
        }
    }

    pub fn get(&self, status: Status) -> &T {
        match status {
            Status::Working => &self.working,
            Status::Break => &self.break_,
            Status::Off => &self.off,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum Presence {
    Auto,
    Away,
    /// Presence isn't changed.
    Keep,
}

/// A Slack status to set.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct SlackStatus {
    /// The emoji and text in Slack stay as they are.
    pub keep_status: bool,
    pub emoji: String,
    pub text: String,
    pub presence: Presence,
}

impl SlackStatus {
    pub fn new(emoji: &str, text: &str, presence: Presence) -> Self {
        Self {
            keep_status: false,
            emoji: emoji.into(),
            text: text.into(),
            presence,
        }
    }

    pub fn read(raw: Option<&Value>, fallback: &Self) -> Self {
        let Some(source) = obj(raw) else {
            return fallback.clone();
        };
        Self {
            keep_status: is_true(source.get("keepStatus")),
            emoji: str(source.get("emoji")).unwrap_or_else(|| fallback.emoji.clone()),
            text: str(source.get("text")).unwrap_or_else(|| fallback.text.clone()),
            presence: one_of(source.get("presence")).unwrap_or(fallback.presence),
        }
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Slack {
    /// The channel (e.g. `#team-status`) that gets the status message on every status change; empty
    /// for none.
    pub channel: String,
    pub statuses: StatusMap<SlackStatus>,
    /// Off: picking that status leaves Slack alone (no status, no channel message).
    pub manual_workflows: StatusMap<bool>,
}

impl Default for Slack {
    fn default() -> Self {
        Self {
            channel: String::new(),
            statuses: StatusMap {
                working: SlackStatus::new(":white_check_mark:", "9:00 - 18:00", Presence::Auto),
                break_: SlackStatus::new(":negative_squared_cross_mark:", "AFK", Presence::Away),
                off: SlackStatus::new(":offline:", "Day's done", Presence::Away),
            },
            manual_workflows: StatusMap {
                working: true,
                break_: true,
                off: true,
            },
        }
    }
}

impl Slack {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        let defaults = Self::default();
        Self {
            channel: string(field(source, "channel")).unwrap_or_default(),
            statuses: StatusMap::read(field(source, "statuses"), |value, status| {
                SlackStatus::read(value, defaults.statuses.get(status))
            }),
            manual_workflows: StatusMap::read(field(source, "manualWorkflows"), |value, _| {
                on_unless_false(value)
            }),
        }
    }
}

/// The channel message per status.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct StatusMessages {
    pub working: String,
    /// The day's first Working; empty: the same as `working`.
    pub first_working: String,
    #[serde(rename = "break")]
    pub break_: String,
    pub off: String,
}

impl Default for StatusMessages {
    fn default() -> Self {
        Self {
            working: "re".into(),
            first_working: String::new(),
            break_: "afk".into(),
            off: "off for today :wave:".into(),
        }
    }
}

impl StatusMessages {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        let defaults = Self::default();
        let optional = |key, fallback: String| string(field(source, key)).unwrap_or(fallback);
        Self {
            working: optional("working", defaults.working),
            first_working: optional("firstWorking", defaults.first_working),
            break_: optional("break", defaults.break_),
            off: str(field(source, "off"))
                .filter(|text| !text.is_empty())
                .unwrap_or(defaults.off),
        }
    }
}

/// Going Off asks this first (before anything is sent); the answer shapes the channel message.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct EndOfDay {
    pub enabled: bool,
    /// Shown above the question, e.g. to post the end-of-day message.
    pub reminder: String,
    pub question: String,
    /// Opened from the check, e.g. the timesheets.
    pub url: String,
    /// The channel message for yes and no; `{message}` is the status message.
    pub done: String,
    pub incomplete: String,
}

impl Default for EndOfDay {
    fn default() -> Self {
        Self {
            enabled: true,
            reminder: String::new(),
            question: "Is today fully tracked?".into(),
            url: "https://example.com/timesheets".into(),
            done: "Tracked: Done | _{message}_".into(),
            incomplete: "Tracked: Incomplete | _{message}_".into(),
        }
    }
}

impl EndOfDay {
    pub fn read(raw: Option<&Value>) -> Self {
        let defaults = Self::default();
        let Some(source) = obj(raw) else {
            return defaults;
        };
        let optional = |key, fallback: String| {
            string(source.get(key)).map_or(fallback, |text| text.trim().to_string())
        };
        let required = |key, fallback: String| {
            Some(trimmed(source.get(key)))
                .filter(|text| !text.is_empty())
                .unwrap_or(fallback)
        };
        Self {
            enabled: on_unless_false(source.get("enabled")),
            reminder: optional("reminder", defaults.reminder),
            question: required("question", defaults.question),
            url: optional("url", defaults.url),
            done: required("done", defaults.done),
            incomplete: required("incomplete", defaults.incomplete),
        }
    }
}

const MAX_RULE_HOURS: f64 = 24.0 * 60.0;
const MAX_BUSY_HOURS: f64 = 24.0 * 14.0;

/// "An absence starts within `hours`": the enabled rule with the fewest hours that reaches it wins.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct StatusRule {
    pub hours: u32,
    pub enabled: bool,
    /// Text placeholders: `{when}` `{start}` `{end}` `{startDate}` `{endDate}` `{tz}`.
    pub slack: SlackStatus,
}

impl StatusRule {
    fn read(raw: &Value, fallback_slack: &SlackStatus) -> Option<Self> {
        let source = raw.as_object()?;
        let hours = within(number(source.get("hours")), 1.0, MAX_RULE_HOURS).map_or(0.0, round);
        (hours > 0.0).then(|| Self {
            hours: hours as u32,
            enabled: on_unless_false(source.get("enabled")),
            slack: SlackStatus::read(source.get("slack"), fallback_slack),
        })
    }
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct StatusRules {
    /// Sorted by hours.
    pub rules: Vec<StatusRule>,
    /// Events with one of these titles are absences (e.g. "Out of office").
    pub absence_titles: Vec<String>,
    /// Busy events at least this long are absences too (manually blocked days).
    pub min_busy_hours: f64,
    /// After times, e.g. CEST; empty: the Mac's time zone abbreviation.
    pub tz_label: String,
}

impl Default for StatusRules {
    fn default() -> Self {
        let heads_up = |hours, emoji, text| StatusRule {
            hours,
            enabled: true,
            slack: SlackStatus::new(emoji, text, Presence::Keep),
        };
        Self {
            rules: vec![
                heads_up(8, ":information_source:", "Away later {when}"),
                heads_up(168, ":warning:", "Upcoming absence {when}"),
            ],
            absence_titles: vec!["Out of office".into()],
            min_busy_hours: 8.0,
            tz_label: String::new(),
        }
    }
}

impl StatusRules {
    pub fn read(raw: Option<&Value>) -> Self {
        let defaults = Self::default();
        let Some(source) = obj(raw) else {
            return defaults;
        };
        let fallback_slack = defaults.rules[0].slack.clone();
        let mut rules = list(source.get("rules"), |item| {
            StatusRule::read(item, &fallback_slack)
        })
        .unwrap_or(defaults.rules);
        rules.sort_by_key(|rule| rule.hours);
        Self {
            rules,
            absence_titles: list_or(source.get("absenceTitles"), defaults.absence_titles),
            min_busy_hours: within(number(source.get("minBusyHours")), 1.0, MAX_BUSY_HOURS)
                .unwrap_or(defaults.min_busy_hours),
            tz_label: string(source.get("tzLabel"))
                .map_or(defaults.tz_label, |text| text.trim().to_string()),
        }
    }
}

/// The day's focus list in Slack: added to the first Working message, sent as a reply to it at the
/// end of the day.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Focus {
    pub enabled: bool,
    /// The first line of the end-of-day reply, e.g. EOD.
    pub end_of_day_title: String,
}

impl Default for Focus {
    fn default() -> Self {
        Self {
            enabled: true,
            end_of_day_title: "EOD".into(),
        }
    }
}

impl Focus {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        let defaults = Self::default();
        Self {
            enabled: on_unless_false(field(source, "enabled")),
            end_of_day_title: string(field(source, "endOfDayTitle"))
                .map_or(defaults.end_of_day_title, |title| title.trim().to_string()),
        }
    }
}
