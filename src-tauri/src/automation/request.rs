//! An action's name and parameters read into a request. Pure.

use std::collections::BTreeMap;

use crate::error::{AppError, ErrorKind};
use crate::store::settings::{Presence, Status};

/// Parameters by name, as strings (query, form, JSON primitives or link query).
pub type Params = BTreeMap<String, String>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    State,
    Show,
    Status,
    Slack,
    Command,
}

/// Every action with its name, in the order errors list them.
const ACTIONS: [(Action, &str); 5] = [
    (Action::State, "state"),
    (Action::Show, "show"),
    (Action::Status, "status"),
    (Action::Slack, "slack"),
    (Action::Command, "command"),
];

impl Action {
    pub fn name(self) -> &'static str {
        ACTIONS
            .iter()
            .find(|(action, _)| *action == self)
            .map_or("", |(_, name)| name)
    }

    pub fn parse(name: &str) -> Result<Self, AutomationError> {
        ACTIONS
            .iter()
            .find(|(_, known)| *known == name)
            .map(|(action, _)| *action)
            .ok_or_else(|| {
                let names: Vec<&str> = ACTIONS.iter().map(|(_, name)| *name).collect();
                AutomationError::new(
                    404,
                    format!("Unknown action “{name}”. Actions: {}.", names.join(", ")),
                )
            })
    }

    /// `command` checks the vault per command instead.
    pub fn needs_vault(self) -> bool {
        matches!(self, Action::Status | Action::Slack)
    }
}

/// Why an action failed; `status` is the HTTP status.
#[derive(Clone, Debug, PartialEq)]
pub struct AutomationError {
    pub status: u16,
    pub kind: ErrorKind,
    pub message: String,
}

impl AutomationError {
    pub fn new(status: u16, message: impl Into<String>) -> Self {
        let kind = match status {
            404 => ErrorKind::NotFound,
            400..=499 => ErrorKind::InvalidInput,
            _ => ErrorKind::Io,
        };
        Self {
            status,
            kind,
            message: message.into(),
        }
    }

    pub fn bad(message: impl Into<String>) -> Self {
        Self::new(400, message)
    }

    pub fn no_vault() -> Self {
        Self::new(409, "Choose your vault in the app’s Settings first.")
    }
}

