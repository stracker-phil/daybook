//! Accessibility, which reading other apps' window titles needs.

use objc2_application_services::{
    AXIsProcessTrusted, AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt,
};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFString};

pub const ACCESSIBILITY_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility";
pub const AUTOMATION_SETTINGS: &str =
    "x-apple.systempreferences:com.apple.preference.security?Privacy_Automation";

pub fn is_trusted() -> bool {
    // SAFETY: a plain query without arguments.
    unsafe { AXIsProcessTrusted() }
}

/// Shows macOS's prompt, which adds Daybook to Privacy → Accessibility; macOS shows it only once.
pub fn prompt() -> bool {
    let options = CFDictionary::<CFString, CFBoolean>::from_slices(
        // SAFETY: the key is the framework's constant string.
        &[unsafe { kAXTrustedCheckOptionPrompt }],
        &[CFBoolean::new(true)],
    );
    // SAFETY: the options dictionary holds the documented key.
    unsafe { AXIsProcessTrustedWithOptions(Some(options.as_opaque())) }
}
