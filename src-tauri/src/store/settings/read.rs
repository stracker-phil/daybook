//! Readers for untrusted JSON (a hand-edited `settings.json`, the page, the prototype's files): each
//! returns `None` or an empty value instead of failing, so a broken part falls back to its default.

use std::collections::HashSet;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

pub type Obj = Map<String, Value>;

pub fn obj(value: Option<&Value>) -> Option<&Obj> {
    value.and_then(Value::as_object)
}

pub fn field<'a>(source: Option<&'a Obj>, key: &str) -> Option<&'a Value> {
    source.and_then(|source| source.get(key))
}

/// A string, or a number as text.
pub fn str(value: Option<&Value>) -> Option<String> {
    match value? {
        Value::String(text) => Some(text.clone()),
        Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

pub fn text(value: Option<&Value>) -> String {
    str(value).unwrap_or_default()
}

pub fn trimmed(value: Option<&Value>) -> String {
    text(value).trim().to_string()
}

/// Only an actual string (no numbers), for fields that keep their fallback otherwise.
pub fn string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(String::from)
}

/// The non-empty trimmed items of a list; anything else is an empty list.
pub fn strings(value: Option<&Value>) -> Vec<String> {
    items(value)
        .iter()
        .map(|item| trimmed(Some(item)))
        .filter(|item| !item.is_empty())
        .collect()
}

/// Each string once, in order.
pub fn distinct(list: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    list.into_iter()
        .filter(|item| seen.insert(item.clone()))
        .collect()
}

pub fn items(value: Option<&Value>) -> &[Value] {
    value
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
}

/// The trimmed strings of a list, or `fallback` when it isn't one.
pub fn list_or(value: Option<&Value>, fallback: Vec<String>) -> Vec<String> {
    if is_list(value) {
        strings(value)
    } else {
        fallback
    }
}

pub fn is_list(value: Option<&Value>) -> bool {
    value.is_some_and(Value::is_array)
}

/// Parses every item, dropping the invalid ones; `None` when it isn't a list (the caller keeps its
/// fallback), so an empty list stays empty.
pub fn list<T>(value: Option<&Value>, parse: impl FnMut(&Value) -> Option<T>) -> Option<Vec<T>> {
    is_list(value).then(|| items(value).iter().filter_map(parse).collect())
}

pub fn flag(value: Option<&Value>) -> Option<bool> {
    value.and_then(Value::as_bool)
}

/// On unless stored as `false`.
pub fn on_unless_false(value: Option<&Value>) -> bool {
    flag(value) != Some(false)
}

pub fn is_true(value: Option<&Value>) -> bool {
    flag(value) == Some(true)
}

/// A JSON number (not text).
pub fn number(value: Option<&Value>) -> Option<f64> {
    value.and_then(Value::as_f64).filter(|n| n.is_finite())
}

/// What JavaScript's `Number()` makes of a value: text is parsed, `null` and `""` are 0, a bool is 0
/// or 1, a missing value is not a number.
pub fn coerced_number(value: Option<&Value>) -> Option<f64> {
    match value? {
        Value::Number(number) => number.as_f64(),
        Value::String(text) if text.trim().is_empty() => Some(0.0),
        Value::String(text) => text.trim().parse::<f64>().ok(),
        Value::Bool(on) => Some(f64::from(u8::from(*on))),
        Value::Null => Some(0.0),
        _ => None,
    }
    .filter(|n| n.is_finite())
}

/// A number within `min..=max`, else `None`.
pub fn within(number: Option<f64>, min: f64, max: f64) -> Option<f64> {
    number.filter(|n| (min..=max).contains(n))
}

/// A value of a string enum (`#[serde(rename_all)]` decides the spelling), else `None`.
pub fn one_of<T: DeserializeOwned>(value: Option<&Value>) -> Option<T> {
    value
        .filter(|value| value.is_string())
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}

/// Any serde value, else `None`.
pub fn parsed<T: DeserializeOwned>(value: Option<&Value>) -> Option<T> {
    value.and_then(|value| serde_json::from_value(value.clone()).ok())
}

/// Distinct weekdays (0 = Sunday … 6), sorted.
pub fn weekdays(value: Option<&Value>) -> Vec<u8> {
    let mut days: Vec<u8> = items(value)
        .iter()
        .filter_map(Value::as_f64)
        .filter(|day| day.fract() == 0.0 && (0.0..=6.0).contains(day))
        .map(|day| day as u8)
        .collect();
    days.sort_unstable();
    days.dedup();
    days
}

/// `"9:45"` → minutes after midnight; `None` when it isn't a time of day.
pub fn parse_clock(value: &str) -> Option<u32> {
    let (hours, minutes) = value.trim().split_once(':')?;
    let digits = |part: &str, len: std::ops::RangeInclusive<usize>| {
        (len.contains(&part.len()) && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse::<u32>().ok())
            .flatten()
    };
    let (hours, minutes) = (digits(hours, 1..=2)?, digits(minutes, 2..=2)?);
    (hours <= 23 && minutes <= 59).then_some(hours * 60 + minutes)
}

pub fn format_clock(minutes: u32) -> String {
    format!("{:02}:{:02}", minutes / 60, minutes % 60)
}

/// `"9:30"` → `"09:30"`; `None` for anything that isn't a time of day.
pub fn clock(value: Option<&Value>) -> Option<String> {
    string(value)
        .as_deref()
        .and_then(parse_clock)
        .map(format_clock)
}

/// `"Evening Wrap"` → `"evening-wrap"`; `fallback` when nothing is left.
pub fn slug(name: &str, fallback: &str) -> String {
    let mut slug = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        fallback.into()
    } else {
        slug.into()
    }
}

/// The stored id or one made from the name, unique within one list (`work`, `work-2`).
pub fn unique_id(source: &Obj, name: &str, seen: &mut HashSet<String>, fallback: &str) -> String {
    let mut id = trimmed(source.get("id"));
    if id.is_empty() {
        id = slug(name, fallback);
    }
    while seen.contains(&id) {
        id.push_str("-2");
    }
    seen.insert(id.clone());
    id
}

/// Letters, digits, `_` and `-` only, and at least one of them.
pub fn is_word(text: &str) -> bool {
    !text.is_empty()
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// JavaScript's `Math.round`: halves round up.
pub fn round(number: f64) -> f64 {
    (number + 0.5).floor()
}
