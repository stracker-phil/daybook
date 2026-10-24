use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A look for one macOS mode: `Default` has a dark sidebar beside light content, `Light` is all light,
/// `Dark` is all dark.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    #[default]
    Default,
    Light,
    Dark,
}

/// The size of all text. The frontend maps each step to its pixel sizes (`TEXT_SIZES` in `src/api/types.ts`).
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "lowercase")]
pub enum TextSize {
    Small,
    #[default]
    Medium,
    Large,
}

/// Accent hues; every palette defines each one.
pub const ACCENTS: &[&str] = &[
    "orange", "gold", "red", "magenta", "purple", "indigo", "blue", "teal", "graphite",
];

pub fn is_accent(name: &str) -> bool {
    ACCENTS.contains(&name)
}

/// The app accent that follows the macOS accent color.
pub const SYSTEM_ACCENT: &str = "system";

/// Whether `name` can be the app accent: the system's or one of `ACCENTS`.
pub fn is_app_accent(name: &str) -> bool {
    name == SYSTEM_ACCENT || is_accent(name)
}
