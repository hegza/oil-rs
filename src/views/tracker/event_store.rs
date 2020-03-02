mod tracked_event;

use crate::prelude::*;
use crate::views::tracker::error::{ItemAlreadyExistsError, LoadError, StoreError};
use crate::views::tracker::NotFoundError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::Path;
pub use tracked_event::TrackedEvent;

#[derive(Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventStore(BTreeMap<Uid, TrackedEvent>);

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Ord, PartialOrd, PartialEq, Eq)]
#[serde(transparent)]
pub struct Uid(pub usize);

impl EventStore {
    /// Returns a new, empty event store
    pub fn new() -> EventStore {
        EventStore(BTreeMap::new())
    }

    /// Returns an event store from a YAML file containing a valid event store
    pub fn from_file<P>(path: P) -> Result<EventStore, LoadError>
    where
        P: AsRef<Path>,
    {
        match fs::OpenOptions::new().read(true).open(&path) {
            Ok(mut file) => {
                let mut contents = String::new();
                file.read_to_string(&mut contents)
                    .expect("cannot read file to string");

                if contents.is_empty() {
                    return Ok(EventStore::new());
                }

                // Try load tracker from file
                let events = serde_yaml::from_str::<EventStore>(&contents);
                match events {
                    Ok(mut events) => {
                        events.update_events();
                        Ok(events)
                    }
                    Err(e) => Err(LoadError::FileContentsMalformed(
                        Box::new(e),
                        path.as_ref().to_string_lossy().to_string(),
                        contents,
                    )),
                }
            }
            _ => Err(LoadError::FileDoesNotExist),
        }
    }

    /// Stores an event store to a YAML file
    pub fn to_file<P>(&self, path: P) -> Result<(), StoreError>
    where
        P: AsRef<Path>,
    {
        // Write the file
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .append(false)
            .truncate(true)
            .open(&path);
        match file {
            Ok(ref mut file) => {
                trace!("Serializing event store");
                match serde_yaml::to_string(self) {
                    Ok(content_str) => {
                        trace!(
                            "Writing serialized stream to file \"{}\"",
                            path.as_ref().canonicalize().unwrap().to_string_lossy()
                        );
                        match file.write_all(&content_str.as_bytes()) {
                            Ok(()) => {
                                trace!("Write successful");
                                Ok(())
                            }
                            Err(_) => Err(StoreError::WriteFailed),
                        }
                    }
                    Err(e) => Err(StoreError::SerializeFailed(Box::new(e))),
                }
            }
            Err(_) => Err(StoreError::FileCreateFailed),
        }
    }

    /// Returns the stored events as an ordered map (inner type)
    #[allow(dead_code)]
    pub fn events_by_uid(&self) -> &BTreeMap<Uid, TrackedEvent> {
        &self.0
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Uid, &TrackedEvent)> {
        self.0.iter()
    }

    pub fn update_events(&mut self) {
        for tracked_event in self.0.values_mut() {
            tracked_event.update();
        }
    }

    /// Resolves the next free UID based on the events that currently exist
    pub fn next_free_uid(&self) -> Uid {
        match self.0.iter().map(|(&uid, _)| uid).max() {
            // No events? Return zero
            None => Uid(0),
            // Return the highest event UID + 1
            Some(uid) => uid.next(),
        }
    }

    /// Adds an event by UID
    ///
    /// If the data structure already had an item with this UID, ItemAlreadyExistsError is returned.
    pub fn add(
        &mut self,
        uid: Uid,
        event: TrackedEvent,
    ) -> Result<(), ItemAlreadyExistsError<Uid, TrackedEvent>> {
        match self.0.insert(uid, event.clone()) {
            None => Ok(()),
            Some(te) => Err(ItemAlreadyExistsError(uid, te, event)),
        }
    }

    /// Removes and returns an event by UID
    ///
    /// If the data structure did not have an item with this UID, NotFoundError is returned
    pub fn remove(&mut self, uid: Uid) -> Result<TrackedEvent, NotFoundError<Uid>> {
        match self.0.remove(&uid) {
            Some(te) => Ok(te),
            None => Err(NotFoundError(uid)),
        }
    }

    /// Returns a &mut to a stored event
    ///
    /// If the data structure did not have an item with this UID, NotFoundError is returned
    pub fn get_mut(&mut self, uid: Uid) -> Result<&mut TrackedEvent, NotFoundError<Uid>> {
        match self.0.get_mut(&uid) {
            Some(te) => Ok(te),
            None => Err(NotFoundError(uid)),
        }
    }
}

impl Uid {
    pub fn next(self) -> Uid {
        Uid(self.0 + 1)
    }
}

impl fmt::Display for Uid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
