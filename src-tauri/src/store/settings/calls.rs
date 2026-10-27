//! Calls: which apps count as being in a meeting, and what joining a call starts.

use serde::Serialize;
use serde_json::Value;
use ts_rs::TS;

use super::read::*;
use super::slack::{Presence, SlackStatus};

pub const ZOOM_BUNDLE: &str = "us.zoom.xos";
pub const TEAMS_BUNDLE: &str = "com.microsoft.teams2";
pub const MEET_BUNDLE: &str = "com.google.Chrome.app.kjgfgldnnfoeklkmfkjfagphfepbbdan";

fn meeting_status(text: &str) -> SlackStatus {
    SlackStatus::new(":spiral_calendar_pad:", text, Presence::Auto)
}

/// An app is in a meeting while it runs and, with `window_title`, has a window whose title contains
/// it, and with `needs_microphone`, records from the microphone. The first match wins.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct MeetingRule {
    pub enabled: bool,
    /// Shown in the log and the call status, e.g. Zoom.
    pub app: String,
    pub bundle_id: String,
    pub window_title: Option<String>,
    /// Only while the app records from the microphone; built-in checks ignore it. Left out of the
    /// JSON while off, so older pages and files stay valid.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    #[ts(as = "Option<bool>", optional)]
    pub needs_microphone: bool,
    pub slack: SlackStatus,
}

impl MeetingRule {
    fn built_in(app: &str, bundle_id: &str, service: &str) -> Self {
        Self {
            enabled: true,
            app: app.into(),
            bundle_id: bundle_id.into(),
            window_title: None,
            needs_microphone: false,
            slack: meeting_status(&format!("In a {service} call")),
        }
    }

    fn read(raw: &Value) -> Option<Self> {
        let source = raw.as_object()?;
        let app = trimmed(source.get("app"));
        let bundle_id = trimmed(source.get("bundleId"));
        if app.is_empty() || bundle_id.is_empty() {
            return None;
        }
        let fallback = meeting_status(&format!("In a {app} meeting"));
        Some(Self {
            enabled: on_unless_false(source.get("enabled")),
            window_title: Some(trimmed(source.get("windowTitle"))).filter(|t| !t.is_empty()),
            needs_microphone: is_true(source.get("needsMicrophone")),
            slack: SlackStatus::read(source.get("slack"), &fallback),
            app,
            bundle_id,
        })
    }
}

/// The default app triggers; Settings offers any of them that were removed.
pub fn built_in_call_apps() -> Vec<MeetingRule> {
    vec![
        MeetingRule::built_in("Zoom", ZOOM_BUNDLE, "Zoom"),
        MeetingRule::built_in("Teams", TEAMS_BUNDLE, "Teams"),
        MeetingRule::built_in("Meet", MEET_BUNDLE, "Google Meet"),
    ]
}

#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Meetings {
    pub detection: bool,
    pub rules: Vec<MeetingRule>,
}

impl Default for Meetings {
    fn default() -> Self {
        Self {
            detection: true,
            rules: built_in_call_apps(),
        }
    }
}

impl Meetings {
    pub fn read(raw: Option<&Value>) -> Self {
        let source = obj(raw);
        let defaults = Self::default();
        Self {
            detection: flag(field(source, "detection")).unwrap_or(defaults.detection),
            rules: list(field(source, "rules"), MeetingRule::read).unwrap_or(defaults.rules),
        }
    }
}

/// An app picked in Settings: its name to show and the bundle id that opens it.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct AppRef {
    pub app: String,
    pub bundle_id: String,
}

impl AppRef {
    fn new(app: &str, bundle_id: &str) -> Self {
        Self {
            app: app.into(),
            bundle_id: bundle_id.into(),
        }
    }

    fn read(raw: &Value) -> Option<Self> {
        let source = raw.as_object()?;
        let bundle_id = trimmed(source.get("bundleId"));
        if bundle_id.is_empty() {
            return None;
        }
        let app = Some(trimmed(source.get("app")))
            .filter(|app| !app.is_empty())
            .unwrap_or_else(|| bundle_id.clone());
        Some(Self { app, bundle_id })
    }
}

/// The app that opens each service's join links; `None`: the app macOS has for the link.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
pub struct OpenWith {
    #[serde(rename = "Zoom")]
    pub zoom: Option<AppRef>,
    #[serde(rename = "Teams")]
    pub teams: Option<AppRef>,
    #[serde(rename = "Meet")]
    pub meet: Option<AppRef>,
}

/// What joining a call from Daybook does.
#[derive(Serialize, Clone, Debug, PartialEq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "camelCase")]
pub struct Joining {
    /// Started in the background first, e.g. a virtual camera.
    pub start_first: Vec<AppRef>,
    pub open_with: OpenWith,
}

impl Default for Joining {
    fn default() -> Self {
        Self {
            start_first: vec![AppRef::new("Camo", "com.reincubate.macos.cam")],
            open_with: OpenWith {
                zoom: None,
                teams: None,
                meet: Some(AppRef::new("Google Meet", MEET_BUNDLE)),
            },
        }
    }
}

impl Joining {
    /// A service left out keeps its default; one stored as anything but an app opens with macOS.
    pub fn read(raw: Option<&Value>) -> Self {
        let defaults = Self::default();
        let Some(source) = obj(raw) else {
            return defaults;
        };
        let open_with = obj(source.get("openWith"));
        let service = |key: &str, fallback: Option<AppRef>| match open_with {
            Some(open_with) if open_with.contains_key(key) => AppRef::read(&open_with[key]),
            _ => fallback,
        };
        Self {
            start_first: list(source.get("startFirst"), AppRef::read)
                .unwrap_or(defaults.start_first),
            open_with: OpenWith {
                zoom: service("Zoom", defaults.open_with.zoom),
                teams: service("Teams", defaults.open_with.teams),
                meet: service("Meet", defaults.open_with.meet),
            },
        }
    }
}
