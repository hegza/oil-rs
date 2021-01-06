mod status;

use crate::datamodel::*;
use crate::prelude::*;
use serde::{Deserialize, Serialize};
pub use status::*;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct EventData {
    text: String,
    interval: Interval,
    stacks: bool,
}

impl EventData {
    pub fn new(interval: Interval, text: String) -> EventData {
        EventData {
            interval,
            text,
            stacks: false,
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
    pub fn interval(&self) -> &Interval {
        &self.interval
    }
    pub fn stacks(&self) -> bool {
        self.stacks
    }
}

impl std::fmt::Display for EventData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stack_str = match self.stacks {
            false => " (re-trigger overrides)",
            true => " (re-trigger stacks)",
        };
        write!(
            f,
            "EventData {{ \"{}\", interval: {}{} }}",
            self.text, &self.interval, stack_str
        )
    }
}
