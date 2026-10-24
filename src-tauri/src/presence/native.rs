//! The macOS side: sleep, wake, lock and unlock notifications, and the lock state from `ioreg`.

use std::path::Path;
use std::ptr::NonNull;
use std::time::Duration;

use block2::RcBlock;
use objc2_app_kit::{
    NSWorkspace, NSWorkspaceDidWakeNotification, NSWorkspaceWillSleepNotification,
};
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSNotificationCenter, NSNotificationName,
    NSString,
};

use super::checks::is_screen_locked_output;
use crate::process;

const IOREG_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Copy, Debug)]
pub enum SystemEvent {
    Sleep,
    Wake,
    Lock,
    Unlock,
}

/// Calls `on_event` on the main thread for every sleep, wake, lock and unlock; for the app's lifetime.
pub fn observe(on_event: impl Fn(SystemEvent) + Clone + 'static) {
    let center = NSWorkspace::sharedWorkspace().notificationCenter();
    // SAFETY: AppKit's constant notification names.
    let sleep = unsafe { NSWorkspaceWillSleepNotification };
    // SAFETY: as above.
    let wake = unsafe { NSWorkspaceDidWakeNotification };
    add_observer(&center, sleep, SystemEvent::Sleep, on_event.clone());
    add_observer(&center, wake, SystemEvent::Wake, on_event.clone());
    let distributed = NSDistributedNotificationCenter::defaultCenter();
    for (name, event) in [
        ("com.apple.screenIsLocked", SystemEvent::Lock),
        ("com.apple.screenIsUnlocked", SystemEvent::Unlock),
    ] {
        add_observer(
            &distributed,
            &NSString::from_str(name),
            event,
            on_event.clone(),
        );
    }
}

fn add_observer(
    center: &NSNotificationCenter,
    name: &NSNotificationName,
    event: SystemEvent,
    on_event: impl Fn(SystemEvent) + 'static,
) {
    let block = RcBlock::new(move |_: NonNull<NSNotification>| on_event(event));
    // SAFETY: the block lives as long as the observer, which is never removed.
    let observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block)
    };
    // Observers stay registered for the app's lifetime.
    std::mem::forget(observer);
}

/// Blocking. An unreadable state counts as locked, so nothing opens at a lock screen.
pub fn screen_locked() -> bool {
    let args = ["-n", "Root", "-d1", "-r"].map(String::from);
    let reason = match process::run(Path::new("/usr/sbin/ioreg"), &args, IOREG_TIMEOUT) {
        Ok(output) if output.success => return is_screen_locked_output(&output.stdout),
        Ok(output) => output.stderr.trim().to_string(),
        Err(e) => e.to_string(),
    };
    log::warn!("Couldn't read the screen lock state: {reason}");
    true
}