/// Anything that fails while running.
impl From<AppError> for AutomationError {
    fn from(error: AppError) -> Self {
        Self {
            status: 422,
            kind: error.kind(),
            message: error.to_string(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    State,
    Show,
    Status {
        status: Status,
        note: String,
    },
    /// Slack as that status sets it; Daybook's own status stays.
    SlackLike {
        status: Status,
        note: String,
    },
    SlackCustom {
        emoji: String,
        text: String,
        presence: Presence,
    },
    Command {
        id: String,
    },
}

fn value<'a>(params: &'a Params, key: &str) -> Option<&'a str> {
    params.get(key).map(|v| v.trim()).filter(|v| !v.is_empty())
}

fn text(params: &Params, key: &str) -> String {
    value(params, key).unwrap_or_default().to_string()
}

fn status(params: &Params) -> Result<Status, AutomationError> {
    match value(params, "status").map(str::to_lowercase).as_deref() {
        Some("working") => Ok(Status::Working),
        Some("break") => Ok(Status::Break),
        Some("off") => Ok(Status::Off),
        _ => Err(AutomationError::bad(
            "“status” must be working, break or off.",
        )),
    }
}

fn presence(params: &Params) -> Result<Presence, AutomationError> {
    match value(params, "presence").map(str::to_lowercase).as_deref() {
        None | Some("auto") => Ok(Presence::Auto),
        Some("away") => Ok(Presence::Away),
        Some(_) => Err(AutomationError::bad("“presence” must be auto or away.")),
    }
}

fn required(params: &Params, key: &str) -> Result<String, AutomationError> {
    value(params, key)
        .map(String::from)
        .ok_or_else(|| AutomationError::bad(format!("“{key}” is missing.")))
}

impl Request {
    pub fn read(action: Action, params: &Params) -> Result<Self, AutomationError> {
        Ok(match action {
            Action::State => Request::State,
            Action::Show => Request::Show,
            Action::Status => Request::Status {
                status: status(params)?,
                note: text(params, "note"),
            },
            Action::Slack if value(params, "status").is_some() => Request::SlackLike {
                status: status(params)?,
                note: text(params, "note"),
            },
            Action::Slack => Request::SlackCustom {
                emoji: text(params, "emoji"),
                text: text(params, "text"),
                presence: presence(params)?,
            },
            Action::Command => Request::Command {
                id: required(params, "id")?,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(items: &[(&str, &str)]) -> Params {
        items
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn read(action: &str, items: &[(&str, &str)]) -> Result<Request, AutomationError> {
        Request::read(Action::parse(action)?, &params(items))
    }

    #[test]
    fn every_action_is_found_by_its_name() {
        for (name, action) in [
            ("state", Action::State),
            ("show", Action::Show),
            ("status", Action::Status),
            ("slack", Action::Slack),
            ("command", Action::Command),
        ] {
            assert_eq!(Action::parse(name), Ok(action), "{name}");
            assert_eq!(action.name(), name);
        }
    }

    #[test]
    fn an_unknown_action_is_not_found_and_lists_the_actions() {
        for name in ["activity", "State", ""] {
            let e = Action::parse(name).unwrap_err();

            assert_eq!(e.status, 404, "{name:?}");
            assert_eq!(e.kind, ErrorKind::NotFound, "{name:?}");
            assert_eq!(
                e.message,
                format!("Unknown action “{name}”. Actions: state, show, status, slack, command."),
                "{name:?}"
            );
        }
    }

    #[test]
    fn only_status_and_slack_need_a_vault() {
        for (action, needs) in [
            (Action::State, false),
            (Action::Show, false),
            (Action::Status, true),
            (Action::Slack, true),
            (Action::Command, false),
        ] {
            assert_eq!(action.needs_vault(), needs, "{}", action.name());
        }
    }

    #[test]
    fn errors_get_a_kind_from_their_status() {
        for (status, kind) in [
            (400, ErrorKind::InvalidInput),
            (409, ErrorKind::InvalidInput),
            (404, ErrorKind::NotFound),
            (500, ErrorKind::Io),
            (503, ErrorKind::Io),
        ] {
            assert_eq!(AutomationError::new(status, "x").kind, kind, "{status}");
        }
        assert_eq!(AutomationError::bad("x").status, 400);
        let no_vault = AutomationError::no_vault();
        assert_eq!(no_vault.status, 409);
        assert_eq!(
            no_vault.message,
            "Choose your vault in the app’s Settings first."
        );
    }

    #[test]
    fn an_app_error_becomes_unprocessable_with_its_kind_and_message() {
        let e = AutomationError::from(AppError::NotFound("No such thing".into()));

        assert_eq!(e.status, 422);
        assert_eq!(e.kind, ErrorKind::NotFound);
        assert_eq!(e.message, "No such thing");
    }

    #[test]
    fn state_and_show_take_no_parameters() {
        assert_eq!(read("state", &[("junk", "1")]), Ok(Request::State));
        assert_eq!(read("show", &[]), Ok(Request::Show));
    }

    #[test]
    fn status_reads_the_status_and_a_trimmed_note() {
        assert_eq!(
            read("status", &[("status", "break"), ("note", "  Lunch  ")]),
            Ok(Request::Status {
                status: Status::Break,
                note: "Lunch".into(),
            })
        );
        assert_eq!(
            read("status", &[("status", "off")]),
            Ok(Request::Status {
                status: Status::Off,
                note: String::new(),
            })
        );
    }

    #[test]
    fn a_status_is_read_without_regard_to_case_and_padding() {
        for (input, expected) in [
            ("working", Status::Working),
            ("Break", Status::Break),
            (" OFF ", Status::Off),
        ] {
            assert_eq!(
                read("status", &[("status", input)]),
                Ok(Request::Status {
                    status: expected,
                    note: String::new(),
                }),
                "{input}"
            );
        }
    }

    #[test]
    fn a_missing_or_unknown_status_is_a_bad_request() {
        for items in [
            vec![],
            vec![("status", "")],
            vec![("status", "  ")],
            vec![("status", "lunch")],
        ] {
            assert_eq!(
                read("status", &items),
                Err(AutomationError::bad(
                    "“status” must be working, break or off."
                )),
                "{items:?}"
            );
        }
    }

    #[test]
    fn slack_with_a_status_sets_slack_like_that_status() {
        assert_eq!(
            read(
                "slack",
                &[
                    ("status", "working"),
                    ("note", "Deep work"),
                    ("emoji", ":x:")
                ]
            ),
            Ok(Request::SlackLike {
                status: Status::Working,
                note: "Deep work".into(),
            })
        );
    }

    #[test]
    fn slack_with_an_unknown_status_is_a_bad_request_not_a_custom_status() {
        assert_eq!(
            read("slack", &[("status", "nope"), ("emoji", ":x:")]),
            Err(AutomationError::bad(
                "“status” must be working, break or off."
            ))
        );
    }

    #[test]
    fn slack_without_a_status_sets_a_custom_status() {
        assert_eq!(
            read("slack", &[("emoji", ":tomato:"), ("text", " Focus ")]),
            Ok(Request::SlackCustom {
                emoji: ":tomato:".into(),
                text: "Focus".into(),
                presence: Presence::Auto,
            })
        );
    }

    #[test]
    fn a_blank_status_selects_the_custom_form() {
        assert_eq!(
            read("slack", &[("status", " "), ("text", "Hi")]),
            Ok(Request::SlackCustom {
                emoji: String::new(),
                text: "Hi".into(),
                presence: Presence::Auto,
            })
        );
    }

    #[test]
    fn slack_without_any_parameter_clears_the_status() {
        assert_eq!(
            read("slack", &[]),
            Ok(Request::SlackCustom {
                emoji: String::new(),
                text: String::new(),
                presence: Presence::Auto,
            })
        );
    }

    #[test]
    fn the_presence_is_auto_or_away_without_regard_to_case() {
        for (input, expected) in [
            ("auto", Presence::Auto),
            ("AWAY", Presence::Away),
            ("away", Presence::Away),
            ("", Presence::Auto),
        ] {
            let Ok(Request::SlackCustom { presence, .. }) = read("slack", &[("presence", input)])
            else {
                panic!("{input}");
            };
            assert_eq!(presence, expected, "{input}");
        }
    }

    #[test]
    fn an_unknown_presence_is_a_bad_request() {
        assert_eq!(
            read("slack", &[("presence", "busy")]),
            Err(AutomationError::bad("“presence” must be auto or away."))
        );
    }

    #[test]
    fn command_reads_the_trimmed_id() {
        assert_eq!(
            read("command", &[("id", " journal:open ")]),
            Ok(Request::Command {
                id: "journal:open".into(),
            })
        );
    }

    #[test]
    fn a_command_without_an_id_is_a_bad_request() {
        for items in [vec![], vec![("id", "")], vec![("id", "   ")]] {
            assert_eq!(
                read("command", &items),
                Err(AutomationError::bad("“id” is missing.")),
                "{items:?}"
            );
        }
    }
}
