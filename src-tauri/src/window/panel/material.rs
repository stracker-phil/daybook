//! The overlay's translucent popover material, as the prototype's panel. The material view sits below
//! the webview (window-vibrancy), so the panel class swap and the alpha fade leave it alone.

use std::ffi::c_void;

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::NSString;
use tauri::utils::config::WindowEffectsConfig;
use tauri::window::{Effect, EffectState, EffectsBuilder};

use crate::store::settings::Scheme;

const RADIUS: f64 = 12.0;
/// The tag window-vibrancy gives its material view (`NS_VIEW_TAG_BLUR_VIEW`).
const MATERIAL_VIEW_TAG: isize = 91_376_254;

/// Always active: the panel never becomes main, so following the window's state would render it grey.
pub fn effects() -> WindowEffectsConfig {
    EffectsBuilder::new()
        .effect(Effect::Popover)
        .state(EffectState::Active)
        .radius(RADIUS)
        .build()
}

/// Whether the page draws dark content: the scheme picked for the current macOS mode is Dark
/// (`Default` has light content, as `schemeLook` in `src/lib/theme.ts`).
fn content_is_dark(light_scheme: Scheme, dark_scheme: Scheme, system_dark: bool) -> bool {
    let scheme = if system_dark {
        dark_scheme
    } else {
        light_scheme
    };
    scheme == Scheme::Dark
}

/// # Safety
/// On the main thread.
unsafe fn system_is_dark() -> bool {
    // SAFETY: plain NSApplication / NSAppearance getters on the main thread.
    unsafe {
        let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
        if app.is_null() {
            return false;
        }
        let appearance: *mut AnyObject = msg_send![&*app, effectiveAppearance];
        if appearance.is_null() {
            return false;
        }
        let name: *mut NSString = msg_send![&*appearance, name];
        !name.is_null() && (*name).to_string().contains("Dark")
    }
}

/// Gives the material the content's tone, so the page's text keeps its contrast when the chosen scheme
/// differs from the macOS mode. Only the material view changes: the page still sees the macOS mode.
///
/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
pub unsafe fn match_content(ns_window: *mut c_void, light_scheme: Scheme, dark_scheme: Scheme) {
    // SAFETY: as the caller promises.
    let dark = content_is_dark(light_scheme, dark_scheme, unsafe { system_is_dark() });
    let name = NSString::from_str(if dark {
        "NSAppearanceNameDarkAqua"
    } else {
        "NSAppearanceNameAqua"
    });
    // SAFETY: as the caller promises.
    let Some(material) = (unsafe { material_view(ns_window) }) else {
        return;
    };
    // SAFETY: plain NSAppearance calls on a live view, on the main thread.
    unsafe {
        let appearance: *mut AnyObject = msg_send![class!(NSAppearance), appearanceNamed: &*name];
        let _: () = msg_send![material, setAppearance: appearance];
    }
}

/// The material view window-vibrancy put into the window's content view.
///
/// # Safety
/// `ns_window` is a live `NSWindow`, on the main thread.
unsafe fn material_view<'a>(ns_window: *mut c_void) -> Option<&'a AnyObject> {
    // SAFETY: plain NSWindow / NSView getters on a live window.
    unsafe {
        let window = &*ns_window.cast::<AnyObject>();
        let content: *mut AnyObject = msg_send![window, contentView];
        if content.is_null() {
            return None;
        }
        let material: *mut AnyObject = msg_send![&*content, viewWithTag: MATERIAL_VIEW_TAG];
        material.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_is_dark_follows_the_scheme_of_the_current_macos_mode() {
        for (light, dark, system_dark, expected) in [
            (Scheme::Default, Scheme::Dark, false, false),
            (Scheme::Light, Scheme::Dark, false, false),
            (Scheme::Dark, Scheme::Default, true, false),
            (Scheme::Dark, Scheme::Default, false, true),
            (Scheme::Default, Scheme::Dark, true, true),
        ] {
            assert_eq!(
                content_is_dark(light, dark, system_dark),
                expected,
                "{light:?} / {dark:?}, system dark: {system_dark}"
            );
        }
    }

    #[test]
    fn the_material_is_one_always_active_popover_with_rounded_corners() {
        let config = effects();
        assert!(matches!(config.effects.as_slice(), [Effect::Popover]));
        assert!(matches!(config.state, Some(EffectState::Active)));
        assert_eq!(config.radius, Some(RADIUS));
    }
}
