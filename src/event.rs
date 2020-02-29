use crate::prelude::*;
use chrono::{Local, NaiveTime as Time, Weekday};
use serde::{Deserialize, Serialize};
use std::default::Default;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    text: String,
    interval: Interval,
    stacks: bool,
}

impl Event {
    pub fn new(interval: Interval, text: String) -> Event {
        Event {
            interval,
            text,
            stacks: true,
        }
    }
    pub fn text(&self) -> &str {
        &self.text
    }
}

impl std::fmt::Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let stack_str = match self.stacks {
            false => " (re-trigger overrides)",
            true => " (re-trigger stacks)",
        };
        write!(
            f,
            "Event {{ \"{}\", interval: {}{} }}",
            self.text, &self.interval, stack_str
        )
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Interval {
    FromLastCompletion(TimeDelta),
    Annual(AnnualDay, Time),
    Monthly(MonthlyDay, Time),
    Weekly(Weekday, Time),
    //Daily(Time), // Not implemented
    //MultiAnnual(Vec<AnnualDay>) // Not implemented
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        warn!("Displaying an interval is not implemented");
        write!(f, "Interval:Display_not_implemented")
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TimeDelta {
    Days(i32),
    Hms {
        hours: u32,
        minutes: u32,
        seconds: u32,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnnualDay {
    pub month: i32,
    pub day: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MonthlyDay {
    pub day: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum State {
    // Never before triggered or completed
    Dormant { registered: LocalTime },
    Triggered(TriggerData),
    Completed(LocalTime),
}

impl Default for State {
    fn default() -> Self {
        State::Dormant {
            registered: Local::now(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct TriggerData {
    trigger_times: Vec<LocalTime>,
}

impl TriggerData {
    pub fn first_triggered(&self) -> &LocalTime {
        self.trigger_times.first().unwrap()
    }
    pub fn last_triggered(&self) -> &LocalTime {
        self.trigger_times.last().unwrap()
    }
}
