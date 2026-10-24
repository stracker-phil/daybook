//! EventKit: calendar access, the event calendars and their events, and change notifications. The only
//! module that talks to EventKit.

use std::ptr::NonNull;
use std::sync::Mutex;

use block2::RcBlock;
use chrono::{DateTime, Local, NaiveDateTime, TimeZone};
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_event_kit::{
    EKAuthorizationStatus, EKCalendar, EKEntityType, EKEvent, EKEventStatus, EKEventStore,
    EKEventStoreChangedNotification, EKParticipantStatus,
};
use objc2_foundation::{NSArray, NSDate, NSError, NSNotification, NSNotificationCenter};
use serde::Serialize;
use tokio::sync::oneshot;
use ts_rs::TS;

use super::events::{CalendarEvent, Event};

/// Whether Daybook may read the calendars.
#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq, TS)]
#[ts(export, export_to = "src/api/types.gen.ts")]
#[serde(rename_all = "kebab-case")]
pub enum CalendarAccess {
    /// macOS hasn't asked yet.
    NotDetermined,
    Restricted,
    Denied,
    FullAccess,
    WriteOnly,
}

pub fn access() -> CalendarAccess {
    match unsafe { EKEventStore::authorizationStatusForEntityType(EKEntityType::Event) } {
        EKAuthorizationStatus::FullAccess => CalendarAccess::FullAccess,
        EKAuthorizationStatus::Restricted => CalendarAccess::Restricted,
        EKAuthorizationStatus::Denied => CalendarAccess::Denied,
        EKAuthorizationStatus::WriteOnly => CalendarAccess::WriteOnly,
        _ => CalendarAccess::NotDetermined,
    }
}

/// Asks for full access to events; macOS asks the owner once and answers later calls by itself.
pub async fn request_access() -> CalendarAccess {
    // The store stays alive until macOS answers.
    let (_store, answered) = ask_for_access();
    let _ = answered.await;
    access()
}

fn ask_for_access() -> (Store, oneshot::Receiver<()>) {
    let (sender, answered) = oneshot::channel();
    let sender = Mutex::new(Some(sender));
    let completion = RcBlock::new(move |_granted: Bool, _error: *mut NSError| {
        if let Some(sender) = sender.lock().ok().and_then(|mut sender| sender.take()) {
            let _ = sender.send(());
        }
    });
    let store = Store::new();
    unsafe {
        store
            .0
            .requestFullAccessToEventsWithCompletion(RcBlock::as_ptr(&completion))
    };
    (store, answered)
}

/// One event store, used behind the calendar's lock only.
pub struct Store(Retained<EKEventStore>);

// SAFETY: EKEventStore may be used from any thread; `Calendar` serializes every use behind a mutex.
unsafe impl Send for Store {}

impl Store {
    pub fn new() -> Self {
        Self(unsafe { EKEventStore::new() })
    }

    fn event_calendars(&self) -> Retained<NSArray<EKCalendar>> {
        unsafe { self.0.calendarsForEntityType(EKEntityType::Event) }
    }

    /// The titles of the calendars with events.
    pub fn calendar_titles(&self) -> Vec<String> {
        self.event_calendars()
            .iter()
            .map(|calendar| unsafe { calendar.title() }.to_string())
            .collect()
    }

    /// The events between `from` and `to` in the calendars titled `titles`.
    pub fn events(
        &self,
        titles: &[String],
        from: NaiveDateTime,
        to: NaiveDateTime,
    ) -> Vec<CalendarEvent> {
        let chosen: Vec<Retained<EKCalendar>> = self
            .event_calendars()
            .iter()
            .filter(|calendar| titles.contains(&unsafe { calendar.title() }.to_string()))
            .collect();
        if chosen.is_empty() {
            return Vec::new();
        }
        let calendars = NSArray::from_retained_slice(&chosen);
        let events = unsafe {
            let predicate = self.0.predicateForEventsWithStartDate_endDate_calendars(
                &ns_date(from),
                &ns_date(to),
                Some(&calendars),
            );
            self.0.eventsMatchingPredicate(&predicate)
        };
        events.iter().map(|event| calendar_event(&event)).collect()
    }
}

fn calendar_event(event: &EKEvent) -> CalendarEvent {
    unsafe {
        CalendarEvent {
            event: Event {
                title: event.title().to_string(),
                calendar: event
                    .calendar()
                    .map(|calendar| calendar.title().to_string())
                    .unwrap_or_default(),
                start: local_time(&event.startDate()),
                end: local_time(&event.endDate()),
                url: event
                    .URL()
                    .and_then(|url| url.absoluteString())
                    .map(|url| url.to_string()),
                location: event.location().map(|text| text.to_string()),
                notes: event.notes().map(|text| text.to_string()),
            },
            all_day: event.isAllDay(),
            canceled: event.status() == EKEventStatus::Canceled,
            declined: declined_by_owner(event),
        }
    }
}

fn declined_by_owner(event: &EKEvent) -> bool {
    let Some(attendees) = (unsafe { event.attendees() }) else {
        return false;
    };
    attendees.iter().any(|attendee| unsafe {
        attendee.isCurrentUser() && attendee.participantStatus() == EKParticipantStatus::Declined
    })
}

fn ns_date(time: NaiveDateTime) -> Retained<NSDate> {
    let seconds = Local
        .from_local_datetime(&time)
        .earliest()
        .map_or(0.0, |local| local.timestamp_millis() as f64 / 1000.0);
    NSDate::dateWithTimeIntervalSince1970(seconds)
}

fn local_time(date: &NSDate) -> NaiveDateTime {
    let millis = (date.timeIntervalSince1970() * 1000.0).round() as i64;
    DateTime::from_timestamp_millis(millis)
        .unwrap_or_default()
        .with_timezone(&Local)
        .naive_local()
}

/// Calls `changed` whenever an event store reports a change: an event, a calendar or the access.
pub fn on_change(changed: impl Fn() + 'static) {
    let block = RcBlock::new(move |_: NonNull<NSNotification>| changed());
    let center = NSNotificationCenter::defaultCenter();
    let observer = unsafe {
        center.addObserverForName_object_queue_usingBlock(
            Some(EKEventStoreChangedNotification),
            None,
            None,
            &block,
        )
    };
    // The observer stays registered for the app's lifetime.
    std::mem::forget(observer);
}
