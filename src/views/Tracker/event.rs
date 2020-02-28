use chrono::{DateTime, Local, NaiveTime as Time, Weekday};
use serde::{Deserialize, Serialize};
use std::fmt;

pub type LocalTime = DateTime<Local>;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Ord, PartialOrd, PartialEq, Eq)]
pub struct Id(pub usize);

impl fmt::Display for Id {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Event {
    interval: Interval,
    text: String,
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Interval {
    FromLastCompletion(TimeDelta),
    Annual(AnnualDay, Time),
    Monthly(MonthlyDay, Time),
    Weekly(Weekday, Time),
    //Daily(Time), // Not implemented
    //MultiAnnual(Vec<AnnualDay>) // Not implemented
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
pub enum EventState {
    // Never before triggered or completed
    Dormant { registered: LocalTime },
    Triggered(TriggerData),
    Completed(LocalTime),
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
