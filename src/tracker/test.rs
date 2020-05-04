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

#[test]
fn multiset_done() {
    let mut cli = TrackerCli::new(Tracker::empty());
    let ev = TEST_EVENT.clone();

    let events = {
        let tracker = &mut cli.tracker;

        // Add three events
        let evs = (0..3)
            .into_iter()
            .map(|_| tracker.add_event(ev.clone()))
            .collect::<Vec<Uid>>();

        // Set events as triggered
        evs.into_iter().for_each(|uid| {
            tracker.event_mut(uid).unwrap().trigger_now();
        });

        tracker.events()
    };

    // Check that all events are triggered
    assert!(events.iter().all(|(_, ev)| ev.is_triggered()));

    // Pick two of the events to be set as done
    let mut ev_it = events.iter().enumerate();
    let set_ev_ids = ev_it
        .by_ref()
        .take(2)
        .map(|(idx, (uid, _))| (idx, uid.clone()))
        .collect::<Vec<(usize, Uid)>>();
    let unset_ev_ids = ev_it
        .take(1)
        .map(|(idx, (uid, _))| (idx, uid.clone()))
        .collect::<Vec<(usize, Uid)>>();

    // Command to set two of the events as done, ie. "0 1"
    let cmd = format!("{} {}", set_ev_ids[0].0, set_ev_ids[1].0);

    // Set two events as done
    cli.call(&cmd);

    let tracker = &mut cli.tracker;

    // Verify that the two events are done, and the last one is still triggered
    assert!(
        tracker.event(set_ev_ids[0].1).unwrap().is_done(),
        "first event was not done after setting it done"
    );
    assert!(tracker.event(set_ev_ids[1].1).unwrap().is_done());
    assert!(tracker.event(unset_ev_ids[0].1).unwrap().is_triggered());
}

#[test]
fn trigger() {
    let mut cli = TrackerCli::new(Tracker::empty());
    let ev = TEST_EVENT.clone();

    let events = {
        let tracker = &mut cli.tracker;

        // Add two events
        tracker.add_event(ev.clone());
        tracker.add_event(ev.clone());

        tracker.events()
    };

    // Check that all events are not triggered
    assert!(events.iter().all(|(_, ev)| !ev.is_triggered()));

    // Pick an event to be set as done
    let mut ev_it = events.into_iter().enumerate();
    let (trig_ev_id, trig_uid) = {
        let ev = ev_it.next().unwrap();
        (ev.0, (ev.1).0)
    };
    let untrig_uid = (ev_it.next().unwrap().1).0;

    // Trigger an event
    // Command to trigger an event, ie. "trigger 1"
    let cmd = format!("trigger {}", trig_ev_id);
    cli.call(&cmd);

    let tracker = &mut cli.tracker;

    // Verify that one event is triggered, one is not
    assert!(
        tracker.event(trig_uid).unwrap().is_triggered(),
        "first event was not triggered after setting it "
    );
    assert!(!tracker.event(untrig_uid).unwrap().is_triggered());
}
