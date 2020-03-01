use super::*;
use crate::event::{Event, Interval, State, TimeDelta};

#[test]
fn event_lifecycle() {
    let mut tracker = Tracker::empty();

    let handle = tracker.add_event(Event::new(
        Interval::FromLastCompletion(TimeDelta::Hms {
            hours: 0,
            minutes: 0,
            seconds: 5,
        }),
        "Test Event".to_string(),
    ));

    // Verify that the event is accessible with its handle
    let event = tracker.get_mut_event(handle).unwrap();

    // Verify that the event is in dormant state
    match event.state() {
        State::Dormant { .. } => {}
        _ => unreachable!(),
    }

    // TODO: Verify that the event is set to trigger after the time delta

    // Remove the event
    tracker.remove_event(handle);

    // Verify that the event is removed
    assert!(tracker.get_mut_event(handle).is_none());
}
