//! The Stage hook protocol's pure parts (docs/stage.md): constants, rectangles in screen points,
//! window bounds from a request, the narrator's flat state and the discovery record.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use ts_rs::TS;

use crate::automation::{AutomationError, Params};

pub use crate::automation::{required, trimmed};

pub const PROTOCOL: u32 = 1;

/// Optional protocol capabilities Daybook implements, listed by `describe`.
pub const FEATURES: [&str; 8] = [
    "reveal",
    "visible",
    "sections",
    "ui",
    "type.append",
    "keys.window",
    "scroll",
    "wait",
];

const MAX_STATE_KEYS: usize = 30;

pub type StageResult<T> = Result<T, AutomationError>;

/// CSS pixels from the page's top-left, as a page measures them.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts", rename = "StageRect")]
pub struct PageRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Whole screen points, origin at the main display's top-left.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ScreenRect {
    pub x: i64,
    pub y: i64,
    pub width: i64,
    pub height: i64,
}

/// A page rectangle in screen points: the window's content origin is added, then rounded.
pub fn to_screen_rect(rect: PageRect, content: ScreenRect) -> ScreenRect {
    ScreenRect {
        x: (content.x as f64 + rect.x).round() as i64,
        y: (content.y as f64 + rect.y).round() as i64,
        width: rect.width.round() as i64,
        height: rect.height.round() as i64,
    }
}

/// `x`, `y`, `width`, `height` from a request: only those given, whole numbers.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Bounds {
    pub x: Option<i64>,
    pub y: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
}

impl Bounds {
    pub fn is_empty(&self) -> bool {
        *self == Bounds::default()
    }

    /// The given values over `current`.
    pub fn over(&self, current: ScreenRect) -> ScreenRect {
        ScreenRect {
            x: self.x.unwrap_or(current.x),
            y: self.y.unwrap_or(current.y),
            width: self.width.unwrap_or(current.width),
            height: self.height.unwrap_or(current.height),
        }
    }
}

/// A whole number in any JSON-ish spelling ("10", "10.0", "1e3"); None otherwise.
pub fn parse_whole(raw: &str) -> Option<i64> {
    raw.parse::<f64>()
        .ok()
        .filter(|v| v.is_finite() && v.fract() == 0.0)
        .map(|v| v as i64)
}

pub fn parse_bounds(params: &Params) -> StageResult<Bounds> {
    let mut bounds = Bounds::default();
    for key in ["x", "y", "width", "height"] {
        let Some(raw) = trimmed(params, key) else {
            continue;
        };
        let value = parse_whole(raw)
            .ok_or_else(|| AutomationError::bad(format!("“{key}” must be a whole number.")))?;
        let positive = || {
            (value > 0)
                .then_some(value)
                .ok_or_else(|| AutomationError::bad(format!("“{key}” must be positive.")))
        };
        match key {
            "x" => bounds.x = Some(value),
            "y" => bounds.y = Some(value),
            "width" => bounds.width = Some(positive()?),
            _ => bounds.height = Some(positive()?),
        }
    }
    Ok(bounds)
}

/// State for a narrator: strings, numbers, booleans and nulls as they are, lists of strings or
/// numbers as comma-separated text; objects are left out; at most 30 keys, in order.
pub fn flat_state(state: Map<String, Value>) -> Map<String, Value> {
    let mut flat = Map::new();
    for (key, value) in state {
        if flat.len() >= MAX_STATE_KEYS {
            break;
        }
        let value = match value {
            Value::Array(items) => {
                let words: Option<Vec<String>> = items
                    .iter()
                    .map(|item| match item {
                        Value::String(s) => Some(s.clone()),
                        Value::Number(n) => Some(n.to_string()),
                        _ => None,
                    })
                    .collect();
                match words {
                    Some(words) => Value::String(words.join(", ")),
                    None => continue,
                }
            }
            Value::Object(_) => continue,
            other => other,
        };
        flat.insert(key, value);
    }
    flat
}

pub fn stage_url(port: u16) -> String {
    format!("{}/stage", crate::http_api::base_url(port))
}

/// The terminal command that registers the hook with Stage (protocol §3.1); tokens are hex, so
/// nothing needs quoting.
pub fn register_command(port: u16, token: &str) -> String {
    format!("stage register {} --token {token}", stage_url(port))
}

/// The legacy discovery file's content (protocol §3.2), keys in the documented order.
#[derive(Serialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryRecord {
    pub protocol: u32,
    pub name: String,
    pub id: String,
    pub url: String,
    pub token: String,
    pub pid: u32,
    /// RFC 3339 in UTC with milliseconds.
    pub updated_at: String,
}

