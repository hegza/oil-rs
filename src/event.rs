use crate::prelude::*;
use crate::tracker::Time;
use chrono::{Duration, NaiveTime, Weekday};
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
    MultiAnnual(Vec<AnnualDay>), // Not implemented
}

impl Interval {
    pub fn to_duration_heuristic(&self) -> Option<Duration> {
        use Interval::*;
        match self {
            FromLastCompletion(delta) => Some(delta.to_duration()),
            Annual(_, _) => Some(Duration::days(365)),
            Monthly(_, _) => Some(Duration::days(30)),
            Weekly(_, _) => Some(Duration::days(7)),
            Daily(_) => Some(Duration::days(1)),
            MultiAnnual(_) => None,
        }
    }
}

impl std::fmt::Display for Interval {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Interval::*;
        match self {
            FromLastCompletion(delta) => write!(f, "triggers {} after previous completion", delta),
            MultiAnnual(days) => unimplemented!(),
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
pub struct Status {
    trigger_times: Vec<Time>,
    pub status: StatusKind,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum StatusKind {
    // Never before triggered or completed with time of registration
    Dormant(Time),
    // Triggered
    Triggered,
    // Completed and ready to trigger again with time of completion
    Completed(Time),
    // Skipped until next trigger, with time of skip
    Skip(Time),
}

/// Implements PartialEq such that Dormant(t), Completed(t) and Skip(t) instances are equal regardless of inner time value.
impl PartialEq for StatusKind {
    fn eq(&self, other: &Self) -> bool {
        use StatusKind::*;
        match (self, other) {
            (Dormant(_), Dormant(_)) => true,
            (Completed(_), Completed(_)) => true,
            (Skip(_), Skip(_)) => true,
            (Triggered, Triggered) => true,
            // Cover the false cases to future-proof and cause a compile error when a new variant is added.
            (Dormant(_), _) => false,
            (Triggered, _) => false,
            (Completed(_), _) => false,
            (Skip(_), _) => false,
        }
    }
}

impl Default for Status {
    fn default() -> Self {
        Status {
            trigger_times: vec![],
            status: StatusKind::Dormant(Time::now()),
        }
    }
}

impl Default for StatusKind {
    fn default() -> Self {
        StatusKind::Dormant(Time::now())
    }
}

impl Status {
    /// Returns true if status is exactly `Triggered`
    pub fn is_triggered(&self) -> bool {
        match self.status {
            StatusKind::Dormant(_) | StatusKind::Completed(_) | StatusKind::Skip(_) => false,
            StatusKind::Triggered => true,
        }
    }
    /// Returns true if status is exactly `Completed`
    pub fn is_done(&self) -> bool {
        match self.status {
            StatusKind::Dormant { .. } | StatusKind::Triggered { .. } | StatusKind::Skip { .. } => {
                false
            }
            StatusKind::Completed { .. } => true,
        }
    }

    /// Returns true if the event moved from an untriggered start to a triggered state
    pub fn trigger_now(&mut self) -> bool {
        let now = Time::now();
        match &self.status {
            StatusKind::Dormant { .. } | StatusKind::Completed(_) => {
                self.status = StatusKind::Triggered;
                self.trigger_times = vec![now];
                true
            }
            StatusKind::Skip(_time_of_skip) => {
                self.status = StatusKind::Completed(now);
                self.trigger_times = vec![];
                false
            }
            // Just add another triggering for already triggered events
            StatusKind::Triggered => {
                self.trigger_times.push(now);
                false
            }
        }
    }
    /// Returns true if the event was completed at this time as effect of this function. Resets the list of trigger times to vec![]. Sets the item completed even if the item is set to be skipped.
    pub fn complete_now(&mut self) -> bool {
        let now = Time::now();
        let ret;
        if let StatusKind::Completed(_) = self.status {
            ret = true;
        } else {
            ret = false;
        }

        self.trigger_times = vec![];
        self.status = StatusKind::Completed(now);

        ret
    }
    /// Sets the event as skipped until the next trigger time
    pub fn skip_now(&mut self) {
        let now = Time::now();
        *self = Status {
            trigger_times: vec![],
            status: StatusKind::Skip(now),
        };
    }

    pub fn prev_trigger_time(&self) -> Option<LocalTime> {
        self.trigger_times.last().and_then(|t| Some(t.0))
    }
}
