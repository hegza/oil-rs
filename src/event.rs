use crate::prelude::*;
use chrono::{Duration, Local, NaiveTime as Time, Weekday};
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
    Daily(Time),
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
    Days(i64),
    Hms(i64, i64, i64),
}

impl TimeDelta {
    pub fn apply_to(&self, time: LocalTime) -> LocalTime {
        time + self.to_duration()
    }
    pub fn to_duration(&self) -> Duration {
        match self {
            TimeDelta::Days(d) => Duration::days(*d),
            TimeDelta::Hms(h, m, s) => {
                Duration::hours(*h as i64)
                    + Duration::minutes(*m as i64)
                    + Duration::seconds(*s as i64)
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct AnnualDay {
    pub month: u32,
    pub day: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MonthlyDay {
    pub day: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum State {
    // Never before triggered or completed, .0 is time registered
    Dormant(LocalTime),
    // .0 is all trigger times since last completion
    Triggered(Vec<LocalTime>),
    // Completed and ready to trigger again
    Completed(LocalTime),
}

impl Default for State {
    fn default() -> Self {
        State::Dormant(Local::now())
    }
}