pub fn discovery_record(
    name: &str,
    id: &str,
    port: u16,
    token: &str,
    pid: u32,
    now: chrono::DateTime<chrono::Utc>,
) -> DiscoveryRecord {
    DiscoveryRecord {
        protocol: PROTOCOL,
        name: name.into(),
        id: id.into(),
        url: stage_url(port),
        token: token.into(),
        pid,
        updated_at: now.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use serde_json::json;

    use super::*;

    fn params(items: &[(&str, &str)]) -> Params {
        items
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    fn object(value: Value) -> Map<String, Value> {
        value.as_object().unwrap().clone()
    }

    fn keys_of(map: &Map<String, Value>) -> Vec<&str> {
        map.keys().map(String::as_str).collect()
    }

    #[test]
    fn a_page_rectangle_moves_to_the_content_origin_and_rounds() {
        let rect = PageRect {
            x: 16.4,
            y: 100.6,
            width: 208.2,
            height: 32.0,
        };
        let content = ScreenRect {
            x: 120,
            y: 88,
            width: 1240,
            height: 692,
        };

        assert_eq!(
            to_screen_rect(rect, content),
            ScreenRect {
                x: 136,
                y: 189,
                width: 208,
                height: 32,
            }
        );
    }

    #[test]
    fn bounds_read_only_the_values_given() {
        let bounds = parse_bounds(&params(&[("x", "10"), ("width", "800")])).unwrap();

        assert_eq!(
            bounds,
            Bounds {
                x: Some(10),
                y: None,
                width: Some(800),
                height: None,
            }
        );
        assert!(!bounds.is_empty());
    }

    #[test]
    fn no_bounds_given_is_empty() {
        let bounds = parse_bounds(&params(&[("window", "hub"), ("x", "  ")])).unwrap();

        assert_eq!(bounds, Bounds::default());
        assert!(bounds.is_empty());
    }

    #[test]
    fn a_bound_that_is_not_a_whole_number_is_a_bad_request() {
        for (key, value) in [
            ("y", "abc"),
            ("x", "1.5"),
            ("width", "inf"),
            ("height", "NaN"),
        ] {
            assert_eq!(
                parse_bounds(&params(&[(key, value)])),
                Err(AutomationError::bad(format!(
                    "“{key}” must be a whole number."
                ))),
                "{key}={value}"
            );
        }
    }

    #[test]
    fn a_size_must_be_positive_but_a_position_may_be_negative() {
        for (key, value) in [("width", "0"), ("height", "0"), ("height", "-5")] {
            assert_eq!(
                parse_bounds(&params(&[(key, value)])),
                Err(AutomationError::bad(format!("“{key}” must be positive."))),
                "{key}={value}"
            );
        }
        assert_eq!(
            parse_bounds(&params(&[("x", "-40"), ("y", "0")])),
            Ok(Bounds {
                x: Some(-40),
                y: Some(0),
                ..Bounds::default()
            })
        );
    }

    #[test]
    fn bounds_replace_only_the_values_given() {
        let current = ScreenRect {
            x: 100,
            y: 200,
            width: 900,
            height: 700,
        };
        let bounds = Bounds {
            x: Some(10),
            height: Some(500),
            ..Bounds::default()
        };

        assert_eq!(
            bounds.over(current),
            ScreenRect {
                x: 10,
                y: 200,
                width: 900,
                height: 500,
            }
        );
        assert_eq!(Bounds::default().over(current), current);
    }

    #[test]
    fn a_required_parameter_is_trimmed_and_missing_ones_are_bad_requests() {
        let given = params(&[("a", "  x  "), ("blank", "   ")]);

        assert_eq!(required(&given, "a"), Ok("x"));
        for key in ["blank", "absent"] {
            assert_eq!(
                required(&given, key),
                Err(AutomationError::bad(format!("“{key}” is missing."))),
                "{key}"
            );
        }
        assert_eq!(trimmed(&given, "blank"), None);
    }

    #[test]
    fn the_narrators_state_is_flat_and_keeps_its_order() {
        let state = object(json!({
            "status": "working",
            "since": 1700000000000u64,
            "vault": true,
            "call": null,
            "modes": ["work", "private"],
            "nested": { "a": 1 },
        }));

        let flat = flat_state(state);

        assert_eq!(
            keys_of(&flat),
            ["status", "since", "vault", "call", "modes"]
        );
        assert_eq!(
            Value::Object(flat),
            json!({
                "status": "working",
                "since": 1700000000000u64,
                "vault": true,
                "call": null,
                "modes": "work, private",
            })
        );
    }

    #[test]
    fn a_list_with_anything_but_strings_and_numbers_is_left_out() {
        let flat = flat_state(object(json!({
            "numbers": [1, 2],
            "mixed": ["a", true],
            "objects": [{ "a": 1 }],
            "empty": [],
        })));

        assert_eq!(
            Value::Object(flat),
            json!({ "numbers": "1, 2", "empty": "" })
        );
    }

    #[test]
    fn the_narrators_state_keeps_the_first_thirty_keys() {
        let many: Map<String, Value> = (0..40).map(|i| (format!("k{i}"), json!(i))).collect();

        let flat = flat_state(many);

        let expected: Vec<String> = (0..30).map(|i| format!("k{i}")).collect();
        assert_eq!(keys_of(&flat), expected);
    }

    #[test]
    fn the_stage_url_and_register_command_name_the_port_and_token() {
        assert_eq!(stage_url(27180), "http://127.0.0.1:27180/v1/stage");
        assert_eq!(
            register_command(27180, "abc"),
            "stage register http://127.0.0.1:27180/v1/stage --token abc"
        );
    }

    #[test]
    fn the_discovery_record_has_the_documented_keys_in_order() {
        let now = Utc.with_ymd_and_hms(2026, 10, 15, 10, 0, 0).unwrap();

        let record = discovery_record("Daybook", "app.example", 27180, "t", 42, now);

        assert_eq!(
            serde_json::to_string(&record).unwrap(),
            r#"{"protocol":1,"name":"Daybook","id":"app.example","url":"http://127.0.0.1:27180/v1/stage","token":"t","pid":42,"updatedAt":"2026-10-15T10:00:00.000Z"}"#
        );
    }

    #[test]
    fn the_features_list_what_the_hook_implements() {
        assert_eq!(PROTOCOL, 1);
        assert_eq!(
            FEATURES,
            [
                "reveal",
                "visible",
                "sections",
                "ui",
                "type.append",
                "keys.window",
                "scroll",
                "wait",
            ]
        );
    }
}
