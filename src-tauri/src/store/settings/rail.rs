//! The rail at the screen edge: where it shows, how it looks, its tooltips.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use ts_rs::TS;

use super::read::*;

/// Which screen edges show the rail; with both it opens on the edge the pointer came to.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum RailEdges {
    Right,
    Left,
    Both,
}

/// During a call the rail sits on one edge.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum RailCallEdge {
    Right,
    Left,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum RailCallPosition {
    Center,
    Top,
    Bottom,
}

/// Where on the edge the rail appears; `Pointer` centres it on where the pointer was.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum RailPosition {
    Center,
    Top,
    Bottom,
    Pointer,
}

/// The edge counts along its full height, or only beside where the rail appears.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum RailSpan {
    Full,
    Rail,
}

/// A pill floats beside the edge, docked sits flush against it, floating has no bar.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum RailShape {
    Pill,
    Docked,
    Floating,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum RailSize {
    Compact,
    Regular,
    Large,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum RailTheme {
    System,
    Light,
    Dark,
}

/// How the hovered button stands out.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum RailHighlight {
    Tinted,
    Filled,
    Gray,
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Rail {
    /// Resting the pointer at the edge shows it; the shortcut works either way.
    pub enabled: bool,
    pub edges: RailEdges,
    /// Stays visible while a call is detected, at `call_edge` and `call_position`.
    pub show_during_meetings: bool,
    pub call_edge: RailCallEdge,
    pub call_position: RailCallPosition,
    /// Points from the screen edge that count as the edge.
    pub trigger_width: u32,
    pub trigger_span: RailSpan,
    /// Milliseconds the pointer rests at the edge before the rail shows.
    pub show_delay: u32,
    /// Milliseconds the pointer stays away before the rail hides.
    pub hide_delay: u32,
    pub shape: RailShape,
    pub size: RailSize,
    pub theme: RailTheme,
    pub highlight: RailHighlight,
    /// Short lines under the icons.
    pub captions: bool,
    pub position: RailPosition,
    /// A rail that stays (shortcut, meeting) fades back while the pointer is away from it.
    pub fade_when_idle: bool,
    /// The first tooltip's wait in milliseconds; moving along the rail then shows them at once.
    pub tooltip_delay: u32,
    /// Tools with choices list them in their tooltip.
    pub hover_menus: bool,
}

impl Default for Rail {
    fn default() -> Self {
        Self {
            enabled: true,
            edges: RailEdges::Right,
            show_during_meetings: true,
            call_edge: RailCallEdge::Right,
            call_position: RailCallPosition::Center,
            trigger_width: 2,
            trigger_span: RailSpan::Full,
            show_delay: 200,
            hide_delay: 700,
            shape: RailShape::Pill,
            size: RailSize::Regular,
            theme: RailTheme::System,
            highlight: RailHighlight::Tinted,
            captions: true,
            position: RailPosition::Center,
            fade_when_idle: false,
            tooltip_delay: 300,
            hover_menus: true,
        }
    }
}

/// A number clamped into `min..=max` and rounded; anything else is `fallback`.
fn whole(value: Option<&Value>, min: f64, max: f64, fallback: u32) -> u32 {
    number(value).map_or(fallback, |n| round(n.clamp(min, max)) as u32)
}

impl Rail {
    /// Older files are read as they were meant: `side` became `edges`, the "slim" shape is a compact
    /// pill without captions, "borderless" became floating.
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        let d = Self::default();
        let get = |key| field(source, key);
        let (shape, slim) = match get("shape").and_then(Value::as_str) {
            Some("borderless") => (Some(RailShape::Floating), false),
            Some("slim") => (None, true),
            _ => (one_of(get("shape")), false),
        };
        Self {
            enabled: flag(get("enabled")).unwrap_or(d.enabled),
            edges: one_of(get("edges"))
                .or_else(|| one_of(get("side")))
                .unwrap_or(d.edges),
            show_during_meetings: flag(get("showDuringMeetings")).unwrap_or(d.show_during_meetings),
            call_edge: one_of(get("callEdge")).unwrap_or(d.call_edge),
            call_position: one_of(get("callPosition")).unwrap_or(d.call_position),
            trigger_width: whole(get("triggerWidth"), 1.0, 40.0, d.trigger_width),
            trigger_span: one_of(get("triggerSpan")).unwrap_or(d.trigger_span),
            show_delay: whole(get("showDelay"), 0.0, 2000.0, d.show_delay),
            hide_delay: whole(get("hideDelay"), 100.0, 5000.0, d.hide_delay),
            shape: shape.unwrap_or(d.shape),
            size: one_of(get("size")).unwrap_or(if slim { RailSize::Compact } else { d.size }),
            theme: one_of(get("theme")).unwrap_or(d.theme),
            highlight: one_of(get("highlight")).unwrap_or(d.highlight),
            captions: flag(get("captions")).unwrap_or(!slim && d.captions),
            position: one_of(get("position")).unwrap_or(d.position),
            fade_when_idle: flag(get("fadeWhenIdle")).unwrap_or(d.fade_when_idle),
            tooltip_delay: whole(get("tooltipDelay"), 0.0, 2000.0, d.tooltip_delay),
            hover_menus: flag(get("hoverMenus")).unwrap_or(d.hover_menus),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn missing_or_broken_values_fall_back_to_the_defaults() {
        assert_eq!(Rail::read(None), Rail::default());
        let broken = json!({ "shape": "hexagon", "triggerWidth": "wide" });
        assert_eq!(Rail::read(Some(&broken)), Rail::default());
    }

    #[test]
    fn an_older_file_keeps_its_edge_and_look() {
        let old = json!({ "enabled": false, "side": "left", "shape": "slim" });
        let rail = Rail::read(Some(&old));
        assert!(!rail.enabled);
        assert_eq!(rail.edges, RailEdges::Left);
        assert_eq!(rail.shape, RailShape::Pill);
        assert_eq!(rail.size, RailSize::Compact);
        assert!(!rail.captions);

        let borderless = json!({ "shape": "borderless" });
        assert_eq!(Rail::read(Some(&borderless)).shape, RailShape::Floating);
    }

    #[test]
    fn a_call_has_its_own_edge_and_place_right_and_centered_unless_set() {
        let rail = Rail::read(None);
        assert_eq!(rail.call_edge, RailCallEdge::Right);
        assert_eq!(rail.call_position, RailCallPosition::Center);

        let set = json!({
            "edges": "both",
            "position": "pointer",
            "callEdge": "left",
            "callPosition": "top",
        });
        let rail = Rail::read(Some(&set));
        assert_eq!(rail.edges, RailEdges::Both);
        assert_eq!(rail.position, RailPosition::Pointer);
        assert_eq!(rail.call_edge, RailCallEdge::Left);
        assert_eq!(rail.call_position, RailCallPosition::Top);

        let invalid = json!({ "callEdge": "both", "callPosition": "pointer" });
        let rail = Rail::read(Some(&invalid));
        assert_eq!(rail.call_edge, RailCallEdge::Right);
        assert_eq!(rail.call_position, RailCallPosition::Center);
    }

    #[test]
    fn numbers_stay_in_range() {
        let raw = json!({
            "triggerWidth": 500,
            "showDelay": -10,
            "hideDelay": 20,
            "tooltipDelay": 12.6,
        });
        let rail = Rail::read(Some(&raw));
        assert_eq!(rail.trigger_width, 40);
        assert_eq!(rail.show_delay, 0);
        assert_eq!(rail.hide_delay, 100);
        assert_eq!(rail.tooltip_delay, 13);
    }
}
