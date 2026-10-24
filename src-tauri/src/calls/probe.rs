//! The detectors' questions to the Mac: `lsappinfo`, `pgrep`, `pmset` and `ps` (no permission needed),
//! the window server, Chrome's tabs through AppleScript (Automation) and window titles through
//! Accessibility. Blocking.

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::ptr::NonNull;
use std::sync::Mutex;
use std::time::Duration;

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_app_kit::NSRunningApplication;
use objc2_application_services::{AXError, AXUIElement};
use objc2_core_foundation::{CFArray, CFRetained, CFString, CFType};
use objc2_core_graphics::{CGWindowListCopyWindowInfo, CGWindowListOption, kCGNullWindowID};
use objc2_foundation::NSString;
use serde::Deserialize;

use super::access;
use super::checks::{
    TEAMS_BUNDLE, automation_refused, bundle_path, microphone_pids, running_bundle_ids,
    runs_from_bundle,
};
use super::detect::{Missing, Probe, ProbeError};
use crate::lock::MutexExt;
use crate::process::{self, Output};

const LIST_TIMEOUT: Duration = Duration::from_secs(5);
const SCRIPT_TIMEOUT: Duration = Duration::from_secs(8);
/// A hung app must not stall the check.
const AX_TIMEOUT_SECONDS: f32 = 0.5;
/// Teams shows a window on this level only during a call, on any Space.
const TEAMS_CALL_LAYER: f64 = 19.0;
const TEAMS_OWNER: &str = "Microsoft Teams";

const CHROME_TABS_JXA: &str = r#"function run() {
  var chrome = Application("Google Chrome");
  if (!chrome.running()) return "{}";
  return JSON.stringify({ urls: chrome.windows.tabs.url(), titles: chrome.windows.tabs.title() });
}"#;

fn run(program: &str, args: &[&str], timeout: Duration) -> Result<Output, String> {
    let args: Vec<String> = args.iter().map(|arg| arg.to_string()).collect();
    process::run(Path::new(program), &args, timeout).map_err(|e| e.to_string())
}

/// Where each app is installed, from Launch Services; cached per bundle id.
#[derive(Default)]
pub struct MacProbe {
    bundle_paths: Mutex<HashMap<String, String>>,
}

impl MacProbe {
    fn bundle_path(&self, bundle_id: &str) -> Option<String> {
        if let Some(known) = self.bundle_paths.lock_unpoisoned().get(bundle_id) {
            return Some(known.clone());
        }
        let args = ["info", "-only", "bundlepath", "-app", bundle_id];
        let output = run("/usr/bin/lsappinfo", &args, LIST_TIMEOUT).ok()?;
        let found = bundle_path(&output.stdout)?;
        self.bundle_paths
            .lock_unpoisoned()
            .insert(bundle_id.to_string(), found.clone());
        Some(found)
    }

    /// Whether a process of the app, its helpers included, records from a microphone.
    fn uses_microphone(&self, bundle_id: &str) -> Result<bool, String> {
        let assertions = run("/usr/bin/pmset", &["-g", "assertions"], LIST_TIMEOUT)?;
        let pids = microphone_pids(&assertions.stdout);
        if pids.is_empty() {
            return Ok(false);
        }
        let Some(app_path) = self.bundle_path(bundle_id) else {
            return Ok(false);
        };
        let pids = pids
            .iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(",");
        // ps fails when none of the processes exists any more.
        let executables = run("/bin/ps", &["-o", "comm=", "-p", &pids], LIST_TIMEOUT)?;
        Ok(runs_from_bundle(&executables.stdout, &app_path))
    }
}

#[derive(Deserialize, Default)]
struct ChromeTabs {
    #[serde(default)]
    urls: Vec<Vec<Option<String>>>,
    #[serde(default)]
    titles: Vec<Vec<Option<String>>>,
}

fn flat(windows: Vec<Vec<Option<String>>>) -> Vec<String> {
    windows
        .into_iter()
        .flatten()
        .map(Option::unwrap_or_default)
        .collect()
}

impl Probe for MacProbe {
    fn running(&self) -> Result<HashSet<String>, String> {
        let output = run("/usr/bin/lsappinfo", &["list"], LIST_TIMEOUT)?;
        if !output.success {
            return Err(format!("lsappinfo failed: {}", output.stderr.trim()));
        }
        Ok(running_bundle_ids(&output.stdout))
    }

    /// Zoom runs CptHost only while in a meeting; pgrep fails without a match.
    fn zoom_in_call(&self) -> Result<bool, String> {
        let output = run("/usr/bin/pgrep", &["-x", "CptHost"], LIST_TIMEOUT)?;
        match (output.success, output.stderr.trim()) {
            (true, _) => Ok(true),
            (false, "") => Ok(false),
            (false, error) => Err(format!("pgrep failed: {error}")),
        }
    }

