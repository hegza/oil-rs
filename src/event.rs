use crate::prelude::*;
use crate::tracker::Time;
use chrono::{Duration, Local, NaiveTime, Weekday};
use serde::{Deserialize, Serialize};
use std::default::Default;

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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Interval {
    FromLastCompletion(TimeDelta),
    Annual(AnnualDay, NaiveTime),
    Monthly(MonthlyDay, NaiveTime),
    Weekly(Weekday, NaiveTime),
    Daily(NaiveTime),
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
    Hm(i64, i64),
}

impl std::fmt::Display for TimeDelta {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TimeDelta::*;
        match self {
            Days(n) => write!(f, "{} days", n),
            Hm(h, m) => {
                match h {
                    0 => write!(f, ""),
                    h => write!(f, "{}h", h),
                }?;
                match m {
                    0 => write!(f, ""),
                    m => write!(f, "{}m", m),
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn hours_minutes_formats_right() {
        let dt = TimeDelta::Hm(1, 20);
        assert_eq!(&format!("{}", dt), "1h20m");

        let dt = TimeDelta::Hm(0, 15);
        assert_eq!(&format!("{}", dt), "15m");

        let dt = TimeDelta::Hm(5, 0);
        assert_eq!(&format!("{}", dt), "5h");
    }
}

impl TimeDelta {
    pub fn apply_to(&self, time: LocalTime) -> LocalTime {
        time + self.to_duration()
    }
    pub fn to_duration(&self) -> Duration {
        match self {
            TimeDelta::Days(d) => Duration::days(*d),
            TimeDelta::Hm(h, m) => Duration::hours(*h as i64) + Duration::minutes(*m as i64),
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

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum Status {
    // Never before triggered or completed, .0 is time registered
    Dormant(Time),
    // .0 is all trigger times since last completion
    TriggeredAt(Vec<Time>),
    // Completed and ready to trigger again
    Completed(Time),
}

impl Default for Status {
    fn default() -> Self {
        Status::Dormant(Time(Local::now()))
    }
}

impl Status {
    pub fn is_triggered(&self) -> bool {
        match self {
            Status::Dormant { .. } | Status::Completed { .. } => false,
            Status::TriggeredAt { .. } => true,
        }
    }
    pub fn is_done(&self) -> bool {
        match self {
            Status::Dormant { .. } | Status::TriggeredAt { .. } => false,
            Status::Completed { .. } => true,
        }
    }

    /// Returns true if the event moved from an untriggered start to a triggered state
    pub fn trigger_now(&mut self) -> bool {
        let now = Local::now();
        match self {
            Status::Dormant { .. } | Status::Completed(_) => {
                *self = Status::TriggeredAt(vec![now.into()]);
                true
            }
            Status::TriggeredAt(trigger_times) => {
                trigger_times.push(now.into());
                false
            }
        }
    }
}
