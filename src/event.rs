use crate::prelude::*;
use chrono::{Duration, Local, NaiveTime as Time, Timelike, Weekday};
use serde::{Deserialize, Serialize, Serializer};
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

impl Interval {
    pub fn to_duration_heuristic(&self) -> Duration {
        use Interval::*;
        match self {
            FromLastCompletion(delta) => delta.to_duration(),
            Annual(_, _) => Duration::days(365),
            Monthly(_, _) => Duration::days(30),
            Weekly(_, _) => Duration::days(7),
            Daily(_) => Duration::days(1),
        }
    }
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Interval::*;
        match self {
            FromLastCompletion(delta) => write!(f, "triggers {} after previous completion", delta),
            Annual(day, time) => write!(
                f,
                "triggers annually on {} at {}",
                day,
                time.format("%H:%M")
            ),
            Monthly(day, time) => {
                write!(f, "triggers monthly on {} at {}", day, time.format("%H:%M"))
            }
            Weekly(weekday, time) => write!(
                f,
                "triggers weekly on {} at {}",
                weekday,
                time.format("%H:%M")
            ),
            Daily(time) => write!(f, "triggers daily at {}", time.format("%H:%M")),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum TimeDelta {
    Days(i64),
    Hms(i64, i64, i64),
}

impl std::fmt::Display for TimeDelta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TimeDelta::*;
        match self {
            Days(n) => write!(f, "{} days", n),
            // TODO: could leave out units for zero inputs
            Hms(h, m, s) => write!(f, "{}h{}m{}s", h, m, s),
        }
    }
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

impl std::fmt::Display for AnnualDay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.", self.month, self.day)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MonthlyDay {
    pub day: u32,
}

impl std::fmt::Display for MonthlyDay {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.", self.day)
    }
}

#[derive(Deserialize, Clone, Debug)]
pub enum State {
    // Never before triggered or completed, .0 is time registered
    Dormant(LocalTime),
    // .0 is all trigger times since last completion
    Triggered(Vec<LocalTime>),
    // Completed and ready to trigger again
    Completed(LocalTime),
}

// Implement a custom serializer with lower precision timestamps, note that the full timestamp must
// exist in runtime, but the serialized format can do with second precision.
impl Serialize for State {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match *self {
            State::Dormant(ref time) => s.serialize_newtype_variant(
                "State",
                0,
                "Dormant",
                // Time with truncated nanoseconds
                &time.with_nanosecond(0).unwrap_or(time.clone()),
            ),
            State::Triggered(ref times) => s.serialize_newtype_variant(
                "State",
                0,
                "Triggered",
                // Time with truncated nanoseconds
                &times
                    .iter()
                    .map(|time| time.with_nanosecond(0).unwrap_or(time.clone()))
                    .collect::<Vec<LocalTime>>(),
            ),
            State::Completed(ref time) => s.serialize_newtype_variant(
                "State",
                0,
                "Completed",
                // Time with truncated nanoseconds
                &time.with_nanosecond(0).unwrap_or(time.clone()),
            ),
        }
    }
}

impl Default for State {
    fn default() -> Self {
        State::Dormant(Local::now())
    }
}

impl State {
    pub fn trigger_now(&mut self, now: LocalTime) {
        match self {
            State::Dormant { .. } | State::Completed(_) => {
                *self = State::Triggered(vec![now]);
            }
            State::Triggered(trigger_times) => {
                trigger_times.push(now);
            }
        }
    }
}