    /// Teams in a call: it records from the microphone (also while in front), or shows its floating
    /// call window, which it does on other Spaces even while muted.
    fn teams_in_call(&self) -> Result<bool, String> {
        let recording = self.uses_microphone(TEAMS_BUNDLE).unwrap_or_else(|e| {
            log::warn!("Checking the microphone failed: {e}");
            false
        });
        Ok(recording || has_window(TEAMS_OWNER, TEAMS_CALL_LAYER))
    }

    fn chrome_tabs(&self) -> Result<(Vec<String>, Vec<String>), ProbeError> {
        let args = ["-l", "JavaScript", "-e", CHROME_TABS_JXA];
        let output =
            run("/usr/bin/osascript", &args, SCRIPT_TIMEOUT).map_err(ProbeError::Failed)?;
        if !output.success {
            if automation_refused(&output.stderr) {
                return Err(ProbeError::Missing(Missing::Automation));
            }
            return Err(ProbeError::Failed(format!(
                "Reading Chrome's tabs failed: {}",
                output.stderr.trim()
            )));
        }
        let tabs: ChromeTabs = serde_json::from_str(output.stdout.trim()).unwrap_or_default();
        Ok((flat(tabs.urls), flat(tabs.titles)))
    }

    fn window_titles(&self, bundle_id: &str) -> Result<Vec<String>, ProbeError> {
        if !access::is_trusted() {
            return Err(ProbeError::Missing(Missing::Accessibility));
        }
        let apps = NSRunningApplication::runningApplicationsWithBundleIdentifier(
            &NSString::from_str(bundle_id),
        );
        let Some(app) = apps.firstObject() else {
            return Ok(Vec::new());
        };
        Ok(ax_window_titles(app.processIdentifier()))
    }
}

fn ax_window_titles(pid: i32) -> Vec<String> {
    // SAFETY: an application element of a live pid; attributes are read with a timeout.
    unsafe {
        let app = AXUIElement::new_application(pid);
        app.set_messaging_timeout(AX_TIMEOUT_SECONDS);
        let Ok(windows) = attribute(&app, "AXWindows") else {
            return Vec::new();
        };
        let Ok(windows) = windows.downcast::<CFArray>() else {
            return Vec::new();
        };
        let windows: &CFArray<AXUIElement> = windows.cast_unchecked();
        windows
            .iter()
            .filter_map(|window| {
                let title = attribute(&window, "AXTitle").ok()?;
                title.downcast_ref::<CFString>().map(ToString::to_string)
            })
            .filter(|title| !title.is_empty())
            .collect()
    }
}

/// # Safety
/// `element` must be a valid accessibility element.
unsafe fn attribute(element: &AXUIElement, name: &str) -> Result<CFRetained<CFType>, AXError> {
    let mut value: *const CFType = std::ptr::null();
    // SAFETY: as the caller promises; `value` is an out pointer.
    let err = unsafe {
        element.copy_attribute_value(&CFString::from_str(name), NonNull::from(&mut value))
    };
    if err != AXError::Success {
        return Err(err);
    }
    NonNull::new(value.cast_mut())
        // SAFETY: a copied attribute value is owned by the caller.
        .map(|value| unsafe { CFRetained::from_raw(value) })
        .ok_or(AXError::NoValue)
}

/// Whether `owner` has a window on `layer`, on any Space (the window server; no permission needed).
fn has_window(owner: &str, layer: f64) -> bool {
    let Some(list) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, kCGNullWindowID)
    else {
        return false;
    };
    let list: *const CFArray = &*list;
    // SAFETY: a CFArray of CFDictionary is toll-free bridged to an NSArray of NSDictionary.
    let (windows, count): (&AnyObject, usize) = unsafe {
        let windows: &AnyObject = &*list.cast();
        (windows, msg_send![windows, count])
    };
    (0..count).any(|i| {
        // SAFETY: an element of the window list.
        let info: Retained<AnyObject> = unsafe { msg_send![windows, objectAtIndex: i] };
        text(&info, "kCGWindowOwnerName").as_deref() == Some(owner)
            && number(&info, "kCGWindowLayer") == Some(layer)
    })
}

fn object_for_key(dictionary: &AnyObject, key: &str) -> Option<Retained<AnyObject>> {
    let key = NSString::from_str(key);
    // SAFETY: `dictionary` is an NSDictionary keyed by strings.
    unsafe { msg_send![dictionary, objectForKey: &*key] }
}

fn number(dictionary: &AnyObject, key: &str) -> Option<f64> {
    // SAFETY: the window list holds NSNumbers under numeric keys.
    object_for_key(dictionary, key).map(|number| unsafe { msg_send![&number, doubleValue] })
}

fn text(dictionary: &AnyObject, key: &str) -> Option<String> {
    object_for_key(dictionary, key)?
        .downcast::<NSString>()
        .ok()
        .map(|text| text.to_string())
}
