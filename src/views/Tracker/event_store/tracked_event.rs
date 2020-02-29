use crate::event::{Event, State};
use crate::prelude::*;
use chrono::Local;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TrackedEvent(Event, State);

impl TrackedEvent {
    pub fn with_state(source: Event, state: State) -> TrackedEvent {
        TrackedEvent(source, state)
    }
    pub fn complete_now(&mut self) -> LocalTime {
        let now = Local::now();
        self.1 = State::Completed(now);
        now
    }
    pub fn text(&self) -> &str {
        self.0.text()
    }
    pub fn event(&self) -> &Event {
        &self.0
    }
    pub fn state(&self) -> &State {
        &self.1
    }
}
