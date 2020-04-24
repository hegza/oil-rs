use super::*;
use crate::event::{EventData, Interval, Status, TimeDelta};
use crate::view::tracker_cli::TrackerCli;
use lazy_static::lazy_static;

lazy_static! {
    static ref TEST_EVENT: EventData = EventData::new(
        Interval::FromLastCompletion(TimeDelta::Hm(0, 1)),
        "Test EventData".to_string(),
    );
}

#[test]
fn event_lifecycle() {
    let mut tracker = Tracker::empty();

    let handle = tracker.add_event(TEST_EVENT.clone());

    // Verify that the event is accessible with its handle
    let event = tracker.event_mut(handle).unwrap();

    // Verify that the event is in dormant state
    match event.1 {
        Status::Dormant { .. } => {}
        _ => unreachable!(),
    }

    // TODO: Verify that the event is set to trigger after the time delta

    // Remove the event
    tracker.remove_event(handle);

    // Verify that the event is removed
    assert!(tracker.event_mut(handle).is_none());
}
