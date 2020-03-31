pub mod command;
mod error;
pub mod event_store;
#[cfg(test)]
mod test;

use crate::event::{Event, State};
use crate::prelude::*;
use crate::tracker::command::{Apply, CommandReceiver, FnApply};
use dialoguer::Confirmation;
pub use error::LoadError;
use error::*;
pub use event_store::Uid;
use event_store::{EventStore, TrackedEvent};
use std::iter::FromIterator;
use std::path::Path;

pub struct Tracker {
    tracked_events: EventStore,
    undo_buffer: Vec<FnApply>,
}

impl Tracker {
    pub fn with_events(tracked_events: EventStore) -> Tracker {
        Tracker {
            tracked_events,
            undo_buffer: Vec::new(),
        }
    }
    pub fn empty() -> Tracker {
        Tracker {
            tracked_events: EventStore::new(),
            undo_buffer: Vec::new(),
        }
    }
    pub fn from_path<P>(path: P) -> Result<Tracker, LoadError>
    where
        P: AsRef<Path>,
    {
        debug!(
            "Reading events for tracker from path: {}",
            path.as_ref().to_string_lossy()
        );
        let events = EventStore::from_file(path);
        match events {
            Ok(events) => Ok(Tracker::with_events(events)),
            Err(e) => Err(e),
        }
    }

    pub fn update_events_from_disk(&mut self) {
        self.tracked_events.update_events();
    }
    pub fn refresh_from_disk<P>(&mut self, path: P) -> Result<(), LoadError>
    where
        P: AsRef<Path>,
    {
        self.tracked_events = match EventStore::from_file(&path) {
            Ok(ev) => ev,
            Err(e) => {
                warn!("Could not refresh events from disk: {:?}", e);
                return Err(e);
            }
        };
        Ok(())
    }
    pub fn store_to_disk<P>(&self, path: P)
    where
        P: AsRef<Path>,
    {
        match self.tracked_events.to_file(&path) {
            Ok(()) => {}
            Err(_) => {
                // FIXME: Tracker should not spawn GUI, forward to TrackerCli
                Confirmation::new()
                    .with_text("Failed to write to disk. Last operation will be cancelled.")
                    .show_default(false)
                    .interact()
                    .unwrap();
            }
        }
    }

    pub fn apply_command(&mut self, cmd: &dyn Apply) -> Result<(), CommandError> {
        let undo_op = match cmd.apply(CommandReceiver::Tracker(self)) {
            Ok(f) => f,
            Err(e) => {
                return Err(e);
            }
        };
        if let Some(apply) = undo_op {
            self.undo_buffer.push(Box::new(apply));
        }
        Ok(())
    }

    pub fn events(&self) -> Vec<(event_store::Uid, &TrackedEvent)> {
        Vec::from_iter(self.tracked_events.iter().map(|(uid, ev)| (*uid, ev)))
    }

    pub fn undo(&mut self) {
        trace!("Undo starts");

        // No-op if nothing in buffer
        if self.undo_buffer.is_empty() {
            debug!("Attempted to undo with empty undo buffer");
            println!("Cannot undo, undo buffer is empty");
            return;
        }

        let undo_op = self.undo_buffer.pop().unwrap();
        undo_op(self);
    }

    pub fn add_event(&mut self, event: Event) -> event_store::Uid {
        self.add_event_with_state(event, State::default())
    }

    // Returns None if an event was not found with id
    pub fn remove_event(&mut self, uid: event_store::Uid) -> Option<(Event, State)> {
        match self.tracked_events.remove(uid) {
            // Found: separate the return value
            Ok(te) => Some((te.event().clone(), te.state().clone())),
            // Not found: return None
            Err(_) => None,
        }
    }

    pub fn add_event_with_state(&mut self, event: Event, state: State) -> event_store::Uid {
        let uid = self.tracked_events.next_free_uid();
        debug!("Registering a new event with UID {}: {:?}", uid, event);
        let tracked_event = TrackedEvent::with_state(event, state);
        trace!("Created TrackedEvent: {:?}", &tracked_event);

        match self.tracked_events.add(uid, tracked_event) {
            Ok(()) => uid,
            Err(ItemAlreadyExistsError(k, ov, _)) => {
                panic!(
                    "Attempted to register an event with UID {} that was already reserved for: {:#?}", k, ov
                );
            }
        }
    }

    /// Returns the event as mutable if it exists with given UID
    pub fn get_event_mut(&mut self, uid: event_store::Uid) -> Option<&mut TrackedEvent> {
        self.tracked_events.get_mut(uid).ok()
    }

    /// Gets the state of the event as mutable if event exists with given UID
    pub fn get_event_state_mut(&mut self, uid: event_store::Uid) -> Option<&mut State> {
        self.get_event_mut(uid).map(|e| e.state_mut())
    }
}
